use actix_multipart::Multipart;
use actix_web::{web, HttpResponse};
use bitcoin::Address;
use futures_util::TryStreamExt as _;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use std::sync::Arc;

use crate::auth::{
    clear_oauth_transaction_cookie, clear_session_cookie, consume_oauth_transaction,
    create_oauth_transaction, session_cookie, Auth, Role, OAUTH_TRANSACTION_COOKIE_NAME,
};
use crate::error::ApiError;
use crate::models::*;
use crate::repo::Repo;
use crate::storage::{is_valid_content_hash, ImageStore, ImageStoreError};
use actix_web::HttpRequest;

fn trusted_forwarded_ip(value: &str, trusted_hops: usize) -> Option<String> {
    let addresses: Vec<std::net::IpAddr> = value
        .split(',')
        .filter_map(|item| item.trim().parse().ok())
        .collect();
    let index = addresses.len().checked_sub(trusted_hops + 1)?;
    Some(addresses[index].to_string())
}

// Forwarded headers are security-sensitive and ignored unless the deployment
// explicitly declares how many downstream proxy entries it trusts.
fn extract_client_ip(req: &HttpRequest) -> String {
    let trust_proxy_headers = std::env::var("TRUST_PROXY_HEADERS")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if trust_proxy_headers {
        let trusted_hops = std::env::var("TRUSTED_PROXY_HOPS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        if let Some(address) = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| trusted_forwarded_ip(value, trusted_hops))
        {
            return address;
        }
    }
    if let Some(peer) = req.peer_addr() {
        return peer.ip().to_string();
    }
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
            .service(web::resource("/boards/{id}").route(web::patch().to(update_board)))
            .service(web::resource("/auth/discord/callback").route(web::get().to(discord_callback)))
            .service(web::resource("/auth/discord/login").route(web::get().to(discord_login)))
            .service(
                web::resource("/auth/bitcoin/challenge").route(web::post().to(bitcoin_challenge)),
            )
            .service(web::resource("/auth/bitcoin/verify").route(web::post().to(bitcoin_verify)))
            .service(web::resource("/auth/refresh").route(web::post().to(refresh_token)))
            .service(web::resource("/auth/logout").route(web::post().to(logout)))
            .service(
                web::resource("/admin/roles")
                    .route(web::post().to(set_subject_role))
                    .route(web::get().to(list_roles)),
            )
            .service(web::resource("/admin/roles/{subject}").route(web::delete().to(delete_role)))
            .service(
                web::resource("/admin/bans")
                    .route(web::post().to(create_subject_ban))
                    .route(web::get().to(list_subject_bans)),
            )
            .service(
                web::resource("/admin/bans/{subject}").route(web::delete().to(delete_subject_ban)),
            )
            .service(
                web::resource("/admin/threads/{id}/author").route(web::get().to(get_thread_author)),
            )
            .service(
                web::resource("/admin/replies/{id}/author").route(web::get().to(get_reply_author)),
            )
            .service(web::resource("/auth/me").route(web::get().to(auth_me)))
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
            ),
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
    let mut new = payload.into_inner();
    new.slug = new.slug.trim().to_string();
    new.title = new.title.trim().to_string();
    validate_board_fields(&new.slug, &new.title)?;
    let board = data.repo.create_board(new).await?;
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
    threads.sort_by_key(|thread| std::cmp::Reverse(thread.bump_time));
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
    let (subject_key, created_by) = private_author_attribution(&auth)?;
    ensure_subject_can_post(data.get_ref(), &auth, &subject_key).await?;
    if let Some(rl) = &data.rate_limiter {
        let ip = extract_client_ip(&req);
        if !rl.allow_thread(&ip) {
            metrics::increment_counter!("rate_limit_denied", "action" => "thread_create");
            return Err(ApiError::RateLimited {
                retry_after: rl.cfg.thread_window.as_secs(),
            });
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
    let mut new = payload.into_inner();
    new.subject = new.subject.trim().to_string();
    new.body = new.body.trim().to_string();
    validate_thread_payload(&new)?;
    let board = data
        .repo
        .get_board(new.board_id)
        .await
        .map_err(|_| ApiError::NotFound)?;
    if board.deleted_at.is_some() {
        return Err(ApiError::NotFound);
    }
    let public_identity =
        derive_public_identity(new.author_name.take(), new.tripcode_password.take())?;
    let thread = data
        .repo
        .create_thread(new, created_by, public_identity)
        .await?;
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
    let board = data.repo.get_board(th.board_id).await?;
    if board.deleted_at.is_some() && !(is_admin && want_deleted) {
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
    let board = data.repo.get_board(thread.board_id).await?;
    if board.deleted_at.is_some() && !(is_admin && want_deleted) {
        return Err(ApiError::NotFound);
    }
    let mut replies = data
        .repo
        .list_replies(thread_id, is_admin && want_deleted)
        .await?;
    replies.sort_by_key(|reply| reply.created_at);
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

fn private_author_attribution(auth: &Auth) -> Result<(String, serde_json::Value), ApiError> {
    let subject = role_subject_key(&auth.0.sub).ok_or(ApiError::Forbidden)?;
    let details = if let Some(address) = auth.0.sub.strip_prefix("btc:") {
        serde_json::json!({
            "v": 1,
            "subject": subject,
            "provider": "bitcoin",
            "address": address,
        })
    } else {
        let (discord_id, username) = auth.0.sub.split_once(':').ok_or(ApiError::Forbidden)?;
        serde_json::json!({
            "v": 1,
            "subject": subject,
            "provider": "discord",
            "discord_id": discord_id,
            "username": username,
        })
    };
    Ok((subject, details))
}

fn validate_board_fields(slug: &str, title: &str) -> Result<(), ApiError> {
    let valid_slug = !slug.is_empty()
        && slug.len() <= 64
        && slug.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte)
        });
    if !valid_slug || title.is_empty() || title.chars().count() > 100 {
        return Err(ApiError::BadRequest);
    }
    Ok(())
}

fn validate_attachment(image_hash: &Option<String>, mime: &Option<String>) -> Result<(), ApiError> {
    match (image_hash, mime) {
        (None, None) => Ok(()),
        (Some(hash), Some(mime))
            if is_valid_content_hash(hash) && ALLOWED_MIME.contains(&mime.as_str()) =>
        {
            Ok(())
        }
        _ => Err(ApiError::BadRequest),
    }
}

fn validate_thread_payload(new: &NewThread) -> Result<(), ApiError> {
    if new.subject.is_empty()
        || new.subject.chars().count() > 200
        || new.body.chars().count() > 2000
    {
        return Err(ApiError::BadRequest);
    }
    validate_attachment(&new.image_hash, &new.mime)
}

fn validate_reply_payload(new: &NewReply) -> Result<(), ApiError> {
    if new.content.chars().count() > 2000 || (new.content.is_empty() && new.image_hash.is_none()) {
        return Err(ApiError::BadRequest);
    }
    validate_attachment(&new.image_hash, &new.mime)
}

fn is_valid_subject_key(subject: &str) -> bool {
    let Some((provider, identifier)) = subject.split_once(':') else {
        return false;
    };
    matches!(provider, "discord" | "btc")
        && !identifier.is_empty()
        && !identifier.contains(':')
        && identifier.chars().count() <= 128
}

fn derive_public_identity(
    author_name: Option<String>,
    tripcode_password: Option<String>,
) -> Result<PublicIdentity, ApiError> {
    let author_name = author_name
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty());
    if author_name
        .as_ref()
        .is_some_and(|name| name.chars().count() > 40)
    {
        return Err(ApiError::BadRequest);
    }

    let tripcode = match tripcode_password {
        Some(password) if (4..=128).contains(&password.chars().count()) => {
            let secret = std::env::var("TRIPCODE_SECRET")
                .or_else(|_| {
                    if cfg!(debug_assertions) {
                        std::env::var("JWT_SECRET")
                    } else {
                        Err(std::env::VarError::NotPresent)
                    }
                })
                .map_err(|_| ApiError::Internal)?;
            let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
                .map_err(|_| ApiError::Internal)?;
            mac.update(b"rib-tripcode-v1\0");
            mac.update(password.as_bytes());
            let digest = mac.finalize().into_bytes();
            Some(format!("!{}", hex::encode(&digest[..6])))
        }
        Some(_) => return Err(ApiError::BadRequest),
        None => None,
    };

    Ok(PublicIdentity {
        author_name,
        tripcode,
    })
}

