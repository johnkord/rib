use actix_multipart::Multipart;
use actix_web::{web, HttpResponse};
use futures_util::TryStreamExt as _;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::str::FromStr;
use bitcoin::Address;

use crate::auth::{Auth, Role};
use crate::error::ApiError;
use crate::models::*;
use crate::repo::Repo;
use crate::storage::{ImageStore, ImageStoreError};
use actix_web::HttpRequest;

// Extract a best-effort client IP (for per-IP rate limiting). Prefers X-Forwarded-For first hop,
// then Forwarded header, then peer address. Falls back to "unknown".
fn extract_client_ip(req: &HttpRequest) -> String {
    // Standard X-Forwarded-For: client, proxy1, proxy2 ...
    if let Some(xff) = req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = xff.split(',').next() {
            let ip = first.trim();
            if !ip.is_empty() { return ip.to_string(); }
        }
    }
    // RFC 7239 Forwarded: for=1.2.3.4;proto=https;by=...
    if let Some(fwd) = req.headers().get("forwarded").and_then(|v| v.to_str().ok()) {
        for part in fwd.split(';') {
            let part = part.trim();
            if let Some(rest) = part.strip_prefix("for=") {
                // value may be quoted or have port
                let cleaned = rest.trim_matches('"');
                let ip_only = cleaned.split(',').next().unwrap_or(cleaned).split(':').next().unwrap_or(cleaned);
                if !ip_only.is_empty() { return ip_only.to_string(); }
            }
        }
    }
    // Actix connection info (may already respect proxy headers if configured)
    if let Some(peer) = req.peer_addr() { return peer.ip().to_string(); }
    "unknown".to_string()
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .service(
                web::resource("/boards")
                    .route(web::get().to(list_boards))
                    .route(web::post().to(create_board)),
            )
            .service(web::resource("/boards/{id}/threads").route(web::get().to(list_threads)))
            .service(web::resource("/threads").route(web::post().to(create_thread)))
            .service(web::resource("/threads/{id}").route(web::get().to(get_thread)))
            .service(web::resource("/threads/{id}/replies").route(web::get().to(list_replies)))
            .service(web::resource("/replies").route(web::post().to(create_reply)))
            .service(web::resource("/images").route(web::post().to(upload_image)))
            .service(
                web::resource("/boards/{id}")
                    .route(web::patch().to(update_board)),
            )
            .service(web::resource("/auth/discord/callback").route(web::get().to(discord_callback)))
            .service(web::resource("/auth/discord/login").route(web::get().to(discord_login)))
            .service(web::resource("/auth/bitcoin/challenge").route(web::post().to(bitcoin_challenge)))
            .service(web::resource("/auth/bitcoin/verify").route(web::post().to(bitcoin_verify)))
            .service(web::resource("/auth/refresh").route(web::post().to(refresh_token)))
            .service(web::resource("/admin/roles")
                .route(web::post().to(set_subject_role))
                .route(web::get().to(list_roles)))
            .service(web::resource("/admin/roles/{subject}").route(web::delete().to(delete_role)))
            .service(
                web::resource("/auth/me")
                    .route(web::get().to(auth_me)),
            )
            // Admin moderation endpoints
            .service(
                web::resource("/admin/boards/{id}/soft-delete")
                    .route(web::post().to(admin_soft_delete_board)),
            )
            .service(
                web::resource("/admin/boards/{id}/restore")
                    .route(web::post().to(admin_restore_board)),
            )
            .service(
                web::resource("/admin/boards/{id}")
                    .route(web::delete().to(admin_hard_delete_board)),
            )
            .service(
                web::resource("/admin/threads/{id}/soft-delete")
                    .route(web::post().to(admin_soft_delete_thread)),
            )
            .service(
                web::resource("/admin/threads/{id}/restore")
                    .route(web::post().to(admin_restore_thread)),
            )
            .service(
                web::resource("/admin/threads/{id}")
                    .route(web::delete().to(admin_hard_delete_thread)),
            )
            .service(
                web::resource("/admin/replies/{id}/soft-delete")
                    .route(web::post().to(admin_soft_delete_reply)),
            )
            .service(
                web::resource("/admin/replies/{id}/restore")
                    .route(web::post().to(admin_restore_reply)),
            )
            .service(
                web::resource("/admin/replies/{id}")
                    .route(web::delete().to(admin_hard_delete_reply)),
            )
    );
    // Public fetch route (no /api/v1 prefix so <img src="/images/{hash}"> works)
    cfg.route("/images/{hash}", web::get().to(get_image));
    // Simple health endpoint for k8s liveness/readiness (lighter than /docs)
    cfg.route("/healthz", web::get().to(health));
}

pub struct AppState {
    pub repo: Arc<dyn Repo>,
    pub image_store: Arc<dyn ImageStore>,
    pub rate_limiter: Option<crate::rate_limit::RateLimiterFacade>,
}

#[utoipa::path(
    get,
    path = "/api/v1/boards",
    params(("include_deleted" = Option<bool>, Query, description = "Admin only: include soft-deleted")),
    responses(
        (status = 200, description = "List boards", body = [Board])
    )
)]
pub async fn list_boards(
    req: HttpRequest,
    auth: Option<Auth>,
    data: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let want_deleted = req.query_string().contains("include_deleted=1");
    let is_admin = auth
        .as_ref()
        .map(|a| a.0.roles.iter().any(|r| matches!(r, Role::Admin)))
        .unwrap_or(false);
    let boards = data.repo.list_boards(is_admin && want_deleted).await?;
    Ok(HttpResponse::Ok().json(boards))
}

