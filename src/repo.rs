use crate::auth::Role as AuthRole;
use crate::models::*;
use serde_json::Value;

#[derive(thiserror::Error, Debug)]
pub enum RepoError {
    #[error("not found")]
    NotFound,
    #[error("conflict")]
    Conflict,
}

pub type RepoResult<T> = Result<T, RepoError>;

use async_trait::async_trait;

#[async_trait]
pub trait BoardRepo: Send + Sync {
    async fn list_boards(&self, include_deleted: bool) -> RepoResult<Vec<Board>>;
    async fn create_board(&self, new: NewBoard) -> RepoResult<Board>;
    async fn update_board(&self, id: Id, upd: UpdateBoard) -> RepoResult<Board>;
    async fn soft_delete_board(&self, id: Id) -> RepoResult<()>;
    async fn restore_board(&self, id: Id) -> RepoResult<()>;
    async fn hard_delete_board(&self, id: Id) -> RepoResult<()>;
    async fn get_board(&self, id: Id) -> RepoResult<Board>;
}

#[async_trait]
pub trait ThreadRepo: Send + Sync {
    async fn list_threads(&self, board_id: Id, include_deleted: bool) -> RepoResult<Vec<Thread>>;
    async fn create_thread(&self, new: NewThread, created_by: Value) -> RepoResult<Thread>; // created_by now supplied by caller (JSON)
    async fn get_thread(&self, id: Id) -> RepoResult<Thread>;
    async fn soft_delete_thread(&self, id: Id) -> RepoResult<()>;
    async fn restore_thread(&self, id: Id) -> RepoResult<()>;
    async fn hard_delete_thread(&self, id: Id) -> RepoResult<()>;
}

#[async_trait]
pub trait ReplyRepo: Send + Sync {
    async fn list_replies(&self, thread_id: Id, include_deleted: bool) -> RepoResult<Vec<Reply>>;
    async fn create_reply(&self, new: NewReply, created_by: Value) -> RepoResult<Reply>; // created_by now supplied by caller (JSON)
    async fn soft_delete_reply(&self, id: Id) -> RepoResult<()>;
    async fn restore_reply(&self, id: Id) -> RepoResult<()>;
    async fn hard_delete_reply(&self, id: Id) -> RepoResult<()>;
    async fn get_reply(&self, id: Id) -> RepoResult<Reply>;
}

#[async_trait]
pub trait RoleRepo: Send + Sync {
    async fn get_subject_role(&self, subject: &str) -> Option<AuthRole>;
    async fn set_subject_role(&self, subject: &str, role: AuthRole) -> RepoResult<()>;
    async fn list_roles(&self) -> RepoResult<Vec<(String, AuthRole)>>;
    async fn delete_role(&self, subject: &str) -> RepoResult<()>;
}

pub trait Repo: BoardRepo + ThreadRepo + ReplyRepo + RoleRepo {}

impl<T> Repo for T where T: BoardRepo + ThreadRepo + ReplyRepo + RoleRepo {}

// Postgres implementation (now the only backend)
pub mod pg {
    use super::*;
    use sqlx::{Pool, Postgres, Row}; // Row is new

    #[derive(Clone)]
    pub struct PgRepo {
        pool: Pool<Postgres>,
    }

    impl PgRepo {
        pub fn new(pool: Pool<Postgres>) -> Self {
            Self { pool }
        }
    }

