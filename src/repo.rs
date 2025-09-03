use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::Utc;

use crate::auth::Role as AuthRole;
use crate::models::*;

#[derive(thiserror::Error, Debug)]
pub enum RepoError {
    #[error("not found")] NotFound,
    #[error("conflict")] Conflict,
}

pub type RepoResult<T> = Result<T, RepoError>;

use async_trait::async_trait;

#[async_trait]
pub trait BoardRepo: Send + Sync {
    async fn list_boards(&self) -> RepoResult<Vec<Board>>;
    async fn create_board(&self, new: NewBoard) -> RepoResult<Board>;
    async fn update_board(&self, id: Id, upd: UpdateBoard) -> RepoResult<Board>;
}

#[async_trait]
pub trait ThreadRepo: Send + Sync {
    async fn list_threads(&self, board_id: Id) -> RepoResult<Vec<Thread>>;
    async fn create_thread(&self, new: NewThread) -> RepoResult<Thread>;
    async fn get_thread(&self, id: Id) -> RepoResult<Thread>;
}

#[async_trait]
pub trait ReplyRepo: Send + Sync {
    async fn list_replies(&self, thread_id: Id) -> RepoResult<Vec<Reply>>;
    async fn create_reply(&self, new: NewReply) -> RepoResult<Reply>;
}

#[async_trait]
pub trait DiscordRoleRepo: Send + Sync {
    async fn get_discord_user_role(&self, discord_id: &str) -> Option<AuthRole>;
    async fn set_discord_user_role(&self, discord_id: &str, role: AuthRole) -> RepoResult<()>;
}

pub trait Repo: BoardRepo + ThreadRepo + ReplyRepo + DiscordRoleRepo {}

impl<T> Repo for T where T: BoardRepo + ThreadRepo + ReplyRepo + DiscordRoleRepo {}

#[cfg(feature = "inmem-store")]
pub mod inmem {
    use super::*;
    use serde::{Serialize, Deserialize};
    use serde_json;
    use std::path::{PathBuf, Path};

    const SNAPSHOT_PATH: &str = "data/state.json";  // NEW

    #[derive(Default, Serialize, Deserialize)]      // + Serialize/Deserialize
    struct State {
        boards:  HashMap<Id, Board>,
        threads: HashMap<Id, Thread>,
        replies: HashMap<Id, Reply>,
        images:  HashMap<Id, Image>, // new
        discord_roles: HashMap<String, AuthRole>,  // NEW: Discord ID -> Role mapping
        next_id: Id,
    }

    #[derive(Clone)]
    pub struct InMemRepo {
        state: Arc<RwLock<State>>,
        snapshot_path: Arc<PathBuf>,              // NEW
    }

    impl InMemRepo {
        // NEW: resolve data dir (env override)
        fn data_dir() -> PathBuf {
            std::env::var("RIB_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("data"))
        }
        // NEW: compute full snapshot path
        fn snapshot_path() -> PathBuf {
            if std::env::var("RIB_DATA_DIR").is_ok() {
                let mut p = Self::data_dir();
                p.push("state.json");
                p
            } else {
                PathBuf::from(SNAPSHOT_PATH)   // use constant default
            }
        }

        // UPDATED: load with logging (no silent swallow)
        fn load_state_from(path: &Path) -> State {
            match std::fs::read(path) {
                Ok(bytes) => match serde_json::from_slice::<State>(&bytes) {
                    Ok(s) => {
                        eprintln!("[inmem] Loaded snapshot '{}'", path.display());
                        s
                    }
                    Err(e) => {
                        eprintln!("[inmem] Failed to parse snapshot '{}': {e}. Starting empty.", path.display());
                        State::default()
                    }
                },
                Err(e) => {
                    eprintln!("[inmem] No snapshot at '{}': {e}. Starting empty.", path.display());
                    State::default()
                }
            }
        }

        // UPDATED: persist uses stored path
        fn persist(&self) {
            let path = self.snapshot_path.clone();
            if let Ok(s) = serde_json::to_vec_pretty(&*self.state.read().unwrap()) {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Err(e) = std::fs::write(&*path, s) {
                    eprintln!("[inmem] Failed to write snapshot '{}': {e}", path.display());
                }
            }
        }

        pub fn new() -> Self {
            let snapshot_path = Self::snapshot_path();
            let state = Self::load_state_from(&snapshot_path);
            Self {
                state: Arc::new(RwLock::new(state)),
                snapshot_path: Arc::new(snapshot_path),
            }
        }

        fn next_id(state: &mut State) -> Id {
            state.next_id += 1;
            state.next_id
        }
    }