#[utoipa::path(
    post,
    path = "/api/v1/boards",
    request_body = NewBoard,
    responses(
        (status = 201, description = "Board created", body = Board),
        (status = 403, description = "Forbidden - Admins only"),   // UPDATED
        (status = 409, description = "Conflict")
    )
)]
pub async fn create_board(
    auth: Auth,
    data: web::Data<AppState>,
    payload: web::Json<NewBoard>,
) -> Result<HttpResponse, ApiError> {
    // ── admin-only guard ───────────────────────────────────────────
    if !auth.0.roles.iter().any(|r| matches!(r, Role::Admin)) {
        return Err(ApiError::Forbidden);
    }
    // ───────────────────────────────────────────────────────────────
    let board = data.repo.create_board(payload.into_inner()).await?;
    Ok(HttpResponse::Created().json(board))
}

#[utoipa::path(
    get,
    path = "/api/v1/boards/{id}/threads",
    params(
        ("id" = Id, Path, description = "Board id"),
        ("include_deleted" = Option<bool>, Query, description = "Admin only: include soft-deleted")
    ),
    responses(
        (status = 200, description = "List threads", body = [Thread]),
        (status = 404, description = "Board not found")
    )
)]
pub async fn list_threads(
    req: HttpRequest,
    auth: Option<Auth>,
    data: web::Data<AppState>,
    path: web::Path<Id>,
) -> Result<HttpResponse, ApiError> {
    let board_id = path.into_inner();
    let want_deleted = req.query_string().contains("include_deleted=1");
    let is_admin = auth
        .as_ref()
        .map(|a| a.0.roles.iter().any(|r| matches!(r, Role::Admin)))
        .unwrap_or(false);
    let board = data
        .repo
        .get_board(board_id)
        .await
        .map_err(|_| ApiError::NotFound)?;
    if board.deleted_at.is_some() && !(is_admin && want_deleted) {
        return Err(ApiError::NotFound);
    }
    let mut threads = data
        .repo
        .list_threads(board_id, is_admin && want_deleted)
        .await?;
    threads.sort_by(|a, b| b.bump_time.cmp(&a.bump_time));
    Ok(HttpResponse::Ok().json(threads))
}

#[utoipa::path(
    post,
    path = "/api/v1/threads",
    request_body = NewThread,
    responses(
        (status = 201, description = "Thread created", body = Thread),
        (status = 404, description = "Board not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn create_thread(
    auth: Auth,
    req: HttpRequest,
    data: web::Data<AppState>,
    payload: web::Json<NewThread>,
) -> Result<HttpResponse, ApiError> {
    if let Some(rl) = &data.rate_limiter {
        let ip = extract_client_ip(&req);
    if !rl.allow_thread(&ip) {
            metrics::increment_counter!("rate_limit_denied", "action" => "thread_create");
            return Err(ApiError::RateLimited { retry_after: rl.cfg.thread_window.as_secs() });
        }
    metrics::increment_counter!("rate_limit_allowed", "action" => "thread_create");
    }
    if !auth
        .0
        .roles
        .iter()
        .any(|r| matches!(r, Role::User | Role::Moderator | Role::Admin))
    {
        return Err(ApiError::Forbidden);
    }
    let board = data
        .repo
        .get_board(payload.board_id)
        .await
        .map_err(|_| ApiError::NotFound)?;
    if board.deleted_at.is_some() {
        return Err(ApiError::NotFound);
    }
    let new = payload.into_inner();
    // Derive a display author from JWT sub (format "id:username")
    let created_by = if let Some(rest) = auth.0.sub.strip_prefix("btc:") {
        serde_json::json!({
            "v": 1,
            "provider": "bitcoin",
            "address": rest,
            "display": format!("btc:{}", &rest[..std::cmp::min(rest.len(), 8)])
        })
    } else {
        let (discord_id, username) = match auth.0.sub.split_once(':') {
            Some((id, u)) => (id.to_string(), u.to_string()),
            None => (auth.0.sub.clone(), auth.0.sub.clone()),
        };
        serde_json::json!({
            "v": 1,
            "provider": "discord",
            "discord_id": discord_id,
            "username": username,
            "display": username,
        })
    };
    let thread = data.repo.create_thread(new, created_by).await?;
    Ok(HttpResponse::Created().json(thread))
}

#[utoipa::path(
    get,
    path = "/api/v1/threads/{id}",
    params(("id" = Id, Path, description = "Thread id"), ("include_deleted" = Option<bool>, Query, description = "Admin only: include soft-deleted")),
    responses(
        (status = 200, description = "Thread", body = Thread),
        (status = 404, description = "Thread not found")
    )
)]
pub async fn get_thread(
    req: HttpRequest,
    auth: Option<Auth>,
    data: web::Data<AppState>,
    path: web::Path<Id>,
) -> Result<HttpResponse, ApiError> {
    let want_deleted = req.query_string().contains("include_deleted=1");
    let is_admin = auth
        .as_ref()
        .map(|a| a.0.roles.iter().any(|r| matches!(r, Role::Admin)))
        .unwrap_or(false);
    let th = data
        .repo
        .get_thread(path.into_inner())
        .await
        .map_err(|e| match e {
            crate::repo::RepoError::NotFound => ApiError::NotFound,
            _ => ApiError::Internal,
        })?;
    if th.deleted_at.is_some() && !(is_admin && want_deleted) {
        return Err(ApiError::NotFound);
    }
    Ok(HttpResponse::Ok().json(th))
}

