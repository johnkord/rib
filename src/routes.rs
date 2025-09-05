use std::sync::Arc;
use actix_web::{web, HttpResponse};
use actix_multipart::Multipart;
use futures_util::TryStreamExt as _;
use sha2::{Sha256, Digest};

use crate::error::ApiError;
use crate::models::*;
use crate::repo::Repo;
use crate::storage::{ImageStore, ImageStoreError};
use crate::auth::{Auth, Role};

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .service(
                web::resource("/boards")
                    .route(web::get().to(list_boards))
                    .route(web::post().to(create_board)),
            )
            .service(web::resource("/boards/{id}/threads").route(web::get().to(list_threads)))
            .service(
                web::resource("/threads")
                    .route(web::post().to(create_thread)),
            )
            .service(web::resource("/threads/{id}").route(web::get().to(get_thread)))
            .service(web::resource("/threads/{id}/replies").route(web::get().to(list_replies)))
            .service(
                web::resource("/replies")
                    .route(web::post().to(create_reply)),
            )
            .service(
                web::resource("/images")
                    .route(web::post().to(upload_image)),
            )
            .service(
                web::resource("/boards/{id}")                    // NEW
                    .route(web::patch().to(update_board)),
            )
            .service(
                web::resource("/auth/discord/callback")
                    .route(web::get().to(discord_callback)),
            )
            .service(
                web::resource("/auth/discord/login")
                    .route(web::get().to(discord_login)),
            )
            .service(
                web::resource("/auth/refresh")
                    .route(web::post().to(refresh_token)),
            )
            .service(
                web::resource("/admin/discord-roles")
                    .route(web::post().to(set_discord_role)),
            )
            .service(
                web::resource("/auth/me")                 // NEW
                    .route(web::get().to(auth_me)),
            )
    );
    // NEW: public fetch route (no /api/v1 prefix so <img src="/images/{hash}"> works)
    cfg.route("/images/{hash}", web::get().to(get_image));
}

#[derive(Clone)]
pub struct AppState { pub repo: Arc<dyn Repo>, pub image_store: Arc<dyn ImageStore> }

#[utoipa::path(
    get,
    path = "/api/v1/boards",
    responses(
        (status = 200, description = "List boards", body = [Board])
    )
)]
pub async fn list_boards(data: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let boards = data.repo.list_boards().await?;
    Ok(HttpResponse::Ok().json(boards))
}

#[utoipa::path(
    post,
    path = "/api/v1/boards",
    request_body = NewBoard,
    responses(
        (status = 201, description = "Board created", body = Board),
        (status = 403, description = "Forbidden – Admins only"),   // UPDATED
        (status = 409, description = "Conflict")
    )
)]
pub async fn create_board(
    auth: Auth,                          // NEW – require JWT
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
        ("id" = Id, Path, description = "Board id")
    ),
    responses(
        (status = 200, description = "List threads", body = [Thread]),
        (status = 404, description = "Board not found")
    )
)]
pub async fn list_threads(data: web::Data<AppState>, path: web::Path<Id>) -> Result<HttpResponse, ApiError> {
    let mut threads = data.repo.list_threads(path.into_inner()).await?;
    threads.sort_by(|a, b| b.bump_time.cmp(&a.bump_time));      // NEW
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
    auth: Auth,  // NEW - require authentication
    data: web::Data<AppState>,
    payload: web::Json<NewThread>,
) -> Result<HttpResponse, ApiError> {
    // Check for any valid role (User, Moderator, or Admin)
    if !auth.0.roles.iter().any(|r| matches!(r, Role::User | Role::Moderator | Role::Admin)) {
        return Err(ApiError::Forbidden);
    }
    
    let thread = data.repo.create_thread(payload.into_inner()).await?;
    Ok(HttpResponse::Created().json(thread))
}

