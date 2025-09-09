use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value; // created_by JSON metadata
use utoipa::ToSchema;

// Always Postgres backed now
pub type Id = i64;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct Board {
    pub id: Id,
    pub slug: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>, // soft delete marker
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct NewBoard {
    pub slug: String,
    pub title: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct Thread {
    pub id: Id,
    pub board_id: Id,
    pub subject: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub bump_time: DateTime<Utc>,
    pub image_hash: Option<String>,
    pub mime: Option<String>,
    pub deleted_at: Option<DateTime<Utc>>, // soft delete marker
    #[serde(skip_serializing)]
    #[schema(skip)]
    #[allow(dead_code)]
    pub created_by: Value, // internal author attribution JSON (hidden from API clients)
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct NewThread {
    pub board_id: Id,
    pub subject: String,
    pub body: String,
    pub image_hash: Option<String>,
    pub mime: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct Reply {
    pub id: Id,
    pub thread_id: Id,
    pub content: String,
    pub image_hash: Option<String>,
    pub mime: Option<String>,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>, // soft delete marker
    #[serde(skip_serializing)]
    #[schema(skip)]
    #[allow(dead_code)]
    pub created_by: Value, // internal author attribution JSON (hidden)
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct NewReply {
    pub thread_id: Id,
    pub content: String,
    pub image_hash: Option<String>,
    pub mime: Option<String>,
}
// Placeholders for future features
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct Image {
    pub id: Id,
    pub thread_id: Option<Id>,
    pub reply_id: Option<Id>,
    pub hash: String,
    pub mime: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct Report {
    pub id: Id,
    pub target_id: Id,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct UpdateBoard {
    pub slug: Option<String>,
    pub title: Option<String>,
}