#[utoipa::path(
    get,
    path = "/api/v1/threads/{id}/replies",
    params(
        ("id" = Id, Path, description = "Thread id")
    ),
    responses(
        (status = 200, description = "List replies", body = [Reply]),
        (status = 404, description = "Thread not found")
    )
)]
pub async fn list_replies(
    req: HttpRequest,
    auth: Option<Auth>,
    data: web::Data<AppState>,
    path: web::Path<Id>,
) -> Result<HttpResponse, ApiError> {
    let thread_id = path.into_inner();
    let want_deleted = req.query_string().contains("include_deleted=1");
    let is_admin = auth
        .as_ref()
        .map(|a| a.0.roles.iter().any(|r| matches!(r, Role::Admin)))
        .unwrap_or(false);
    let thread = data
        .repo
        .get_thread(thread_id)
        .await
        .map_err(|_| ApiError::NotFound)?;
    if thread.deleted_at.is_some() && !(is_admin && want_deleted) {
        return Err(ApiError::NotFound);
    }
    let mut replies = data
        .repo
        .list_replies(thread_id, is_admin && want_deleted)
        .await?;
    replies.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(HttpResponse::Ok().json(replies))
}

// ---------------- Admin moderation handlers -----------------------
macro_rules! ensure_admin {
    ($auth:expr) => {
        if !$auth.0.roles.iter().any(|r| matches!(r, Role::Admin)) {
            return Err(ApiError::Forbidden);
        }
    };
}
macro_rules! ensure_moderator_or_admin {
    ($auth:expr) => {
        if !$auth
            .0
            .roles
            .iter()
            .any(|r| matches!(r, Role::Moderator | Role::Admin))
        {
            return Err(ApiError::Forbidden);
        }
    };
}

