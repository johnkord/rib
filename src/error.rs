use actix_web::{HttpResponse, ResponseError};
use serde::Serialize;

use crate::repo::RepoError;

#[derive(Debug, Serialize)]
pub struct ApiErrorBody {
    pub error: String,
}

#[derive(thiserror::Error, Debug)]
pub enum ApiError {
    #[error("not found")] NotFound,
    #[error("conflict")] Conflict,
    #[error("internal error")] Internal,
    #[error("forbidden")] Forbidden,
    #[error("bad request")] BadRequest,
}

impl From<RepoError> for ApiError {
    fn from(e: RepoError) -> Self {
        match e {
            RepoError::NotFound  => ApiError::NotFound,
            RepoError::Conflict  => ApiError::Conflict,
        }
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ApiError::NotFound => HttpResponse::NotFound().json(ApiErrorBody { error: self.to_string() }),
            ApiError::Conflict => HttpResponse::Conflict().json(ApiErrorBody { error: self.to_string() }),
            ApiError::Internal => HttpResponse::InternalServerError().json(ApiErrorBody { error: self.to_string() }),
            ApiError::Forbidden => HttpResponse::Forbidden().json(ApiErrorBody { error: self.to_string() }),
            ApiError::BadRequest => HttpResponse::BadRequest().json(ApiErrorBody { error: self.to_string() }),
        }
    }
}
