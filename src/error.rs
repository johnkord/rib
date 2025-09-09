use actix_web::{HttpResponse, ResponseError};
use serde::Serialize;

use crate::repo::RepoError;

#[derive(Debug, Serialize)]
pub struct ApiErrorBody {
    pub error: String,
}

#[derive(thiserror::Error, Debug)]
pub enum ApiError {
    #[error("not found")]
    NotFound,
    #[error("conflict")]
    Conflict,
    #[error("internal error")]
    Internal,
    #[error("forbidden")]
    Forbidden,
    #[error("insufficient funds")]
    InsufficientFunds,
    #[error("bad request")]
    BadRequest,
    #[error("rate limited")]
    RateLimited { retry_after: u64 },
}

impl From<RepoError> for ApiError {
    fn from(e: RepoError) -> Self {
        match e {
            RepoError::NotFound => ApiError::NotFound,
            RepoError::Conflict => ApiError::Conflict,
        }
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        let mut builder = match self {
            ApiError::NotFound => HttpResponse::NotFound(),
            ApiError::Conflict => HttpResponse::Conflict(),
            ApiError::Internal => HttpResponse::InternalServerError(),
            ApiError::Forbidden => HttpResponse::Forbidden(),
            ApiError::InsufficientFunds => HttpResponse::Forbidden(),
            ApiError::BadRequest => HttpResponse::BadRequest(),
            ApiError::RateLimited { retry_after } => {
                let mut b = HttpResponse::TooManyRequests();
                b.insert_header(("Retry-After", retry_after.to_string()));
                b
            }
        };
        builder.json(ApiErrorBody { error: self.to_string() })
    }
}