pub async fn admin_soft_delete_board(
    auth: Auth,
    data: web::Data<AppState>,
    path: web::Path<Id>,
) -> Result<HttpResponse, ApiError> {
    ensure_admin!(auth);
    data.repo.soft_delete_board(path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"status":"ok"})))
}
pub async fn admin_restore_board(
    auth: Auth,
    data: web::Data<AppState>,
    path: web::Path<Id>,
) -> Result<HttpResponse, ApiError> {
    ensure_admin!(auth);
    data.repo.restore_board(path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"status":"ok"})))
}
pub async fn admin_hard_delete_board(
    auth: Auth,
    data: web::Data<AppState>,
    path: web::Path<Id>,
) -> Result<HttpResponse, ApiError> {
    ensure_admin!(auth);
    data.repo.hard_delete_board(path.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

pub async fn admin_soft_delete_thread(
    auth: Auth,
    data: web::Data<AppState>,
    path: web::Path<Id>,
) -> Result<HttpResponse, ApiError> {
    ensure_moderator_or_admin!(auth);
    data.repo.soft_delete_thread(path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"status":"ok"})))
}
pub async fn admin_restore_thread(
    auth: Auth,
    data: web::Data<AppState>,
    path: web::Path<Id>,
) -> Result<HttpResponse, ApiError> {
    ensure_moderator_or_admin!(auth);
    data.repo.restore_thread(path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"status":"ok"})))
}
pub async fn admin_hard_delete_thread(
    auth: Auth,
    data: web::Data<AppState>,
    path: web::Path<Id>,
) -> Result<HttpResponse, ApiError> {
    ensure_admin!(auth);
    data.repo.hard_delete_thread(path.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

pub async fn admin_soft_delete_reply(
    auth: Auth,
    data: web::Data<AppState>,
    path: web::Path<Id>,
) -> Result<HttpResponse, ApiError> {
    ensure_moderator_or_admin!(auth);
    data.repo.soft_delete_reply(path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"status":"ok"})))
}
pub async fn admin_restore_reply(
    auth: Auth,
    data: web::Data<AppState>,
    path: web::Path<Id>,
) -> Result<HttpResponse, ApiError> {
    ensure_moderator_or_admin!(auth);
    data.repo.restore_reply(path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"status":"ok"})))
}
pub async fn admin_hard_delete_reply(
    auth: Auth,
    data: web::Data<AppState>,
    path: web::Path<Id>,
) -> Result<HttpResponse, ApiError> {
    ensure_admin!(auth);
    let id = path.into_inner();
    // Fetch reply to capture image hash before deletion
    let reply = data.repo.get_reply(id).await.ok();
    data.repo.hard_delete_reply(id).await?;
    if let Some(r) = reply {
        if let Some(hash) = r.image_hash {
            let _ = data.image_store.delete(&hash).await;
        }
    }
    Ok(HttpResponse::NoContent().finish())
}
// ------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/api/v1/replies",
    request_body = NewReply,
    responses(
        (status = 201, description = "Reply created", body = Reply),
        (status = 404, description = "Thread not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn create_reply(
    auth: Auth,
    req: HttpRequest,
    data: web::Data<AppState>,
    payload: web::Json<NewReply>,
) -> Result<HttpResponse, ApiError> {
    if let Some(rl) = &data.rate_limiter {
        let ip = extract_client_ip(&req);
    if !rl.allow_reply(&ip) {
            metrics::increment_counter!("rate_limit_denied", "action" => "reply_create");
            return Err(ApiError::RateLimited { retry_after: rl.cfg.reply_window.as_secs() });
        }
    metrics::increment_counter!("rate_limit_allowed", "action" => "reply_create");
    }
    if !auth
        .0
        .roles
        .iter()
        .any(|r| matches!(r, Role::User | Role::Moderator | Role::Admin))
    {
        return Err(ApiError::Forbidden);
    }
    let thread = data
        .repo
        .get_thread(payload.thread_id)
        .await
        .map_err(|_| ApiError::NotFound)?;
    if thread.deleted_at.is_some() {
        return Err(ApiError::NotFound);
    }
    let new = payload.into_inner();
    let created_by = if let Some(rest) = auth.0.sub.strip_prefix("btc:") {
        serde_json::json!({
            "v": 1,
            "provider": "bitcoin",
            "address": rest,
            "display": format!("btc:{}", &rest[..std::cmp::min(rest.len(), 8)])
        })
    } else {
        let (discord_id, username) = match auth.0.sub.split_once(':') {
            Some((id, u)) => (id.to_string(), u.to_string()),
            None => (auth.0.sub.clone(), auth.0.sub.clone()),
        };
        serde_json::json!({
            "v": 1,
            "provider": "discord",
            "discord_id": discord_id,
            "username": username,
            "display": username,
        })
    };
    let reply = data.repo.create_reply(new, created_by).await?;
    Ok(HttpResponse::Created().json(reply))
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct FileUploadResponse {
    pub hash: String,
    pub mime: String,
    pub size: usize,
    pub duplicate: bool, // true when upload was a duplicate (idempotent)
}

const FILE_SIZE_LIMIT: usize = 25 * 1024 * 1024; // 25 MB

const ALLOWED_MIME: &[&str] = &[
    // Images
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/bmp",
    "image/tiff",
    "image/svg+xml",
    // Videos
    "video/mp4",
    "video/webm",
    "video/avi",
    "video/mov",
    "video/wmv",
    "video/flv",
    // Audio
    "audio/mpeg",
    "audio/wav",
    "audio/ogg",
    "audio/flac",
    "audio/aac",
    "audio/m4a",
    // Documents
    "application/pdf",
    "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.ms-excel",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.ms-powerpoint",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "application/rtf",
    "application/vnd.oasis.opendocument.text",
    "application/vnd.oasis.opendocument.spreadsheet",
    "application/vnd.oasis.opendocument.presentation",
    // Plain text and code
    "text/plain",
    "text/csv",
    "text/html",
    "text/css",
    "text/javascript",
    "application/json",
    "application/xml",
    "text/xml",
    "application/yaml",
    // Archives
    "application/zip",
    "application/x-rar-compressed",
    "application/x-7z-compressed",
    "application/x-tar",
    "application/gzip",
    "application/x-bzip2",
    // Other common formats
    "application/octet-stream", // Generic binary
];

#[utoipa::path(
    post,
    path = "/api/v1/images",
    responses(
    (status = 201, description = "File stored (new)", body = FileUploadResponse),
    (status = 200, description = "File already existed (idempotent)", body = FileUploadResponse),
        (status = 415, description = "Unsupported media type"),
        (status = 413, description = "Payload too large"),
    )
)]
pub async fn upload_image(
    req: HttpRequest,
    data: web::Data<AppState>,
    mut payload: Multipart,
) -> Result<HttpResponse, ApiError> {
    use actix_web::http::StatusCode;
    if let Some(rl) = &data.rate_limiter {
        let ip = extract_client_ip(&req);
    if !rl.allow_image(&ip) {
            metrics::increment_counter!("rate_limit_denied", "action" => "image_upload");
            return Err(ApiError::RateLimited { retry_after: rl.cfg.image_window.as_secs() });
        }
    metrics::increment_counter!("rate_limit_allowed", "action" => "image_upload");
    }
    let mut bytes: Vec<u8> = Vec::new();
    while let Some(field) = payload.try_next().await.map_err(|e| {
        log::error!("multipart error: {e}");
        ApiError::Internal
    })? {
        if let Some(name) = field.content_disposition().get_name() {
            if name != "file" {
                continue;
            }
        } else {
            continue;
        }
        let mut field_stream = field;
        let mut hasher = Sha256::new();
        while let Some(chunk) = field_stream.try_next().await.map_err(|e| {
            log::error!("stream read error: {e}");
            ApiError::Internal
        })? {
            if bytes.len() + chunk.len() > FILE_SIZE_LIMIT {
                return Ok(HttpResponse::build(StatusCode::PAYLOAD_TOO_LARGE).finish());
            }
            hasher.update(&chunk);
            bytes.extend_from_slice(&chunk);
        }
        let hash = format!("{:x}", hasher.finalize());
        // Infer MIME
        let mime = infer::get(&bytes)
            .map(|t| t.mime_type().to_string())
            .unwrap_or_else(|| "application/octet-stream".into());
        if !ALLOWED_MIME.contains(&mime.as_str()) {
            return Ok(HttpResponse::UnsupportedMediaType().finish());
        }
        // Attempt to persist (idempotent semantics)
        let (status_code, duplicate_flag) = match data.image_store.save(&hash, &mime, &bytes).await
        {
            Ok(()) => (actix_web::http::StatusCode::CREATED, false),
            Err(ImageStoreError::Duplicate) => (actix_web::http::StatusCode::OK, true),
            Err(e) => {
                log::error!("image_store save error: {e}");
                return Err(ApiError::Internal);
            }
        };
        let resp = FileUploadResponse {
            hash,
            mime,
            size: bytes.len(),
            duplicate: duplicate_flag,
        };
        return Ok(HttpResponse::build(status_code).json(resp));
    }
    Ok(HttpResponse::BadRequest().finish())
}

