use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// Use signed 64-bit when talking to Postgres (BIGINT maps to i64 in sqlx)
#[cfg(feature = "postgres-store")]
pub type Id = i64;
#[cfg(not(feature = "postgres-store"))]
pub type Id = u64;

#[cfg_attr(feature = "postgres-store", derive(sqlx::FromRow))]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Board {
    pub id: Id,
    pub slug: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
}

#[cfg_attr(feature = "postgres-store", derive(sqlx::FromRow))]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewBoard {
    pub slug: String,
    pub title: String,
}

#[cfg_attr(feature = "postgres-store", derive(sqlx::FromRow))]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Thread {
    pub id: Id,
    pub board_id: Id,
    pub subject: String,
    pub body: String,                 // NEW
    pub created_at: DateTime<Utc>,           // NEW
    pub bump_time: DateTime<Utc>,
    pub image_hash: Option<String>, // new
    pub mime:       Option<String>, // new
}

#[cfg_attr(feature = "postgres-store", derive(sqlx::FromRow))]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewThread {
    pub board_id:  Id,
    pub subject:   String,
    pub body:      String,            // NEW
    pub image_hash: Option<String>, // new
    pub mime:       Option<String>, // new
}

#[cfg_attr(feature = "postgres-store", derive(sqlx::FromRow))]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Reply {
    pub id: Id,
    pub thread_id: Id,
    pub content: String,
    pub image_hash: Option<String>, // new
    pub mime:       Option<String>, // new
    pub created_at: DateTime<Utc>,
}

#[cfg_attr(feature = "postgres-store", derive(sqlx::FromRow))]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewReply {
    pub thread_id:  Id,
    pub content:    String,
    pub image_hash: Option<String>, // new
    pub mime:       Option<String>, // new
}

// Placeholders for future features
#[cfg_attr(feature = "postgres-store", derive(sqlx::FromRow))]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Image {
    pub id: Id,
    pub thread_id: Option<Id>,
    pub reply_id: Option<Id>,
    pub hash: String,
    pub mime: String,
}

#[cfg_attr(feature = "postgres-store", derive(sqlx::FromRow))]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Report {
    pub id: Id,
    pub target_id: Id,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

#[cfg_attr(feature = "postgres-store", derive(sqlx::FromRow))]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateBoard {                     // NEW
    pub slug:  Option<String>,
    pub title: Option<String>,
}
