use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// Always Postgres backed now
pub type Id = i64;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct Board {
    pub id: Id,
    pub slug: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>, // NEW soft delete marker
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
    pub body: String,                 // NEW
    pub created_at: DateTime<Utc>,           // NEW
    pub bump_time: DateTime<Utc>,
    pub image_hash: Option<String>, // new
    pub mime:       Option<String>, // new
    pub deleted_at: Option<DateTime<Utc>>, // NEW soft delete marker
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct NewThread {
    pub board_id:  Id,
    pub subject:   String,
    pub body:      String,            // NEW
    pub image_hash: Option<String>, // new
    pub mime:       Option<String>, // new
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct Reply {
    pub id: Id,
    pub thread_id: Id,
    pub content: String,
    pub image_hash: Option<String>, // new
    pub mime:       Option<String>, // new
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>, // NEW soft delete marker
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct NewReply {
    pub thread_id:  Id,
    pub content:    String,
    pub image_hash: Option<String>, // new
    pub mime:       Option<String>, // new
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
pub struct UpdateBoard {                     // NEW
    pub slug:  Option<String>,
    pub title: Option<String>,
}