// Serve stored image / video by hash
pub async fn get_image(
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let hash = path.into_inner();
    if hash.len() < 2 {
        return Err(ApiError::NotFound);
    }
    match data.image_store.load(&hash).await {
        Ok((bytes, mime)) => Ok(HttpResponse::Ok()
            .insert_header(("Content-Type", mime))
            .body(bytes)),
        Err(ImageStoreError::NotFound) => Err(ApiError::NotFound),
        Err(e) => {
            log::error!("image_store load error: {e}");
            Err(ApiError::Internal)
        }
    }
}

// ---------------------------------------------------------------------
#[utoipa::path(
    patch,
    path = "/api/v1/boards/{id}",
    request_body = UpdateBoard,
    params(("id" = Id, Path, description = "Board id")),
    responses(
        (status = 200, description = "Board updated", body = Board),
        (status = 404, description = "Board not found"),
        (status = 409, description = "Conflict")
    )
)]
pub async fn update_board(
    auth: Auth,
    data: web::Data<AppState>,
    path: web::Path<Id>,
    payload: web::Json<UpdateBoard>,
) -> Result<HttpResponse, ApiError> {
    // ── admin-only guard ────────────────────────────────────────────
    if !auth.0.roles.iter().any(|r| matches!(r, Role::Admin)) {
        return Err(ApiError::Forbidden); // 403
    }
    // ────────────────────────────────────────────────────────────────
    let board = data
        .repo
        .update_board(path.into_inner(), payload.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(board))
}
// ---------------------------------------------------------------------

// Discord OAuth endpoints
pub async fn discord_login() -> Result<HttpResponse, ApiError> {
    // Graceful degradation: return 503 JSON if Discord OAuth isn't configured
    let client_id = match std::env::var("DISCORD_CLIENT_ID") {
        Ok(v) => v,
        Err(_) => {
            return Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "discord_oauth_not_configured",
                "message": "Set DISCORD_CLIENT_ID / DISCORD_CLIENT_SECRET to enable Discord login"
            })));
        }
    };
    // Prefer explicit env; otherwise synthesize from FRONTEND_URL replacing scheme/host
    let redirect_uri = std::env::var("DISCORD_REDIRECT_URI")
        .ok()
        .or_else(|| {
            std::env::var("FRONTEND_URL").ok().map(|f| {
                // Normalize trailing slash
                let base = f.trim_end_matches('/');
                format!("{}/api/v1/auth/discord/callback", base)
            })
        })
        .unwrap_or_else(|| "http://localhost:8080/api/v1/auth/discord/callback".to_string());

    let auth_url = format!(
        "https://discord.com/api/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope=identify",
        client_id,
        urlencoding::encode(&redirect_uri)
    );

    Ok(HttpResponse::Found()
        .insert_header(("Location", auth_url))
        .finish())
}

#[derive(serde::Deserialize)]
pub struct DiscordCallback {
    code: String,
}

#[derive(serde::Deserialize)]
struct DiscordTokenResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: String, // Keep for completeness even if unused
}

#[derive(serde::Deserialize)]
struct DiscordUser {
    id: String,
    username: String,
    #[allow(dead_code)]
    discriminator: String, // Keep for completeness even if unused
}

pub async fn discord_callback(
    query: web::Query<DiscordCallback>,
    data: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    use actix_web::http::header;

    let client_id = match std::env::var("DISCORD_CLIENT_ID") {
        Ok(v) => v,
        Err(_) => {
            return Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "discord_oauth_not_configured",
                "stage": "client_id"
            })));
        }
    };
    let client_secret = match std::env::var("DISCORD_CLIENT_SECRET") {
        Ok(v) => v,
        Err(_) => {
            return Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "discord_oauth_not_configured",
                "stage": "client_secret"
            })));
        }
    };
    let redirect_uri = std::env::var("DISCORD_REDIRECT_URI")
        .ok()
        .or_else(|| {
            std::env::var("FRONTEND_URL").ok().map(|f| {
                let base = f.trim_end_matches('/');
                format!("{}/api/v1/auth/discord/callback", base)
            })
        })
        .unwrap_or_else(|| "http://localhost:8080/api/v1/auth/discord/callback".to_string());

    // Exchange code for token
    let client = reqwest::Client::new();
    let token_response = client
        .post("https://discord.com/api/oauth2/token")
        .form(&[
            ("client_id", &client_id),
            ("client_secret", &client_secret),
            ("grant_type", &"authorization_code".to_string()),
            ("code", &query.code),
            ("redirect_uri", &redirect_uri),
        ])
        .send()
        .await
        .map_err(|_| ApiError::Internal)?
        .json::<DiscordTokenResponse>()
        .await
        .map_err(|_| ApiError::Internal)?;

    // Get user info
    let user = client
        .get("https://discord.com/api/users/@me")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", token_response.access_token),
        )
        .send()
        .await
        .map_err(|_| ApiError::Internal)?
        .json::<DiscordUser>()
        .await
        .map_err(|_| ApiError::Internal)?;

    // Look up role assignment (repo override > bootstrap env > default user)
    let bootstrap_admins = std::env::var("BOOTSTRAP_ADMIN_DISCORD_IDS").unwrap_or_default();
    let is_bootstrap_admin = bootstrap_admins
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .any(|s| s.trim() == user.id);

    let subject_key = format!("discord:{}", user.id);
    let role = data.repo.get_subject_role(&subject_key).await
        .or_else(|| if is_bootstrap_admin { Some(crate::auth::Role::Admin) } else { None })
        .unwrap_or(crate::auth::Role::User);

    // Generate JWT
    let jwt = crate::auth::create_jwt(&user.id, &user.username, vec![role])
        .map_err(|_| ApiError::Internal)?;

    // Redirect to frontend with token
    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());

    Ok(HttpResponse::Found()
        .insert_header(("Location", format!("{}/?token={}", frontend_url, jwt)))
        .finish())
}