    impl Default for InMemRepo {           // keeps `#[cfg(feature)]` callers unchanged
        fn default() -> Self { Self::new() }
    }

    #[async_trait]
    impl BoardRepo for InMemRepo {
        async fn list_boards(&self) -> RepoResult<Vec<Board>> {
            let s = self.state.read().unwrap();
            Ok(s.boards.values().cloned().collect())
        }
        async fn create_board(&self, new: NewBoard) -> RepoResult<Board> {
            let mut s = self.state.write().unwrap();
            if s.boards.values().any(|b| b.slug == new.slug) {
                return Err(RepoError::Conflict);
            }
            let id = Self::next_id(&mut s);
            let board = Board { id, slug: new.slug, title: new.title, created_at: Utc::now() };
            s.boards.insert(id, board.clone());
            drop(s);                       // release lock before persisting
            self.persist();                // NEW
            Ok(board)
        }
        async fn update_board(&self, id: Id, upd: UpdateBoard) -> RepoResult<Board> {  // NEW
            let mut s = self.state.write().unwrap();

            // ── 1. uniqueness check BEFORE mutable borrow ────────────
            if let Some(ref slug) = upd.slug {
                if s.boards.values().any(|b| b.slug == *slug && b.id != id) {
                    return Err(RepoError::Conflict);
                }
            }

            // ── 2. now safe to take mutable reference ────────────────
            let board = s.boards.get_mut(&id).ok_or(RepoError::NotFound)?;

            if let Some(slug) = upd.slug { board.slug = slug; }
            if let Some(title) = upd.title { board.title = title; }

            let updated = board.clone();
            drop(s);
            self.persist();
            Ok(updated)
        }
    }

    #[async_trait]
    impl ThreadRepo for InMemRepo {
        async fn list_threads(&self, board_id: Id) -> RepoResult<Vec<Thread>> {
            let s = self.state.read().unwrap();
            let mut v: Vec<_> = s.threads.values()
                .filter(|t| t.board_id == board_id)
                .cloned()
                .collect();
            v.sort_by(|a, b| b.bump_time.cmp(&a.bump_time));        // NEW (latest first)
            Ok(v)
        }
        async fn create_thread(&self, new: NewThread) -> RepoResult<Thread> {
            let mut s = self.state.write().unwrap();
            if !s.boards.contains_key(&new.board_id) { return Err(RepoError::NotFound); }
            let now = Utc::now();                                   // NEW
            let id = Self::next_id(&mut s);
            let thread = Thread {
                id,
                board_id: new.board_id,
                subject: new.subject,
                body: new.body.clone(),           // ← added
                created_at: now,
                bump_time:  now,
                image_hash: new.image_hash.clone(),
                mime: new.mime.clone(),
            };
            s.threads.insert(id, thread.clone());
            // store image row if attachment present
            if let (Some(h), Some(m)) = (new.image_hash, new.mime) {
                let img_id = Self::next_id(&mut s);
                let img = Image { id: img_id, thread_id: Some(id), reply_id: None, hash: h, mime: m };
                s.images.insert(img_id, img);
            }
            drop(s);                       // release lock before persisting
            self.persist();                // NEW
            Ok(thread)
        }
        async fn get_thread(&self, id: Id) -> RepoResult<Thread> {
            let s = self.state.read().unwrap();
            s.threads.get(&id).cloned().ok_or(RepoError::NotFound)
        }
    }

    #[async_trait]
    impl ReplyRepo for InMemRepo {
        async fn list_replies(&self, thread_id: Id) -> RepoResult<Vec<Reply>> {
            let s = self.state.read().unwrap();
            let mut v: Vec<_> = s.replies
                .values()
                .filter(|r| r.thread_id == thread_id)
                .cloned()
                .collect();
            v.sort_by(|a, b| a.created_at.cmp(&b.created_at));    // ascending
            Ok(v)
        }
        async fn create_reply(&self, new: NewReply) -> RepoResult<Reply> {
            let mut s = self.state.write().unwrap();
            if !s.threads.contains_key(&new.thread_id) { return Err(RepoError::NotFound); }
            let id = Self::next_id(&mut s);
            let reply = Reply {
                id,
                thread_id: new.thread_id,
                content: new.content,
                image_hash: new.image_hash, // new
                mime: new.mime,             // new
                created_at: Utc::now(),
            };
            s.replies.insert(id, reply.clone());
            // bump thread
            if let Some(th) = s.threads.get_mut(&new.thread_id) { th.bump_time = Utc::now(); }
            drop(s);                       // release lock before persisting
            self.persist();                // NEW
            Ok(reply)
        }
    }