    #[async_trait]
    impl BoardRepo for PgRepo {
        async fn list_boards(&self, include_deleted: bool) -> RepoResult<Vec<Board>> {
            let sql = if include_deleted {
                "SELECT id, slug, title, created_at, deleted_at FROM boards ORDER BY id"
            } else {
                "SELECT id, slug, title, created_at, deleted_at FROM boards WHERE deleted_at IS NULL ORDER BY id"
            };
            let recs = sqlx::query_as::<_, Board>(sql)
                .fetch_all(&self.pool)
                .await
                .map_err(|_| RepoError::NotFound)?;
            Ok(recs)
        }
        async fn create_board(&self, new: NewBoard) -> RepoResult<Board> {
            let rec = sqlx::query_as::<_, Board>("INSERT INTO boards (slug, title) VALUES ($1,$2) RETURNING id, slug, title, created_at, deleted_at")
                .bind(&new.slug).bind(&new.title)
                .fetch_one(&self.pool).await.map_err(|_| RepoError::Conflict)?;
            Ok(rec)
        }
        async fn update_board(&self, id: Id, upd: UpdateBoard) -> RepoResult<Board> {
            // Build dynamic SQL (simple field subset)
            let mut slug = None;
            let mut title = None;
            if let Some(s) = upd.slug {
                slug = Some(s);
            }
            if let Some(t) = upd.title {
                title = Some(t);
            }
            let rec = sqlx::query_as::<_, Board>(
                "UPDATE boards SET slug = COALESCE($2, slug), title = COALESCE($3, title) WHERE id=$1 RETURNING id, slug, title, created_at, deleted_at"
            )
            .bind(id)
            .bind(slug.as_ref())
            .bind(title.as_ref())
            .fetch_one(&self.pool).await.map_err(|_| RepoError::NotFound)?;
            Ok(rec)
        }
        async fn get_board(&self, id: Id) -> RepoResult<Board> {
            let rec = sqlx::query_as::<_, Board>(
                "SELECT id, slug, title, created_at, deleted_at FROM boards WHERE id=$1",
            )
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| RepoError::NotFound)?;
            Ok(rec)
        }
        async fn soft_delete_board(&self, id: Id) -> RepoResult<()> {
            let res = sqlx::query(
                "UPDATE boards SET deleted_at = COALESCE(deleted_at, now()) WHERE id=$1",
            )
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|_| RepoError::NotFound)?;
            if res.rows_affected() == 0 {
                return Err(RepoError::NotFound);
            }
            Ok(())
        }
        async fn restore_board(&self, id: Id) -> RepoResult<()> {
            let res = sqlx::query("UPDATE boards SET deleted_at = NULL WHERE id=$1")
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|_| RepoError::NotFound)?;
            if res.rows_affected() == 0 {
                return Err(RepoError::NotFound);
            }
            Ok(())
        }
        async fn hard_delete_board(&self, id: Id) -> RepoResult<()> {
            let res = sqlx::query("DELETE FROM boards WHERE id=$1")
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|_| RepoError::NotFound)?;
            if res.rows_affected() == 0 {
                return Err(RepoError::NotFound);
            }
            Ok(())
        }
    }

    #[async_trait]
    impl ThreadRepo for PgRepo {
        async fn list_threads(
            &self,
            board_id: Id,
            include_deleted: bool,
        ) -> RepoResult<Vec<Thread>> {
            let base = r#"
          SELECT t.id, t.board_id, t.subject, t.body, t.created_at, t.bump_time, t.created_by,
              img.hash as image_hash, img.mime as mime, t.deleted_at
                FROM threads t
                LEFT JOIN LATERAL (
                   SELECT i.hash, i.mime FROM images i
                   WHERE i.thread_id = t.id
                   ORDER BY i.id ASC LIMIT 1
                ) img ON TRUE
                WHERE t.board_id = $1
            "#;
            let sql = if include_deleted {
                format!("{base} ORDER BY t.bump_time DESC")
            } else {
                format!("{base} AND t.deleted_at IS NULL ORDER BY t.bump_time DESC")
            };
            let recs = sqlx::query_as::<_, Thread>(&sql)
                .bind(board_id)
                .fetch_all(&self.pool)
                .await
                .map_err(|_| RepoError::NotFound)?;
            Ok(recs)
        }
        async fn create_thread(&self, new: NewThread, created_by: Value) -> RepoResult<Thread> {
            let mut tx = self.pool.begin().await.map_err(|_| RepoError::Conflict)?;

            // insert thread and capture its id
            let rec = sqlx::query(
                "INSERT INTO threads (board_id, subject, body, created_by) VALUES ($1,$2,$3,$4) RETURNING id"
            )
                .bind(new.board_id)
                .bind(&new.subject)
                .bind(&new.body)
                .bind(&created_by)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| RepoError::NotFound)?;
            let thread_id: Id = rec.get::<Id, _>("id");

            if let (Some(hash), Some(mime)) = (new.image_hash.as_ref(), new.mime.as_ref()) {
                let _ = sqlx::query(
                    "INSERT INTO images (thread_id, reply_id, hash, mime) VALUES ($1, NULL, $2, $3) ON CONFLICT (hash) DO NOTHING"
                )
                    .bind(thread_id)
                    .bind(hash)
                    .bind(mime)
                    .execute(&mut *tx)
                    .await;
            }

            tx.commit().await.map_err(|_| RepoError::Conflict)?;

            // fetch and return full thread record
            let thread = sqlx::query_as::<_, Thread>(
                r#"
          SELECT t.id, t.board_id, t.subject, t.body, t.created_at, t.bump_time, t.created_by,
              img.hash as image_hash, img.mime as mime, t.deleted_at
                FROM threads t
                LEFT JOIN LATERAL (
                    SELECT i.hash, i.mime
                    FROM images i
                    WHERE i.thread_id = t.id
                    ORDER BY i.id ASC
                    LIMIT 1
                ) img ON TRUE
                WHERE t.id = $1
            "#,
            )
            .bind(thread_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| RepoError::NotFound)?;

            Ok(thread)
        }
        async fn get_thread(&self, id: Id) -> RepoResult<Thread> {
            let thread = sqlx::query_as::<_, Thread>(r#"
          SELECT t.id, t.board_id, t.subject, t.body, t.created_at, t.bump_time, t.created_by,
              img.hash as image_hash, img.mime as mime, t.deleted_at
                FROM threads t
                LEFT JOIN LATERAL (
                   SELECT i.hash, i.mime FROM images i WHERE i.thread_id = t.id ORDER BY i.id ASC LIMIT 1
                ) img ON TRUE
                WHERE t.id = $1
            "#).bind(id).fetch_one(&self.pool).await.map_err(|_| RepoError::NotFound)?;
            Ok(thread)
        }
        async fn soft_delete_thread(&self, id: Id) -> RepoResult<()> {
            let res = sqlx::query(
                "UPDATE threads SET deleted_at = COALESCE(deleted_at, now()) WHERE id=$1",
            )
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|_| RepoError::NotFound)?;
            if res.rows_affected() == 0 {
                return Err(RepoError::NotFound);
            }
            Ok(())
        }
        async fn restore_thread(&self, id: Id) -> RepoResult<()> {
            let res = sqlx::query("UPDATE threads SET deleted_at = NULL WHERE id=$1")
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|_| RepoError::NotFound)?;
            if res.rows_affected() == 0 {
                return Err(RepoError::NotFound);
            }
            Ok(())
        }
        async fn hard_delete_thread(&self, id: Id) -> RepoResult<()> {
            let res = sqlx::query("DELETE FROM threads WHERE id=$1")
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|_| RepoError::NotFound)?;
            if res.rows_affected() == 0 {
                return Err(RepoError::NotFound);
            }
            Ok(())
        }
    }

    #[async_trait]
    impl ReplyRepo for PgRepo {
        async fn list_replies(
            &self,
            thread_id: Id,
            include_deleted: bool,
        ) -> RepoResult<Vec<Reply>> {
            let base = r#"
                SELECT r.id, r.thread_id, r.content, img.hash as image_hash, img.mime as mime, r.created_at, r.deleted_at, r.created_by
                FROM replies r
                LEFT JOIN LATERAL (
                   SELECT i.hash, i.mime FROM images i WHERE i.reply_id = r.id ORDER BY i.id ASC LIMIT 1
                ) img ON TRUE
                WHERE r.thread_id = $1
            "#;
            let sql = if include_deleted {
                format!("{base} ORDER BY r.created_at ASC")
            } else {
                format!("{base} AND r.deleted_at IS NULL ORDER BY r.created_at ASC")
            };
            let recs = sqlx::query_as::<_, Reply>(&sql)
                .bind(thread_id)
                .fetch_all(&self.pool)
                .await
                .map_err(|_| RepoError::NotFound)?;
            Ok(recs)
        }
        async fn create_reply(&self, new: NewReply, created_by: Value) -> RepoResult<Reply> {
            let mut tx = self.pool.begin().await.map_err(|_| RepoError::Conflict)?;

            let rec = sqlx::query(
                "INSERT INTO replies (thread_id, content, created_by) VALUES ($1,$2,$3) RETURNING id"
            )
                .bind(new.thread_id)
                .bind(&new.content)
                .bind(&created_by)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| RepoError::NotFound)?;
            let reply_id: Id = rec.get::<Id, _>("id");

            if let (Some(hash), Some(mime)) = (new.image_hash.as_ref(), new.mime.as_ref()) {
                let _ = sqlx::query(
                    "INSERT INTO images (thread_id, reply_id, hash, mime) VALUES (NULL, $1, $2, $3) ON CONFLICT (hash) DO NOTHING"
                )
                    .bind(reply_id)
                    .bind(hash)
                    .bind(mime)
                    .execute(&mut *tx)
                    .await;
            }

            // bump parent thread
            let _ = sqlx::query("UPDATE threads SET bump_time = now() WHERE id=$1")
                .bind(new.thread_id)
                .execute(&mut *tx)
                .await;

            tx.commit().await.map_err(|_| RepoError::Conflict)?;

            // fetch and return full reply record
            let reply = sqlx::query_as::<_, Reply>(
                r#"
          SELECT r.id, r.thread_id, r.content,
              img.hash as image_hash, img.mime as mime,
              r.created_at, r.deleted_at, r.created_by
                FROM replies r
                LEFT JOIN LATERAL (
                    SELECT i.hash, i.mime
                    FROM images i
                    WHERE i.reply_id = r.id
                    ORDER BY i.id ASC
                    LIMIT 1
                ) img ON TRUE
                WHERE r.id = $1
            "#,
            )
            .bind(reply_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| RepoError::NotFound)?;

            Ok(reply)
        }
        async fn soft_delete_reply(&self, id: Id) -> RepoResult<()> {
            let res = sqlx::query(
                "UPDATE replies SET deleted_at = COALESCE(deleted_at, now()) WHERE id=$1",
            )
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|_| RepoError::NotFound)?;
            if res.rows_affected() == 0 {
                return Err(RepoError::NotFound);
            }
            Ok(())
        }
        async fn restore_reply(&self, id: Id) -> RepoResult<()> {
            let res = sqlx::query("UPDATE replies SET deleted_at = NULL WHERE id=$1")
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|_| RepoError::NotFound)?;
            if res.rows_affected() == 0 {
                return Err(RepoError::NotFound);
            }
            Ok(())
        }
        async fn hard_delete_reply(&self, id: Id) -> RepoResult<()> {
            // Need to also detach reply's image (image row cascades ON DELETE, but we want to allow external store cleanup)
            let res = sqlx::query("DELETE FROM replies WHERE id=$1")
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|_| RepoError::NotFound)?;
            if res.rows_affected() == 0 {
                return Err(RepoError::NotFound);
            }
            Ok(())
        }
        async fn get_reply(&self, id: Id) -> RepoResult<Reply> {
            let rec = sqlx::query_as::<_, Reply>(
                r#"
          SELECT r.id, r.thread_id, r.content,
              img.hash as image_hash, img.mime as mime,
              r.created_at, r.deleted_at, r.created_by
                FROM replies r
                LEFT JOIN LATERAL (
                    SELECT i.hash, i.mime
                    FROM images i
                    WHERE i.reply_id = r.id
                    ORDER BY i.id ASC
                    LIMIT 1
                ) img ON TRUE
                WHERE r.id=$1
            "#,
            )
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| RepoError::NotFound)?;
            Ok(rec)
        }
    }

    #[async_trait]
    impl RoleRepo for PgRepo {
        async fn get_subject_role(&self, subject: &str) -> Option<AuthRole> {
            if let Ok(rec) = sqlx::query("SELECT role FROM user_roles WHERE subject=$1")
                .bind(subject)
                .fetch_one(&self.pool)
                .await
            {
                let role: String = rec.get("role");
                return match role.as_str() {
                    "admin" => Some(AuthRole::Admin),
                    "moderator" => Some(AuthRole::Moderator),
                    "user" => Some(AuthRole::User),
                    _ => None,
                };
            }
            None
        }
        async fn set_subject_role(&self, subject: &str, role: AuthRole) -> RepoResult<()> {
            let role_str = match role { AuthRole::Admin => "admin", AuthRole::Moderator => "moderator", AuthRole::User => "user" };
            let _ = sqlx::query("INSERT INTO user_roles (subject, role, updated_at) VALUES ($1,$2, now()) ON CONFLICT (subject) DO UPDATE SET role=EXCLUDED.role, updated_at=now()")
                .bind(subject)
                .bind(role_str)
                .execute(&self.pool)
                .await
                .map_err(|_| RepoError::Conflict)?;
            Ok(())
        }
        async fn list_roles(&self) -> RepoResult<Vec<(String, AuthRole)>> {
            let rows = sqlx::query("SELECT subject, role FROM user_roles ORDER BY subject")
                .fetch_all(&self.pool)
                .await
                .map_err(|_| RepoError::NotFound)?;
            let mut out = Vec::with_capacity(rows.len());
            for r in rows {
                let subject: String = r.get("subject");
                let role_str: String = r.get("role");
                if let Some(role) = match role_str.as_str() { "admin"=>Some(AuthRole::Admin), "moderator"=>Some(AuthRole::Moderator), "user"=>Some(AuthRole::User), _=>None } {
                    out.push((subject, role));
                }
            }
            Ok(out)
        }
        async fn delete_role(&self, subject: &str) -> RepoResult<()> {
            let res = sqlx::query("DELETE FROM user_roles WHERE subject=$1")
                .bind(subject)
                .execute(&self.pool)
                .await
                .map_err(|_| RepoError::NotFound)?;
            if res.rows_affected()==0 { return Err(RepoError::NotFound); }
            Ok(())
        }
    } // end impl RoleRepo
} // end pg module