pub async fn refresh_token(auth: Auth) -> Result<HttpResponse, ApiError> {
    let jwt = crate::auth::create_jwt(&auth.0.sub, &auth.0.sub, auth.0.roles)
        .map_err(|_| ApiError::Internal)?;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "token": jwt })))
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct SetSubjectRoleRequest { subject: String, role: String }

#[utoipa::path(
    post,
    path = "/api/v1/admin/roles",
    request_body = SetSubjectRoleRequest,
    responses(
        (status = 200, description = "Role updated"),
        (status = 403, description = "Forbidden - Admin only"),
        (status = 400, description = "Invalid role/subject")
    )
)]
pub async fn set_subject_role(
    auth: Auth,
    data: web::Data<AppState>,
    payload: web::Json<SetSubjectRoleRequest>,
) -> Result<HttpResponse, ApiError> {
    if !auth.0.roles.iter().any(|r| matches!(r, Role::Admin)) { return Err(ApiError::Forbidden); }
    let subj = payload.subject.trim();
    if subj.is_empty() || !subj.contains(':') { return Err(ApiError::BadRequest); }
    let role = match payload.role.to_lowercase().as_str() { "user"=>Role::User, "moderator"=>Role::Moderator, "admin"=>Role::Admin, _=>return Err(ApiError::BadRequest) };
    data.repo.set_subject_role(subj, role).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"message":"Role updated","subject":subj,"role":payload.role})))
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct RoleAssignment { subject: String, role: String }

#[utoipa::path(
    get,
    path = "/api/v1/admin/roles",
    responses(
        (status = 200, description = "List role assignments", body = [RoleAssignment]),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_roles(auth: Auth, data: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    if !auth.0.roles.iter().any(|r| matches!(r, Role::Admin)) { return Err(ApiError::Forbidden); }
    let rows = data.repo.list_roles().await?;
    let resp: Vec<RoleAssignment> = rows.into_iter().map(|(s,r)| RoleAssignment { subject: s, role: match r { Role::Admin=>"admin".into(), Role::Moderator=>"moderator".into(), Role::User=>"user".into() } }).collect();
    Ok(HttpResponse::Ok().json(resp))
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/roles/{subject}",
    params(("subject"=String, Path, description="Subject key to delete")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    )
)]
pub async fn delete_role(auth: Auth, data: web::Data<AppState>, path: web::Path<String>) -> Result<HttpResponse, ApiError> {
    if !auth.0.roles.iter().any(|r| matches!(r, Role::Admin)) { return Err(ApiError::Forbidden); }
    let subj = path.into_inner();
    data.repo.delete_role(&subj).await.map_err(|e| match e { crate::repo::RepoError::NotFound => ApiError::NotFound, _ => ApiError::Internal })?;
    Ok(HttpResponse::NoContent().finish())
}

#[derive(serde::Serialize)]
struct MeResponse {
    id: String,
    username: String,
    discord_id: String,
    role: String,
}