async fn ensure_subject_not_banned(data: &AppState, subject: &str) -> Result<(), ApiError> {
    if data.repo.is_subject_banned(subject).await? {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

async fn ensure_subject_can_post(
    data: &AppState,
    auth: &Auth,
    subject: &str,
) -> Result<(), ApiError> {
    ensure_subject_not_banned(data, subject).await?;
    if !auth.0.sub.starts_with("btc:") {
        let (discord_id, _) = auth.0.sub.split_once(':').ok_or(ApiError::Forbidden)?;
        let assigned_role = data.repo.get_subject_role(subject).await;
        if discord_admission_role(assigned_role, is_bootstrap_discord_id(discord_id)).is_none() {
            return Err(ApiError::Forbidden);
        }
    }
    Ok(())
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct AuthorAttribution {
    subject: String,
    details: serde_json::Value,
}

fn author_attribution(details: serde_json::Value) -> Result<AuthorAttribution, ApiError> {
    let subject = details
        .get("subject")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .or_else(
            || match details.get("provider").and_then(serde_json::Value::as_str) {
                Some("discord") => details
                    .get("discord_id")
                    .and_then(serde_json::Value::as_str)
                    .map(|id| format!("discord:{id}")),
                Some("bitcoin") => details
                    .get("address")
                    .and_then(serde_json::Value::as_str)
                    .map(|address| format!("btc:{address}")),
                _ => None,
            },
        )
        .ok_or(ApiError::NotFound)?;
    Ok(AuthorAttribution { subject, details })
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/threads/{id}/author",
    params(("id" = Id, Path, description = "Thread id")),
    responses(
        (status = 200, description = "Private author attribution", body = AuthorAttribution),
        (status = 403, description = "Moderator role required"),
        (status = 404, description = "Thread or attribution not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_thread_author(
    auth: Auth,
    data: web::Data<AppState>,
    path: web::Path<Id>,
) -> Result<HttpResponse, ApiError> {
    ensure_moderator_or_admin!(auth);
    let thread = data.repo.get_thread(path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(author_attribution(thread.created_by)?))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/replies/{id}/author",
    params(("id" = Id, Path, description = "Reply id")),
    responses(
        (status = 200, description = "Private author attribution", body = AuthorAttribution),
        (status = 403, description = "Moderator role required"),
        (status = 404, description = "Reply or attribution not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_reply_author(
    auth: Auth,
    data: web::Data<AppState>,
    path: web::Path<Id>,
) -> Result<HttpResponse, ApiError> {
    ensure_moderator_or_admin!(auth);
    let reply = data.repo.get_reply(path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(author_attribution(reply.created_by)?))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/bans",
    request_body = NewSubjectBan,
    responses(
        (status = 201, description = "Subject banned", body = SubjectBan),
        (status = 400, description = "Invalid subject or reason"),
        (status = 403, description = "Moderator role required")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_subject_ban(
    auth: Auth,
    data: web::Data<AppState>,
    payload: web::Json<NewSubjectBan>,
) -> Result<HttpResponse, ApiError> {
    ensure_moderator_or_admin!(auth);
    let mut new = payload.into_inner();
    new.subject = new.subject.trim().to_string();
    new.reason = new.reason.trim().to_string();
    if !is_valid_subject_key(&new.subject)
        || new.reason.is_empty()
        || new.reason.chars().count() > 500
    {
        return Err(ApiError::BadRequest);
    }
    let ban = data.repo.create_subject_ban(new, &auth.0.sub).await?;
    Ok(HttpResponse::Created().json(ban))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/bans",
    responses(
        (status = 200, description = "Active subject bans", body = [SubjectBan]),
        (status = 403, description = "Moderator role required")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_subject_bans(
    auth: Auth,
    data: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    ensure_moderator_or_admin!(auth);
    Ok(HttpResponse::Ok().json(data.repo.list_subject_bans().await?))
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/bans/{subject}",
    params(("subject" = String, Path, description = "Provider subject key")),
    responses(
        (status = 204, description = "Ban removed"),
        (status = 403, description = "Moderator role required"),
        (status = 404, description = "Ban not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_subject_ban(
    auth: Auth,
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    ensure_moderator_or_admin!(auth);
    data.repo.delete_subject_ban(&path.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
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
    let id = path.into_inner();
    let hashes = data.repo.list_board_image_hashes(id).await?;
    data.repo.hard_delete_board(id).await?;
    delete_unreferenced_images(data.get_ref(), hashes).await?;
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
    let id = path.into_inner();
    let hashes = data.repo.list_thread_image_hashes(id).await?;
    data.repo.hard_delete_thread(id).await?;
    delete_unreferenced_images(data.get_ref(), hashes).await?;
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
            delete_unreferenced_images(data.get_ref(), vec![hash]).await?;
        }
    }
    Ok(HttpResponse::NoContent().finish())
}

async fn delete_unreferenced_images(data: &AppState, hashes: Vec<String>) -> Result<(), ApiError> {
    let unique_hashes: std::collections::HashSet<String> = hashes.into_iter().collect();
    for hash in unique_hashes {
        if !data.repo.is_image_referenced(&hash).await? {
            if let Err(error) = data.image_store.delete(&hash).await {
                log::error!("failed to delete unreferenced image {hash}: {error}");
            }
        }
    }
    Ok(())
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
    let (subject_key, created_by) = private_author_attribution(&auth)?;
    ensure_subject_can_post(data.get_ref(), &auth, &subject_key).await?;
    if let Some(rl) = &data.rate_limiter {
        let ip = extract_client_ip(&req);
        if !rl.allow_reply(&ip) {
            metrics::increment_counter!("rate_limit_denied", "action" => "reply_create");
            return Err(ApiError::RateLimited {
                retry_after: rl.cfg.reply_window.as_secs(),
            });
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
    let mut new = payload.into_inner();
    new.content = new.content.trim().to_string();
    validate_reply_payload(&new)?;
    let thread = data
        .repo
        .get_thread(new.thread_id)
        .await
        .map_err(|_| ApiError::NotFound)?;
    if thread.deleted_at.is_some() {
        return Err(ApiError::NotFound);
    }
    let public_identity =
        derive_public_identity(new.author_name.take(), new.tripcode_password.take())?;
    let reply = data
        .repo
        .create_reply(new, created_by, public_identity)
        .await?;
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

fn detect_upload_mime(bytes: &[u8]) -> String {
    if let Some(kind) = infer::get(bytes) {
        return kind.mime_type().to_string();
    }
    if std::str::from_utf8(bytes).is_ok() && !bytes.contains(&0) {
        return "text/plain".to_string();
    }
    "application/octet-stream".to_string()
}

fn is_inline_preview_mime(mime: &str) -> bool {
    (mime.starts_with("image/") && mime != "image/svg+xml")
        || mime.starts_with("video/")
        || mime.starts_with("audio/")
}

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
    auth: Auth,
    req: HttpRequest,
    data: web::Data<AppState>,
    mut payload: Multipart,
) -> Result<HttpResponse, ApiError> {
    use actix_web::http::StatusCode;
    let subject_key = role_subject_key(&auth.0.sub).ok_or(ApiError::Forbidden)?;
    ensure_subject_can_post(data.get_ref(), &auth, &subject_key).await?;
    if let Some(rl) = &data.rate_limiter {
        let ip = extract_client_ip(&req);
        if !rl.allow_image(&ip) {
            metrics::increment_counter!("rate_limit_denied", "action" => "image_upload");
            return Err(ApiError::RateLimited {
                retry_after: rl.cfg.image_window.as_secs(),
            });
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
        let mime = detect_upload_mime(&bytes);
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
    req: HttpRequest,
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let hash = path.into_inner();
    if !is_valid_content_hash(&hash) {
        return Err(ApiError::NotFound);
    }
    let etag = format!("\"{hash}\"");
    if req
        .headers()
        .get(actix_web::http::header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        == Some(etag.as_str())
    {
        return Ok(HttpResponse::NotModified().finish());
    }
    match data.image_store.load(&hash).await {
        Ok((bytes, mime)) => {
            let mut response = HttpResponse::Ok();
            response
                .insert_header(("Content-Type", mime.as_str()))
                .insert_header(("ETag", etag))
                .insert_header(("Cache-Control", "public, max-age=31536000, immutable"));
            if !is_inline_preview_mime(&mime) {
                response.insert_header((
                    "Content-Disposition",
                    format!("attachment; filename=\"{hash}\""),
                ));
            }
            Ok(response.body(bytes))
        }
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
    let mut update = payload.into_inner();
    update.slug = update.slug.map(|slug| slug.trim().to_string());
    update.title = update.title.map(|title| title.trim().to_string());
    if update.slug.as_ref().is_some_and(|slug| {
        validate_board_fields(slug, update.title.as_deref().unwrap_or("x")).is_err()
    }) || update
        .title
        .as_ref()
        .is_some_and(|title| title.is_empty() || title.chars().count() > 100)
    {
        return Err(ApiError::BadRequest);
    }
    let board = data.repo.update_board(path.into_inner(), update).await?;
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

    let transaction = create_oauth_transaction().map_err(|_| ApiError::Internal)?;
    let auth_url = format!(
        "https://discord.com/api/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope=identify&state={}&code_challenge={}&code_challenge_method=S256",
        client_id,
        urlencoding::encode(&redirect_uri),
        transaction.state,
        transaction.code_challenge,
    );

    Ok(HttpResponse::Found()
        .insert_header(("Location", auth_url))
        .cookie(transaction.cookie)
        .finish())
}

#[derive(serde::Deserialize)]
pub struct DiscordCallback {
    code: String,
    state: String,
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

fn discord_admission_role(assigned_role: Option<Role>, is_bootstrap_admin: bool) -> Option<Role> {
    if is_bootstrap_admin {
        Some(Role::Admin)
    } else {
        assigned_role
    }
}

pub async fn discord_callback(
    req: HttpRequest,
    query: web::Query<DiscordCallback>,
    data: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
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

    let transaction_cookie = req
        .cookie(OAUTH_TRANSACTION_COOKIE_NAME)
        .ok_or(ApiError::BadRequest)?;
    let pkce_verifier = consume_oauth_transaction(transaction_cookie.value(), &query.state)
        .map_err(|_| ApiError::BadRequest)?;

    // Exchange code for token
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| ApiError::Internal)?;
    let token_http_response = client
        .post("https://discord.com/api/oauth2/token")
        .form(&[
            ("client_id", &client_id),
            ("client_secret", &client_secret),
            ("grant_type", &"authorization_code".to_string()),
            ("code", &query.code),
            ("redirect_uri", &redirect_uri),
            ("code_verifier", &pkce_verifier),
        ])
        .send()
        .await
        .map_err(|_| ApiError::Internal)?;
    if !token_http_response.status().is_success() {
        log::warn!(
            "Discord token exchange failed with status {}",
            token_http_response.status()
        );
        return Err(ApiError::BadRequest);
    }
    let token_response = token_http_response
        .json::<DiscordTokenResponse>()
        .await
        .map_err(|_| ApiError::Internal)?;

    // Get user info
    let user_http_response = client
        .get("https://discord.com/api/users/@me")
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", token_response.access_token),
        )
        .send()
        .await
        .map_err(|_| ApiError::Internal)?;
    if !user_http_response.status().is_success() {
        log::warn!(
            "Discord user lookup failed with status {}",
            user_http_response.status()
        );
        return Err(ApiError::BadRequest);
    }
    let user = user_http_response
        .json::<DiscordUser>()
        .await
        .map_err(|_| ApiError::Internal)?;

    // Only explicitly assigned Discord subjects may post. Bootstrap admins are
    // the recovery path when no role assignment is available.
    let bootstrap_admins = std::env::var("BOOTSTRAP_ADMIN_DISCORD_IDS").unwrap_or_default();
    let is_bootstrap_admin = bootstrap_admins
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .any(|s| s.trim() == user.id);

    let subject_key = format!("discord:{}", user.id);
    let assigned_role = data.repo.get_subject_role(&subject_key).await;
    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());
    let Some(role) = discord_admission_role(assigned_role, is_bootstrap_admin) else {
        return Ok(HttpResponse::Found()
            .insert_header((
                "Location",
                format!(
                    "{}/login?error=discord_not_allowlisted",
                    frontend_url.trim_end_matches('/')
                ),
            ))
            .cookie(clear_oauth_transaction_cookie())
            .finish());
    };

    // Generate JWT
    let jwt = crate::auth::create_jwt(&user.id, &user.username, vec![role])
        .map_err(|_| ApiError::Internal)?;

    // Redirect to frontend with token
    Ok(HttpResponse::Found()
        .insert_header((
            "Location",
            format!("{}/", frontend_url.trim_end_matches('/')),
        ))
        .cookie(session_cookie(&jwt))
        .cookie(clear_oauth_transaction_cookie())
        .finish())
}

fn role_subject_key(jwt_subject: &str) -> Option<String> {
    if jwt_subject.starts_with("btc:") {
        Some(jwt_subject.to_string())
    } else {
        jwt_subject
            .split_once(':')
            .map(|(discord_id, _)| format!("discord:{discord_id}"))
    }
}

fn is_bootstrap_discord_id(discord_id: &str) -> bool {
    std::env::var("BOOTSTRAP_ADMIN_DISCORD_IDS")
        .unwrap_or_default()
        .split(',')
        .filter(|value| !value.trim().is_empty())
        .any(|value| value.trim() == discord_id)
}

pub async fn refresh_token(
    auth: Auth,
    data: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let subject_key = role_subject_key(&auth.0.sub).ok_or(ApiError::Forbidden)?;
    ensure_subject_not_banned(data.get_ref(), &subject_key).await?;
    let assigned_role = data.repo.get_subject_role(&subject_key).await;
    let role = if let Some(address) = auth.0.sub.strip_prefix("btc:") {
        let _ = address;
        assigned_role.unwrap_or(Role::User)
    } else {
        let (discord_id, _) = auth.0.sub.split_once(':').ok_or(ApiError::Forbidden)?;
        discord_admission_role(assigned_role, is_bootstrap_discord_id(discord_id))
            .ok_or(ApiError::Forbidden)?
    };
    let jwt = crate::auth::create_jwt(&auth.0.sub, &auth.0.sub, vec![role])
        .map_err(|_| ApiError::Internal)?;

    Ok(HttpResponse::Ok()
        .cookie(session_cookie(&jwt))
        .json(serde_json::json!({ "token": jwt })))
}

pub async fn logout() -> Result<HttpResponse, ApiError> {
    Ok(HttpResponse::NoContent()
        .cookie(clear_session_cookie())
        .finish())
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct SetSubjectRoleRequest {
    subject: String,
    role: String,
}

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
    if !auth.0.roles.iter().any(|r| matches!(r, Role::Admin)) {
        return Err(ApiError::Forbidden);
    }
    let subj = payload.subject.trim();
    if !is_valid_subject_key(subj) {
        return Err(ApiError::BadRequest);
    }
    let role = match payload.role.to_lowercase().as_str() {
        "user" => Role::User,
        "moderator" => Role::Moderator,
        "admin" => Role::Admin,
        _ => return Err(ApiError::BadRequest),
    };
    data.repo.set_subject_role(subj, role).await?;
    Ok(HttpResponse::Ok()
        .json(serde_json::json!({"message":"Role updated","subject":subj,"role":payload.role})))
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct RoleAssignment {
    subject: String,
    role: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/roles",
    responses(
        (status = 200, description = "List role assignments", body = [RoleAssignment]),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_roles(auth: Auth, data: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    if !auth.0.roles.iter().any(|r| matches!(r, Role::Admin)) {
        return Err(ApiError::Forbidden);
    }
    let rows = data.repo.list_roles().await?;
    let resp: Vec<RoleAssignment> = rows
        .into_iter()
        .map(|(s, r)| RoleAssignment {
            subject: s,
            role: match r {
                Role::Admin => "admin".into(),
                Role::Moderator => "moderator".into(),
                Role::User => "user".into(),
            },
        })
        .collect();
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
pub async fn delete_role(
    auth: Auth,
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    if !auth.0.roles.iter().any(|r| matches!(r, Role::Admin)) {
        return Err(ApiError::Forbidden);
    }
    let subj = path.into_inner();
    data.repo.delete_role(&subj).await.map_err(|e| match e {
        crate::repo::RepoError::NotFound => ApiError::NotFound,
        _ => ApiError::Internal,
    })?;
    Ok(HttpResponse::NoContent().finish())
}

#[derive(serde::Serialize, utoipa::ToSchema)]
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
        (status = 200, description = "Current user info or null when anonymous", body = Option<MeResponse>)
    )
)]
pub async fn auth_me(auth: Option<Auth>) -> Result<HttpResponse, ApiError> {
    let Some(auth) = auth else {
        return Ok(HttpResponse::Ok().json(Option::<MeResponse>::None));
    };
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
    } else if let Some((id, u)) = sub.split_once(':') {
        (sub.clone(), u.to_string(), id.to_string())
    } else {
        (sub.clone(), sub.clone(), sub.clone())
    };
    let me = MeResponse {
        id,
        username,
        discord_id,
        role: role.to_string(),
    };
    Ok(HttpResponse::Ok().json(me))
}

// Very lightweight health handler (no DB ping yet; fast fail if process unhealthy)
pub async fn health() -> Result<HttpResponse, ApiError> {
    Ok(HttpResponse::Ok().finish())
}

// (Removed bandcamp_oembed_proxy)

// ---------------- Bitcoin Proof-of-Value Auth --------------------
use once_cell::sync::Lazy;
use rand::RngCore;
use std::collections::HashMap;
use std::time::{Duration as StdDuration, SystemTime};
use tokio::sync::Mutex;

static BTC_CHALLENGES: Lazy<Mutex<HashMap<String, (String, SystemTime)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
const BTC_CHALLENGE_TTL_SECS: u64 = 300; // 5 minutes
const BTC_MIN_BALANCE_SATS: u64 = 1_000_000; // 0.01 BTC

// Internal helper (used in tests) to insert a deterministic challenge for an address.
// Not exposed via HTTP, safe for production build though only called from tests.
pub async fn btc_test_insert_challenge(address: &str, challenge: &str) {
    let mut map = BTC_CHALLENGES.lock().await;
    map.insert(
        address.to_string(),
        (challenge.to_string(), SystemTime::now()),
    );
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct BitcoinChallengeRequest {
    pub address: String,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct BitcoinChallengeResponse {
    pub challenge: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/bitcoin/challenge",
    request_body = BitcoinChallengeRequest,
    responses(
        (status = 200, description = "Challenge issued", body = BitcoinChallengeResponse),
        (status = 400, description = "Bad request")
    )
)]
pub async fn bitcoin_challenge(
    payload: web::Json<BitcoinChallengeRequest>,
) -> Result<HttpResponse, ApiError> {
    let address = payload.address.trim();
    if address.is_empty() {
        return Err(ApiError::BadRequest);
    }
    // Basic length sanity
    if address.len() < 26 || address.len() > 100 {
        return Err(ApiError::BadRequest);
    }
    // Reject syntactically invalid addresses early
    if Address::from_str(address).is_err() {
        return Err(ApiError::BadRequest);
    }
    // ───────────────────────────────────────────────────────────────────
    // Generate 32 random bytes hex for nonce
    let mut nonce_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = hex::encode(nonce_bytes);
    let challenge = format!(
        "Prove you own Bitcoin address {} (nonce {})",
        address, nonce
    );
    {
        let mut map = BTC_CHALLENGES.lock().await;
        map.retain(|_, (_, issued)| {
            issued.elapsed().unwrap_or_default() <= StdDuration::from_secs(BTC_CHALLENGE_TTL_SECS)
        });
        map.insert(address.to_string(), (challenge.clone(), SystemTime::now()));
    }
    Ok(HttpResponse::Ok().json(BitcoinChallengeResponse { challenge }))
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct BitcoinVerifyRequest {
    pub address: String,
    pub signature: String,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct BitcoinVerifyResponse {
    pub token: String,
}

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
pub async fn bitcoin_verify(
    payload: web::Json<BitcoinVerifyRequest>,
) -> Result<HttpResponse, ApiError> {
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
    let skip_balance = debug_test_flag("BTC_AUTH_TEST_SKIP_BALANCE");
    // Skip signature verification (used when we only want to test balance aggregation with a mock UTXO response)
    let test_skip_sig = debug_test_flag("BTC_AUTH_TEST_SKIP_SIG");
    // Env override for min balance
    let min_balance = std::env::var("BTC_MIN_BALANCE_SATS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(BTC_MIN_BALANCE_SATS);
    // ───────────────────────────────────────────────────────────────────
    // Signature verification (unless explicitly skipped)
    if !test_skip_sig {
        if let Err(e) =
            verify_bitcoin_message(&payload.address, &challenge, &payload.signature).await
        {
            log::warn!("bitcoin signature verify failed: {e}");
            return Err(ApiError::BadRequest);
        }
    }
    // Balance check (unless explicitly skipped)
    if !skip_balance {
        match fetch_btc_balance_sats(&payload.address).await {
            Ok(sats) if sats >= min_balance => {}
            Ok(_) => return Err(ApiError::InsufficientFunds),
            Err(_) => return Err(ApiError::Internal),
        }
    }
    // Issue JWT (User role)
    let jwt = crate::auth::create_bitcoin_jwt(&payload.address, vec![Role::User])
        .map_err(|_| ApiError::Internal)?;
    Ok(HttpResponse::Ok()
        .cookie(session_cookie(&jwt))
        .json(BitcoinVerifyResponse { token: jwt }))
}

async fn verify_bitcoin_message(
    address: &str,
    message: &str,
    signature_b64: &str,
) -> anyhow::Result<()> {
    use base64::Engine;
    use bitcoin::address::Payload;
    use bitcoin::{Address, Network};
    use secp256k1::{
        ecdsa::RecoverableSignature, ecdsa::RecoveryId, Message as SecpMessage, Secp256k1,
    };
    use sha2::{Digest, Sha256};
    use std::str::FromStr;

    // 1. Decode base64 signature (65 bytes: header + 64) ---------------------
    let raw = base64::engine::general_purpose::STANDARD
        .decode(signature_b64.as_bytes())
        .map_err(|e| anyhow::anyhow!(e))?;
    if raw.len() != 65 {
        anyhow::bail!("unexpected sig length (want 65)");
    }

    let header = raw[0]; // 27..34 allowed by Core (27 + recid + (4 if compressed))
    if !(27..=34).contains(&header) {
        anyhow::bail!("invalid header byte");
    }
    let rec_id = RecoveryId::from_i32(((header - 27) & 0x03) as i32)?;
    let is_compressed = ((header - 27) & 0x04) != 0;
    let sig = RecoverableSignature::from_compact(&raw[1..65], rec_id)?;

    // 2. Build Core-compatible preimage: varint(len(magic)) + magic + varint(len(msg)) + msg
    //    Magic string per Bitcoin Core: "Bitcoin Signed Message:\n"
    const MAGIC: &str = "Bitcoin Signed Message:\n"; // includes trailing newline
    fn ser_varint(n: usize) -> Vec<u8> {
        if n < 253 {
            vec![n as u8]
        } else {
            vec![
                253,
                (n & 0xff) as u8,
                ((n >> 8) & 0xff) as u8,
                ((n >> 16) & 0xff) as u8,
            ]
        }
    }
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
        Sha256::digest(d1)
    };

    // 3. Recover public key
    let secp = Secp256k1::new();
    let msg = SecpMessage::from_digest_slice(&hash)?;
    let pubkey = secp.recover_ecdsa(&msg, &sig)?;
    let pk_compressed = pubkey.serialize();
    let pk_uncompressed = pubkey.serialize_uncompressed();
    let pk_bytes: &[u8] = if is_compressed {
        &pk_compressed
    } else {
        &pk_uncompressed
    };

    // 4. Compare derived address
    let addr = Address::from_str(address)?;
    match &addr.payload {
        Payload::PubkeyHash(pkh) => {
            use bitcoin::hashes::{hash160, Hash};
            let derived = hash160::Hash::hash(pk_bytes);
            let derived_bytes: &[u8] = derived.as_ref();
            let pkh_bytes: &[u8] = pkh.as_ref();
            if derived_bytes != pkh_bytes {
                anyhow::bail!("address mismatch");
            }
        }
        Payload::WitnessProgram(wp) => {
            // Support only native segwit v0 P2WPKH (program = HASH160(compressed pubkey))
            if wp.version().to_num() != 0 || wp.program().len() != 20 {
                anyhow::bail!("unsupported witness program for signing");
            }
            if !is_compressed {
                anyhow::bail!("segwit signature must use compressed key");
            }
            use bitcoin::hashes::{hash160, Hash};
            let derived = hash160::Hash::hash(&pk_compressed);
            let derived_bytes: &[u8] = derived.as_ref();
            if wp.program().as_bytes() != derived_bytes {
                anyhow::bail!("address mismatch");
            }
        }
        _ => anyhow::bail!("unsupported address type for signing"),
    }

    // 5. Network must be mainnet (adjust if you later support testnet) ------
    if addr.network != Network::Bitcoin {
        anyhow::bail!("wrong network");
    }
    Ok(())
}

async fn fetch_btc_balance_sats(address: &str) -> anyhow::Result<u64> {
    // Test override (avoids network) ----------------------------------------
    if let Some(sats) = debug_balance_override() {
        return Ok(sats);
    }
    // Blockstream API (no key) fallback to BlockCypher
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    // Allow overriding Blockstream base for tests (defaults to production endpoint)
    let blockstream_base = std::env::var("BTC_BLOCKSTREAM_API_BASE")
        .unwrap_or_else(|_| "https://blockstream.info/api".to_string());
    // Try Blockstream first
    if let Ok(r) = client
        .get(format!(
            "{}/address/{}/utxo",
            blockstream_base.trim_end_matches('/'),
            address
        ))
        .send()
        .await
    {
        if r.status().is_success() {
            let utxos: serde_json::Value = r.json().await?;
            let mut total: u64 = 0;
            if let Some(arr) = utxos.as_array() {
                for utxo in arr {
                    let confirmed = utxo
                        .get("status")
                        .and_then(|status| status.get("confirmed"))
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false);
                    if confirmed {
                        if let Some(value) = utxo.get("value").and_then(serde_json::Value::as_u64) {
                            total = total.saturating_add(value);
                        }
                    }
                }
            }
            return Ok(total);
        }
    }
    // Fallback BlockCypher
    #[derive(serde::Deserialize)]
    struct BalanceResp {
        balance: u64,
    }
    let resp = client
        .get(format!(
            "https://api.blockcypher.com/v1/btc/main/addrs/{}/balance",
            address
        ))
        .send()
        .await?;
    if !resp.status().is_success() {
        anyhow::bail!("balance api fail");
    }
    let b: BalanceResp = resp.json().await?;
    Ok(b.balance)
}
// -----------------------------------------------------------------

#[cfg(debug_assertions)]
fn debug_test_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[cfg(not(debug_assertions))]
fn debug_test_flag(_name: &str) -> bool {
    false
}

#[cfg(debug_assertions)]
fn debug_balance_override() -> Option<u64> {
    std::env::var("BTC_AUTH_TEST_BALANCE_OVERRIDE")
        .ok()
        .and_then(|value| value.parse().ok())
}

#[cfg(not(debug_assertions))]
fn debug_balance_override() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::{
        derive_public_identity, detect_upload_mime, discord_admission_role, is_inline_preview_mime,
        is_valid_subject_key, role_subject_key, trusted_forwarded_ip, validate_board_fields,
        validate_reply_payload, validate_thread_payload,
    };
    use crate::auth::Role;
    use crate::models::{NewReply, NewThread};
    use crate::storage::is_valid_content_hash;

    #[test]
    fn discord_admission_rejects_unassigned_subjects() {
        assert_eq!(discord_admission_role(None, false), None);
    }

    #[test]
    fn discord_admission_uses_explicit_assignment() {
        assert_eq!(
            discord_admission_role(Some(Role::Moderator), false),
            Some(Role::Moderator)
        );
    }

    #[test]
    fn discord_admission_bootstrap_admin_overrides_assignment() {
        assert_eq!(
            discord_admission_role(Some(Role::User), true),
            Some(Role::Admin)
        );
    }

    #[test]
    fn role_subject_key_normalizes_auth_providers() {
        assert_eq!(
            role_subject_key("1234:alice"),
            Some("discord:1234".to_string())
        );
        assert_eq!(
            role_subject_key("btc:bc1qexample"),
            Some("btc:bc1qexample".to_string())
        );
        assert_eq!(role_subject_key("invalid"), None);
    }

    #[test]
    fn content_hash_validation_rejects_non_hex_and_unicode() {
        assert!(is_valid_content_hash(&"a".repeat(64)));
        assert!(!is_valid_content_hash(&"A".repeat(64)));
        assert!(!is_valid_content_hash("aé"));
        assert!(!is_valid_content_hash("short"));
    }

    #[test]
    fn upload_mime_detection_recognizes_plain_text() {
        assert_eq!(detect_upload_mime(b"hello world"), "text/plain");
        assert_eq!(
            detect_upload_mime(&[0, 159, 146, 150]),
            "application/octet-stream"
        );
    }

    #[test]
    fn only_passive_media_is_previewed_inline() {
        assert!(is_inline_preview_mime("image/png"));
        assert!(is_inline_preview_mime("video/mp4"));
        assert!(!is_inline_preview_mime("image/svg+xml"));
        assert!(!is_inline_preview_mime("text/html"));
        assert!(!is_inline_preview_mime("application/pdf"));
    }

    #[test]
    fn tripcodes_are_stable_and_passwords_are_not_returned() {
        std::env::set_var(
            "TRIPCODE_SECRET",
            "tripcode-test-secret-abcdefghijklmnopqrstuvwxyz",
        );
        let first = derive_public_identity(Some(" Alice ".to_string()), Some("secret".to_string()))
            .expect("identity");
        let second = derive_public_identity(None, Some("secret".to_string())).expect("identity");
        let different =
            derive_public_identity(None, Some("different".to_string())).expect("identity");

        assert_eq!(first.author_name.as_deref(), Some("Alice"));
        assert_eq!(first.tripcode, second.tripcode);
        assert_ne!(first.tripcode, different.tripcode);
        assert!(first
            .tripcode
            .as_deref()
            .is_some_and(|tripcode| tripcode.starts_with('!') && tripcode.len() == 13));
    }

    #[test]
    fn tripcodes_validate_name_and_password_lengths() {
        assert!(derive_public_identity(None, Some("abc".to_string())).is_err());
        assert!(derive_public_identity(Some("a".repeat(41)), None).is_err());
    }

    #[test]
    fn content_payload_validation_enforces_lengths_and_attachment_pairs() {
        let valid_thread = NewThread {
            board_id: 1,
            subject: "subject".to_string(),
            body: "body".to_string(),
            image_hash: None,
            mime: None,
            author_name: None,
            tripcode_password: None,
        };
        assert!(validate_thread_payload(&valid_thread).is_ok());
        assert!(validate_thread_payload(&NewThread {
            image_hash: Some("a".repeat(64)),
            ..valid_thread.clone()
        })
        .is_err());

        let valid_reply = NewReply {
            thread_id: 1,
            content: "reply".to_string(),
            image_hash: None,
            mime: None,
            author_name: None,
            tripcode_password: None,
        };
        assert!(validate_reply_payload(&valid_reply).is_ok());
        assert!(validate_reply_payload(&NewReply {
            content: String::new(),
            ..valid_reply
        })
        .is_err());
    }

    #[test]
    fn board_and_subject_keys_use_canonical_formats() {
        assert!(validate_board_fields("tech-news", "Tech News").is_ok());
        assert!(validate_board_fields("Bad Slug", "Title").is_err());
        assert!(is_valid_subject_key("discord:123456"));
        assert!(is_valid_subject_key("btc:bc1qexample"));
        assert!(!is_valid_subject_key("discord:"));
        assert!(!is_valid_subject_key("other:value"));
    }

    #[test]
    fn forwarded_ip_uses_validated_chain_from_the_right() {
        assert_eq!(
            trusted_forwarded_ip("198.51.100.10, 10.0.0.4", 1).as_deref(),
            Some("198.51.100.10")
        );
        assert_eq!(
            trusted_forwarded_ip("spoofed, 198.51.100.10", 0).as_deref(),
            Some("198.51.100.10")
        );
        assert_eq!(trusted_forwarded_ip("spoofed", 0), None);
    }
}