#[utoipa::path(
    get,
    path = "/api/v1/threads/{id}",
    params(("id" = Id, Path, description = "Thread id")),
    responses(
        (status = 200, description = "Thread", body = Thread),
        (status = 404, description = "Thread not found")
    )
)]
pub async fn get_thread(data: web::Data<AppState>, path: web::Path<Id>) -> Result<HttpResponse, ApiError> {
    let th = data.repo.get_thread(path.into_inner()).await.map_err(|e| match e { crate::repo::RepoError::NotFound => ApiError::NotFound, _ => ApiError::Internal })?;
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
pub async fn list_replies(data: web::Data<AppState>, path: web::Path<Id>) -> Result<HttpResponse, ApiError> {
    let mut replies = data.repo.list_replies(path.into_inner()).await?;
    replies.sort_by(|a, b| a.created_at.cmp(&b.created_at));   // NEW
    Ok(HttpResponse::Ok().json(replies))
}

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
    auth: Auth,  // NEW - require authentication
    data: web::Data<AppState>,
    payload: web::Json<NewReply>
) -> Result<HttpResponse, ApiError> {
    // Check for any valid role (User, Moderator, or Admin)
    if !auth.0.roles.iter().any(|r| matches!(r, Role::User | Role::Moderator | Role::Admin)) {
        return Err(ApiError::Forbidden);
    }
    
    let reply = data.repo.create_reply(payload.into_inner()).await?;
    Ok(HttpResponse::Created().json(reply))
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ImageUploadResponse {
    pub hash: String,
    pub mime: String,
    pub size: usize,
}

const IMAGE_SIZE_LIMIT: usize = 10 * 1024 * 1024; // 10 MB

const ALLOWED_MIME: &[&str] = &[
    "image/png", "image/jpeg", "image/gif", "image/webp",
    "video/mp4", "video/webm"
];

#[utoipa::path(
    post,
    path = "/api/v1/images",
    responses(
        (status = 201, description = "Image stored", body = ImageUploadResponse),
        (status = 409, description = "Duplicate image"),
        (status = 415, description = "Unsupported media type"),
        (status = 413, description = "Payload too large"),
    )
)]
pub async fn upload_image(data: web::Data<AppState>, mut payload: Multipart) -> Result<HttpResponse, ApiError> {
    use actix_web::http::StatusCode;
    let mut bytes: Vec<u8> = Vec::new();
    while let Some(field) = payload.try_next().await.map_err(|e| {
        log::error!("multipart error: {e}");
        ApiError::Internal
    })? {
        if let Some(name) = field.content_disposition().get_name() {
            if name != "file" { continue; }
        } else { continue; }
        let mut field_stream = field;
        let mut hasher = Sha256::new();
        while let Some(chunk) = field_stream.try_next().await.map_err(|e| {
            log::error!("stream read error: {e}");
            ApiError::Internal
        })? {
            if bytes.len() + chunk.len() > IMAGE_SIZE_LIMIT { return Ok(HttpResponse::build(StatusCode::PAYLOAD_TOO_LARGE).finish()); }
            hasher.update(&chunk);
            bytes.extend_from_slice(&chunk);
        }
        let hash = format!("{:x}", hasher.finalize());
        // Infer MIME
        let mime = infer::get(&bytes).map(|t| t.mime_type().to_string()).unwrap_or_else(|| "application/octet-stream".into());
        if !ALLOWED_MIME.contains(&mime.as_str()) {
            return Ok(HttpResponse::UnsupportedMediaType().finish());
        }
        match data.image_store.save(&hash, &mime, &bytes).await {
            Ok(()) => {},
            Err(ImageStoreError::Duplicate) => return Err(ApiError::Conflict),
            Err(e) => { log::error!("image_store save error: {e}"); return Err(ApiError::Internal); }
        }
         let resp = ImageUploadResponse { hash, mime, size: bytes.len() };
         return Ok(HttpResponse::Created().json(resp));
    }
    Ok(HttpResponse::BadRequest().finish())
}

// NEW: serve stored image / video by hash
pub async fn get_image(data: web::Data<AppState>, path: web::Path<String>) -> Result<HttpResponse, ApiError> {
    let hash = path.into_inner();
    if hash.len() < 2 { return Err(ApiError::NotFound); }
    match data.image_store.load(&hash).await {
        Ok((bytes, mime)) => Ok(HttpResponse::Ok().insert_header(("Content-Type", mime)).body(bytes)),
        Err(ImageStoreError::NotFound) => Err(ApiError::NotFound),
        Err(e) => { log::error!("image_store load error: {e}"); Err(ApiError::Internal) }
    }
}