    #[async_trait]
    impl DiscordRoleRepo for InMemRepo {
        async fn get_discord_user_role(&self, discord_id: &str) -> Option<AuthRole> {
            let s = self.state.read().unwrap();
            s.discord_roles.get(discord_id).cloned()
        }

        async fn set_discord_user_role(&self, discord_id: &str, role: AuthRole) -> RepoResult<()> {
            let mut s = self.state.write().unwrap();
            s.discord_roles.insert(discord_id.to_string(), role);
            drop(s);
            self.persist();
            Ok(())
        }
    }
}

// Postgres implementation (feature = "postgres-store")
#[cfg(feature = "postgres-store")]
pub mod pg {
    use super::*;
    use sqlx::{Pool, Postgres};
    use chrono::Utc;

    #[derive(Clone)]
    pub struct PgRepo { pool: Pool<Postgres> }

    impl PgRepo {
        pub fn new(pool: Pool<Postgres>) -> Self { Self { pool } }
    }

    #[async_trait]
    impl BoardRepo for PgRepo {
        async fn list_boards(&self) -> RepoResult<Vec<Board>> {
            let recs = sqlx::query_as::<_, Board>("SELECT id, slug, title, created_at FROM boards ORDER BY id")
                .fetch_all(&self.pool).await.map_err(|_| RepoError::NotFound)?; // simplify error mapping
            Ok(recs)
        }
        async fn create_board(&self, new: NewBoard) -> RepoResult<Board> {
            let rec = sqlx::query_as::<_, Board>("INSERT INTO boards (slug, title) VALUES ($1,$2) RETURNING id, slug, title, created_at")
                .bind(&new.slug).bind(&new.title)
                .fetch_one(&self.pool).await.map_err(|_| RepoError::Conflict)?;
            Ok(rec)
        }
        async fn update_board(&self, id: Id, upd: UpdateBoard) -> RepoResult<Board> {
            // Build dynamic SQL (simple field subset)
            let mut slug = None; let mut title = None;
            if let Some(s) = upd.slug { slug = Some(s); }
            if let Some(t) = upd.title { title = Some(t); }
            let rec = sqlx::query_as::<_, Board>(
                "UPDATE boards SET slug = COALESCE($2, slug), title = COALESCE($3, title) WHERE id=$1 RETURNING id, slug, title, created_at"
            )
            .bind(id)
            .bind(slug.as_ref())
            .bind(title.as_ref())
            .fetch_one(&self.pool).await.map_err(|_| RepoError::NotFound)?;
            Ok(rec)
        }
    }

