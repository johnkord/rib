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
}

impl From<RepoError> for ApiError {
    fn from(e: RepoError) -> Self {
        match e {
            RepoError::NotFound => ApiError::NotFound,
            RepoError::Conflict => ApiError::Conflict,
            RepoError::Internal(_) => ApiError::Internal,
        }
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        use actix_web::http::StatusCode;
        let status = match self {
            ApiError::NotFound => StatusCode::NOT_FOUND,
            ApiError::Conflict => StatusCode::CONFLICT,
            ApiError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        HttpResponse::build(status).json(ApiErrorBody { error: self.to_string() })
    }
}