// NEW -----------------------------------------------------------------
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
    auth: Auth,                       // NEW – JWT claims extractor
    data: web::Data<AppState>,
    path: web::Path<Id>,
    payload: web::Json<UpdateBoard>,
) -> Result<HttpResponse, ApiError> {
    // ── admin-only guard ────────────────────────────────────────────
    if !auth.0.roles.iter().any(|r| matches!(r, Role::Admin)) {
        return Err(ApiError::Forbidden); // 403
    }
    // ────────────────────────────────────────────────────────────────
    let board =
        data.repo.update_board(path.into_inner(), payload.into_inner()).await?;
    Ok(HttpResponse::Ok().json(board))
}
// ---------------------------------------------------------------------

// Discord OAuth endpoints
pub async fn discord_login() -> Result<HttpResponse, ApiError> {
    // Graceful degradation: return 503 JSON if Discord OAuth isn't configured
    let client_id = match std::env::var("DISCORD_CLIENT_ID") {
        Ok(v) => v,
        Err(_) => {
            return Ok(HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({
                    "error": "discord_oauth_not_configured",
                    "message": "Set DISCORD_CLIENT_ID / DISCORD_CLIENT_SECRET to enable Discord login"
                })));
        }
    };
    let redirect_uri = std::env::var("DISCORD_REDIRECT_URI")
        .unwrap_or_else(|_| "http://localhost:8080/api/v1/auth/discord/callback".to_string());

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
    token_type: String,  // Keep for completeness even if unused
}

#[derive(serde::Deserialize)]
struct DiscordUser {
    id: String,
    username: String,
    #[allow(dead_code)]
    discriminator: String,  // Keep for completeness even if unused
}

pub async fn discord_callback(
    query: web::Query<DiscordCallback>,
    data: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    use actix_web::http::header;
    
    let client_id = match std::env::var("DISCORD_CLIENT_ID") { Ok(v) => v, Err(_) => {
        return Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "discord_oauth_not_configured",
            "stage": "client_id"
        })));
    }};
    let client_secret = match std::env::var("DISCORD_CLIENT_SECRET") { Ok(v) => v, Err(_) => {
        return Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "discord_oauth_not_configured",
            "stage": "client_secret"
        })));
    }};
    let redirect_uri = std::env::var("DISCORD_REDIRECT_URI")
        .unwrap_or_else(|_| "http://localhost:8080/api/v1/auth/discord/callback".to_string());
    
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
        .header(header::AUTHORIZATION, format!("Bearer {}", token_response.access_token))
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

    let role = data.repo
        .get_discord_user_role(&user.id).await
        .or_else(|| if is_bootstrap_admin { Some(crate::auth::Role::Admin) } else { None })
        .unwrap_or(crate::auth::Role::User);

    // Generate JWT
    let jwt = crate::auth::create_jwt(&user.id, &user.username, vec![role])
        .map_err(|_| ApiError::Internal)?;
    
    // Redirect to frontend with token
    let frontend_url = std::env::var("FRONTEND_URL")
        .unwrap_or_else(|_| "http://localhost:5173".to_string());
    
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
pub struct SetDiscordRoleRequest {
    discord_id: String,
    role: String,  // "user", "moderator", or "admin"
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/discord-roles",
    request_body = SetDiscordRoleRequest,
    responses(
        (status = 200, description = "Role updated"),
        (status = 403, description = "Forbidden - Admin only"),
        (status = 400, description = "Invalid role")
    )
)]
pub async fn set_discord_role(
    auth: Auth,
    data: web::Data<AppState>,
    payload: web::Json<SetDiscordRoleRequest>,
) -> Result<HttpResponse, ApiError> {
    // Admin-only endpoint
    if !auth.0.roles.iter().any(|r| matches!(r, Role::Admin)) {
        return Err(ApiError::Forbidden);
    }
    
    // Parse role string to enum
    let role = match payload.role.to_lowercase().as_str() {
        "user" => Role::User,
        "moderator" => Role::Moderator,
        "admin" => Role::Admin,
        _ => return Err(ApiError::BadRequest),
    };
    
    // Set the role in the repository
    data.repo.set_discord_user_role(&payload.discord_id, role).await?;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Role updated successfully",
        "discord_id": payload.discord_id,
        "role": payload.role
    })))
}

#[derive(serde::Serialize)]
struct MeResponse {
    id: String,
    username: String,
    discord_id: String,
    role: String,
}

// NEW: return authenticated user info
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
    let me = MeResponse {
        id: auth.0.sub.clone(),
        username: auth.0.sub.clone(),      // username not persisted; fallback to discord id
        discord_id: auth.0.sub.clone(),
        role: role.to_string(),
    };
    Ok(HttpResponse::Ok().json(me))
}