    #[async_trait]
    impl ThreadRepo for PgRepo {
        async fn list_threads(&self, board_id: Id) -> RepoResult<Vec<Thread>> {
            let recs = sqlx::query_as::<_, Thread>(r#"
                SELECT t.id, t.board_id, t.subject, t.body, t.created_at, t.bump_time,
                       img.hash as image_hash, img.mime as mime
                FROM threads t
                LEFT JOIN LATERAL (
                   SELECT i.hash, i.mime FROM images i
                   WHERE i.thread_id = t.id
                   ORDER BY i.id ASC LIMIT 1
                ) img ON TRUE
                WHERE t.board_id = $1
                ORDER BY t.bump_time DESC
            "#)
                .bind(board_id)
                .fetch_all(&self.pool).await.map_err(|_| RepoError::NotFound)?;
            Ok(recs)
        }
        async fn create_thread(&self, new: NewThread) -> RepoResult<Thread> {
            let mut tx = self.pool.begin().await.map_err(|_| RepoError::Conflict)?;
            let rec = sqlx::query!(
                "INSERT INTO threads (board_id, subject, body) VALUES ($1,$2,$3) RETURNING id, board_id, subject, body, created_at, bump_time",
                new.board_id, new.subject, new.body
            ).fetch_one(&mut *tx).await.map_err(|_| RepoError::NotFound)?;
            if let (Some(hash), Some(mime)) = (new.image_hash.as_ref(), new.mime.as_ref()) {
                // Insert image row if it does not already exist; associate to thread
                let _ = sqlx::query!(
                    "INSERT INTO images (thread_id, reply_id, hash, mime) VALUES ($1,NULL,$2,$3) ON CONFLICT (hash) DO NOTHING",
                    rec.id, hash, mime
                ).execute(&mut *tx).await;
            }
            tx.commit().await.map_err(|_| RepoError::Conflict)?;
            // Re-select with image join to populate struct
            let thread = sqlx::query_as::<_, Thread>(r#"
                SELECT t.id, t.board_id, t.subject, t.body, t.created_at, t.bump_time,
                       img.hash as image_hash, img.mime as mime
                FROM threads t
                LEFT JOIN LATERAL (
                   SELECT i.hash, i.mime FROM images i WHERE i.thread_id = t.id ORDER BY i.id ASC LIMIT 1
                ) img ON TRUE
                WHERE t.id = $1
            "#).bind(rec.id).fetch_one(&self.pool).await.map_err(|_| RepoError::NotFound)?;
            Ok(thread)
        }
        async fn get_thread(&self, id: Id) -> RepoResult<Thread> {
            let thread = sqlx::query_as::<_, Thread>(r#"
                SELECT t.id, t.board_id, t.subject, t.body, t.created_at, t.bump_time,
                       img.hash as image_hash, img.mime as mime
                FROM threads t
                LEFT JOIN LATERAL (
                   SELECT i.hash, i.mime FROM images i WHERE i.thread_id = t.id ORDER BY i.id ASC LIMIT 1
                ) img ON TRUE
                WHERE t.id = $1
            "#).bind(id).fetch_one(&self.pool).await.map_err(|_| RepoError::NotFound)?;
            Ok(thread)
        }
    }

    #[async_trait]
    impl ReplyRepo for PgRepo {
        async fn list_replies(&self, thread_id: Id) -> RepoResult<Vec<Reply>> {
            let recs = sqlx::query_as::<_, Reply>(r#"
                SELECT r.id, r.thread_id, r.content, img.hash as image_hash, img.mime as mime, r.created_at
                FROM replies r
                LEFT JOIN LATERAL (
                   SELECT i.hash, i.mime FROM images i WHERE i.reply_id = r.id ORDER BY i.id ASC LIMIT 1
                ) img ON TRUE
                WHERE r.thread_id = $1
                ORDER BY r.created_at ASC
            "#)
                .bind(thread_id)
                .fetch_all(&self.pool).await.map_err(|_| RepoError::NotFound)?;
            Ok(recs)
        }
        async fn create_reply(&self, new: NewReply) -> RepoResult<Reply> {
            let mut tx = self.pool.begin().await.map_err(|_| RepoError::Conflict)?;
            let rec = sqlx::query!(
                "INSERT INTO replies (thread_id, content) VALUES ($1,$2) RETURNING id, thread_id, content, created_at",
                new.thread_id, new.content
            ).fetch_one(&mut *tx).await.map_err(|_| RepoError::NotFound)?;
            if let (Some(hash), Some(mime)) = (new.image_hash.as_ref(), new.mime.as_ref()) {
                let _ = sqlx::query!(
                    "INSERT INTO images (thread_id, reply_id, hash, mime) VALUES (NULL,$1,$2,$3) ON CONFLICT (hash) DO NOTHING",
                    rec.id, hash, mime
                ).execute(&mut *tx).await;
            }
            let _ = sqlx::query("UPDATE threads SET bump_time = now() WHERE id=$1")
                .bind(new.thread_id)
                .execute(&mut *tx).await;
            tx.commit().await.map_err(|_| RepoError::Conflict)?;
            // Re-select with image join
            let reply = sqlx::query_as::<_, Reply>(r#"
                SELECT r.id, r.thread_id, r.content, img.hash as image_hash, img.mime as mime, r.created_at
                FROM replies r
                LEFT JOIN LATERAL (
                   SELECT i.hash, i.mime FROM images i WHERE i.reply_id = r.id ORDER BY i.id ASC LIMIT 1
                ) img ON TRUE
                WHERE r.id = $1
            "#).bind(rec.id).fetch_one(&self.pool).await.map_err(|_| RepoError::NotFound)?;
            Ok(reply)
        }
    }

    #[async_trait]
    impl DiscordRoleRepo for PgRepo {
        async fn get_discord_user_role(&self, _discord_id: &str) -> Option<AuthRole> { None }
        async fn set_discord_user_role(&self, _discord_id: &str, _role: AuthRole) -> RepoResult<()> { Ok(()) }
    }
}
