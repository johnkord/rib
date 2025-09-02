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

pub trait BoardRepo: Send + Sync {
    fn list_boards(&self) -> RepoResult<Vec<Board>>;
    fn create_board(&self, new: NewBoard) -> RepoResult<Board>;
    fn update_board(&self, id: Id, upd: UpdateBoard) -> RepoResult<Board>;
}

pub trait ThreadRepo: Send + Sync {
    fn list_threads(&self, board_id: Id) -> RepoResult<Vec<Thread>>;
    fn create_thread(&self, new: NewThread) -> RepoResult<Thread>;
    fn get_thread(&self, id: Id) -> RepoResult<Thread>;
}

pub trait ReplyRepo: Send + Sync {
    fn list_replies(&self, thread_id: Id) -> RepoResult<Vec<Reply>>;
    fn create_reply(&self, new: NewReply) -> RepoResult<Reply>;
}

pub trait DiscordRoleRepo: Send + Sync {
    fn get_discord_user_role(&self, discord_id: &str) -> Option<AuthRole>;
    fn set_discord_user_role(&self, discord_id: &str, role: AuthRole) -> RepoResult<()>;
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

    impl BoardRepo for InMemRepo {
        fn list_boards(&self) -> RepoResult<Vec<Board>> {
            let s = self.state.read().unwrap();
            Ok(s.boards.values().cloned().collect())
        }
        fn create_board(&self, new: NewBoard) -> RepoResult<Board> {
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
        fn update_board(&self, id: Id, upd: UpdateBoard) -> RepoResult<Board> {  // NEW
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

    impl ThreadRepo for InMemRepo {
        fn list_threads(&self, board_id: Id) -> RepoResult<Vec<Thread>> {
            let s = self.state.read().unwrap();
            let mut v: Vec<_> = s.threads.values()
                .filter(|t| t.board_id == board_id)
                .cloned()
                .collect();
            v.sort_by(|a, b| b.bump_time.cmp(&a.bump_time));        // NEW (latest first)
            Ok(v)
        }
        fn create_thread(&self, new: NewThread) -> RepoResult<Thread> {
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
        fn get_thread(&self, id: Id) -> RepoResult<Thread> {
            let s = self.state.read().unwrap();
            s.threads.get(&id).cloned().ok_or(RepoError::NotFound)
        }
    }

    impl ReplyRepo for InMemRepo {
        fn list_replies(&self, thread_id: Id) -> RepoResult<Vec<Reply>> {
            let s = self.state.read().unwrap();
            let mut v: Vec<_> = s.replies
                .values()
                .filter(|r| r.thread_id == thread_id)
                .cloned()
                .collect();
            v.sort_by(|a, b| a.created_at.cmp(&b.created_at));    // ascending
            Ok(v)
        }
        fn create_reply(&self, new: NewReply) -> RepoResult<Reply> {
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

    impl DiscordRoleRepo for InMemRepo {
        fn get_discord_user_role(&self, discord_id: &str) -> Option<AuthRole> {
            let s = self.state.read().unwrap();
            s.discord_roles.get(discord_id).cloned()
        }

        fn set_discord_user_role(&self, discord_id: &str, role: AuthRole) -> RepoResult<()> {
            let mut s = self.state.write().unwrap();
            s.discord_roles.insert(discord_id.to_string(), role);
            drop(s);
            self.persist();
            Ok(())
        }
    }
}
