use std::sync::Arc;
use actix_web::{web, HttpResponse};
use actix_multipart::Multipart;
use futures_util::TryStreamExt as _;
use sha2::{Sha256, Digest};
use std::io::Write;

use crate::error::ApiError;
use crate::models::*;
use crate::repo::Repo;

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
    );
    // NEW: public fetch route (no /api/v1 prefix so <img src="/images/{hash}"> works)
    cfg.route("/images/{hash}", web::get().to(get_image));
}

#[derive(Clone)]
pub struct AppState { pub repo: Arc<dyn Repo> }

#[utoipa::path(
    get,
    path = "/api/v1/boards",
    responses(
        (status = 200, description = "List boards", body = [Board])
    )
)]
pub async fn list_boards(data: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let boards = data.repo.list_boards()?;
    Ok(HttpResponse::Ok().json(boards))
}

#[utoipa::path(
    post,
    path = "/api/v1/boards",
    request_body = NewBoard,
    responses(
        (status = 201, description = "Board created", body = Board),
        (status = 409, description = "Conflict")
    )
)]
pub async fn create_board(data: web::Data<AppState>, payload: web::Json<NewBoard>) -> Result<HttpResponse, ApiError> {
    let board = data.repo.create_board(payload.into_inner())?;
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
    let mut threads = data.repo.list_threads(path.into_inner())?;
    threads.sort_by(|a, b| b.bump_time.cmp(&a.bump_time));      // NEW
    Ok(HttpResponse::Ok().json(threads))
}

#[utoipa::path(
    post,
    path = "/api/v1/threads",
    request_body = NewThread,
    responses(
        (status = 201, description = "Thread created", body = Thread),
        (status = 404, description = "Board not found")
    )
)]
pub async fn create_thread(
    data: web::Data<AppState>,
    payload: web::Json<NewThread>,
) -> Result<HttpResponse, ApiError> {
    let thread = data.repo.create_thread(payload.into_inner())?;
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
    let th = data.repo.get_thread(path.into_inner()).map_err(|e| match e { crate::repo::RepoError::NotFound => ApiError::NotFound, _ => ApiError::Internal })?;
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
    let mut replies = data.repo.list_replies(path.into_inner())?;
    replies.sort_by(|a, b| a.created_at.cmp(&b.created_at));   // NEW
    Ok(HttpResponse::Ok().json(replies))
}

#[utoipa::path(
    post,
    path = "/api/v1/replies",
    request_body = NewReply,
    responses(
        (status = 201, description = "Reply created", body = Reply),
        (status = 404, description = "Thread not found")
    )
)]
pub async fn create_reply(data: web::Data<AppState>, payload: web::Json<NewReply>) -> Result<HttpResponse, ApiError> {
    let reply = data.repo.create_reply(payload.into_inner())?;
    Ok(HttpResponse::Created().json(reply))
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ImageUploadResponse {
    pub hash: String,
    pub mime: String,
    pub size: usize,
}

const IMAGE_SIZE_LIMIT: usize = 10 * 1024 * 1024; // 10 MB
const ALLOWED_MIME: &[&str] = &[
    "image/png", "image/jpeg", "image/gif", "image/webp",
    "video/mp4", "video/webm"                // new
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
pub async fn upload_image(mut payload: Multipart) -> Result<HttpResponse, ApiError> {
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
        let dir = format!("data/images/{}/", &hash[0..2]);
        let path = format!("{}{}", dir, hash);
        if std::path::Path::new(&path).exists() {
            // duplicate – respond with same payload instead of empty body
            let resp = ImageUploadResponse {
                hash,
                mime: mime.clone(),          // already inferred above
                size: bytes.len(),
            };
            return Ok(HttpResponse::Conflict().json(resp));
        }
        std::fs::create_dir_all(&dir).map_err(|e| { log::error!("mkdir error: {e}"); ApiError::Internal })?;
        let mut f = std::fs::File::create(&path).map_err(|e| { log::error!("file create error: {e}"); ApiError::Internal })?;
        f.write_all(&bytes).map_err(|e| { log::error!("file write error: {e}"); ApiError::Internal })?;
        let resp = ImageUploadResponse { hash, mime, size: bytes.len() };
        return Ok(HttpResponse::Created().json(resp));
    }
    Ok(HttpResponse::BadRequest().finish())
}

// NEW: serve stored image / video by hash
pub async fn get_image(path: web::Path<String>) -> Result<HttpResponse, ApiError> {
    let hash = path.into_inner();
    if hash.len() < 2 {
        return Err(ApiError::NotFound);
    }
    let file_path = format!("data/images/{}/{}", &hash[0..2], hash);
    let bytes = std::fs::read(&file_path).map_err(|_| ApiError::NotFound)?;
    let mime = infer::get(&bytes)
        .map(|t| t.mime_type())
        .unwrap_or("application/octet-stream");
    Ok(HttpResponse::Ok()
        .insert_header(("Content-Type", mime))
        .body(bytes))
}