// Return authenticated user info
#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    responses(
        (status = 200, description = "Current user info", body = MeResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn auth_me(auth: Auth) -> Result<HttpResponse, ApiError> {
    // choose highest privilege (Admin > Moderator > User); claims already vetted
    let role = if auth.0.roles.iter().any(|r| matches!(r, Role::Admin)) {
        "admin"
    } else if auth.0.roles.iter().any(|r| matches!(r, Role::Moderator)) {
        "moderator"
    } else {
        "user"
    };
    let sub = &auth.0.sub;
    let (id, username, discord_id) = if let Some(rest) = sub.strip_prefix("btc:") {
        (sub.clone(), rest.to_string(), String::new())
    } else if let Some((id, u)) = sub.split_once(':') { (sub.clone(), u.to_string(), id.to_string()) } else { (sub.clone(), sub.clone(), sub.clone()) };
    let me = MeResponse { id, username, discord_id, role: role.to_string() };
    Ok(HttpResponse::Ok().json(me))
}

// Very lightweight health handler (no DB ping yet; fast fail if process unhealthy)
pub async fn health() -> Result<HttpResponse, ApiError> {
    Ok(HttpResponse::Ok().finish())
}

// (Removed bandcamp_oembed_proxy)

// ---------------- Bitcoin Proof-of-Value Auth --------------------
use std::collections::HashMap;
use std::time::{SystemTime, Duration as StdDuration};
use once_cell::sync::Lazy;
use rand::RngCore;
use tokio::sync::Mutex;

static BTC_CHALLENGES: Lazy<Mutex<HashMap<String, (String, SystemTime)>>> = Lazy::new(|| Mutex::new(HashMap::new()));
const BTC_CHALLENGE_TTL_SECS: u64 = 300; // 5 minutes
const BTC_MIN_BALANCE_SATS: u64 = 1_000_000; // 0.01 BTC

// Internal helper (used in tests) to insert a deterministic challenge for an address.
// Not exposed via HTTP, safe for production build though only called from tests.
pub async fn btc_test_insert_challenge(address: &str, challenge: &str) {
    let mut map = BTC_CHALLENGES.lock().await;
    map.insert(address.to_string(), (challenge.to_string(), SystemTime::now()));
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct BitcoinChallengeRequest { pub address: String }
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct BitcoinChallengeResponse { pub challenge: String }

#[utoipa::path(
    post,
    path = "/api/v1/auth/bitcoin/challenge",
    request_body = BitcoinChallengeRequest,
    responses(
        (status = 200, description = "Challenge issued", body = BitcoinChallengeResponse),
        (status = 400, description = "Bad request")
    )
)]
pub async fn bitcoin_challenge(payload: web::Json<BitcoinChallengeRequest>) -> Result<HttpResponse, ApiError> {
    let address = payload.address.trim();
    if address.is_empty() { return Err(ApiError::BadRequest); }
    // Basic length sanity
    if address.len() < 26 || address.len() > 100 { return Err(ApiError::BadRequest); }
    // Reject syntactically invalid addresses early
    if Address::from_str(address).is_err() { return Err(ApiError::BadRequest); }
    // ───────────────────────────────────────────────────────────────────
    // Generate 32 random bytes hex for nonce
    let mut nonce_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = hex::encode(nonce_bytes);
    let challenge = format!("Prove you own Bitcoin address {} (nonce {})", address, nonce);
    {
        let mut map = BTC_CHALLENGES.lock().await;
        map.insert(address.to_string(), (challenge.clone(), SystemTime::now()));
    }
    Ok(HttpResponse::Ok().json(BitcoinChallengeResponse { challenge }))
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct BitcoinVerifyRequest { pub address: String, pub signature: String }
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct BitcoinVerifyResponse { pub token: String }

#[utoipa::path(
    post,
    path = "/api/v1/auth/bitcoin/verify",
    request_body = BitcoinVerifyRequest,
    responses(
        (status = 200, description = "JWT token", body = BitcoinVerifyResponse),
        (status = 400, description = "Bad request"),
        (status = 403, description = "Forbidden / insufficient balance"),
        (status = 410, description = "Challenge expired")
    )
)]
pub async fn bitcoin_verify(payload: web::Json<BitcoinVerifyRequest>) -> Result<HttpResponse, ApiError> {
    use actix_web::http::StatusCode;
    // Retrieve *and* remove challenge (single-use)
    let (challenge, issued) = {
        let mut map = BTC_CHALLENGES.lock().await;
        map.remove(&payload.address).ok_or(ApiError::BadRequest)?
    };
    if issued.elapsed().unwrap_or_default() > StdDuration::from_secs(BTC_CHALLENGE_TTL_SECS) {
        return Ok(HttpResponse::build(StatusCode::GONE).finish());
    }
    // Test helpers (never set in production): granular skips instead of monolithic BTC_AUTH_TEST_ACCEPT
    let skip_balance = std::env::var("BTC_AUTH_TEST_SKIP_BALANCE").ok().map(|v| v=="1"||v=="true").unwrap_or(false);
    // Skip signature verification (used when we only want to test balance aggregation with a mock UTXO response)
    let test_skip_sig = std::env::var("BTC_AUTH_TEST_SKIP_SIG").ok().map(|v| v=="1"||v=="true").unwrap_or(false);
    // Env override for min balance
    let min_balance = std::env::var("BTC_MIN_BALANCE_SATS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(BTC_MIN_BALANCE_SATS);
    // ───────────────────────────────────────────────────────────────────
    // Signature verification (unless explicitly skipped)
    if !test_skip_sig {
        if let Err(e) = verify_bitcoin_message(&payload.address, &challenge, &payload.signature).await {
            log::warn!("bitcoin signature verify failed: {e}");
            return Err(ApiError::BadRequest);
        }
    }
    // Balance check (unless explicitly skipped)
    if !skip_balance {
        match fetch_btc_balance_sats(&payload.address).await {
            Ok(sats) if sats >= min_balance => {},
            Ok(_) => return Err(ApiError::InsufficientFunds),
            Err(_) => return Err(ApiError::Internal)
        }
    }
    // Issue JWT (User role)
    let jwt = crate::auth::create_bitcoin_jwt(&payload.address, vec![Role::User])
        .map_err(|_| ApiError::Internal)?;
    Ok(HttpResponse::Ok().json(BitcoinVerifyResponse { token: jwt }))
}

async fn verify_bitcoin_message(address: &str, message: &str, signature_b64: &str) -> anyhow::Result<()> {
    use bitcoin::{Address, Network};
    use bitcoin::address::Payload;
    use std::str::FromStr;
    use base64::Engine;
    use secp256k1::{Message as SecpMessage, Secp256k1, ecdsa::RecoverableSignature, ecdsa::RecoveryId};
    use sha2::{Sha256, Digest};

    // 1. Decode base64 signature (65 bytes: header + 64) ---------------------
    let raw = base64::engine::general_purpose::STANDARD
        .decode(signature_b64.as_bytes())
        .map_err(|e| anyhow::anyhow!(e))?;
    if raw.len() != 65 { anyhow::bail!("unexpected sig length (want 65)"); }

    let header = raw[0]; // 27..34 allowed by Core (27 + recid + (4 if compressed))
    if header < 27 || header > 34 { anyhow::bail!("invalid header byte"); }
    let rec_id = RecoveryId::from_i32(((header - 27) & 0x03) as i32)?;
    let is_compressed = ((header - 27) & 0x04) != 0;
    let sig = RecoverableSignature::from_compact(&raw[1..65], rec_id)?;

    // 2. Build Core-compatible preimage: varint(len(magic)) + magic + varint(len(msg)) + msg
    //    Magic string per Bitcoin Core: "Bitcoin Signed Message:\n"
    const MAGIC: &str = "Bitcoin Signed Message:\n"; // includes trailing newline
    fn ser_varint(n: usize) -> Vec<u8> { if n < 253 { vec![n as u8] } else { vec![253, (n & 0xff) as u8, ((n>>8)&0xff) as u8, ((n>>16)&0xff) as u8] } }
    let mut data = Vec::with_capacity(1 + MAGIC.len() + 9 + message.len());
    // magic length (always <253)
    data.push(MAGIC.len() as u8);
    data.extend_from_slice(MAGIC.as_bytes());
    // message length + message
    data.extend_from_slice(&ser_varint(message.len()));
    data.extend_from_slice(message.as_bytes());

    // Double SHA256
    let hash = {
        let d1 = Sha256::digest(&data);
        let d2 = Sha256::digest(&d1);
        d2
    };

    // 3. Recover public key
    let secp = Secp256k1::new();
    let msg = SecpMessage::from_digest_slice(&hash)?;
    let pubkey = secp.recover_ecdsa(&msg, &sig)?;
    let pk_compressed = pubkey.serialize();
    let pk_uncompressed = pubkey.serialize_uncompressed();
    let pk_bytes: &[u8] = if is_compressed { &pk_compressed } else { &pk_uncompressed };

    // 4. Compare derived address
    let addr = Address::from_str(address)?;
    match &addr.payload {
        Payload::PubkeyHash(pkh) => {
            use bitcoin::hashes::{hash160, Hash};
            let derived = hash160::Hash::hash(pk_bytes);
            let derived_bytes: &[u8] = derived.as_ref();
            let pkh_bytes: &[u8] = pkh.as_ref();
            if derived_bytes != pkh_bytes { anyhow::bail!("address mismatch"); }
        }
        Payload::WitnessProgram(wp) => {
            // Support only native segwit v0 P2WPKH (program = HASH160(compressed pubkey))
            if wp.version().to_num() != 0 || wp.program().len() != 20 { anyhow::bail!("unsupported witness program for signing"); }
            if !is_compressed { anyhow::bail!("segwit signature must use compressed key"); }
            use bitcoin::hashes::{hash160, Hash};
            let derived = hash160::Hash::hash(&pk_compressed);
            let derived_bytes: &[u8] = derived.as_ref();
            if wp.program().as_bytes() != derived_bytes { anyhow::bail!("address mismatch"); }
        }
        _ => anyhow::bail!("unsupported address type for signing"),
    }

    // 5. Network must be mainnet (adjust if you later support testnet) ------
    if addr.network != Network::Bitcoin { anyhow::bail!("wrong network"); }
    Ok(())
}

async fn fetch_btc_balance_sats(address: &str) -> anyhow::Result<u64> {
    // Test override (avoids network) ----------------------------------------
    if let Ok(v) = std::env::var("BTC_AUTH_TEST_BALANCE_OVERRIDE") {
        if let Ok(sats) = v.parse::<u64>() { return Ok(sats); }
    }
    // Blockstream API (no key) fallback to BlockCypher
    let client = reqwest::Client::new();
    // Allow overriding Blockstream base for tests (defaults to production endpoint)
    let blockstream_base = std::env::var("BTC_BLOCKSTREAM_API_BASE").unwrap_or_else(|_| "https://blockstream.info/api".to_string());
    // Try Blockstream first
    if let Ok(r) = client.get(format!("{}/address/{}/utxo", blockstream_base.trim_end_matches('/'), address)).send().await {
        if r.status().is_success() {
            let utxos: serde_json::Value = r.json().await?;
            let mut total: u64 = 0;
            if let Some(arr) = utxos.as_array() { for u in arr { if let Some(v) = u.get("value").and_then(|v| v.as_u64()) { total += v; } } }
            return Ok(total);
        }
    }
    // Fallback BlockCypher
    #[derive(serde::Deserialize)] struct BalanceResp { final_balance: u64 }
    let resp = client.get(format!("https://api.blockcypher.com/v1/btc/main/addrs/{}/balance", address)).send().await?;
    if !resp.status().is_success() { anyhow::bail!("balance api fail"); }
    let b: BalanceResp = resp.json().await?; Ok(b.final_balance)
}
// -----------------------------------------------------------------
