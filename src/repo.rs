use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::Utc;

use crate::models::*;

#[derive(thiserror::Error, Debug)]
pub enum RepoError {
    #[error("not found")] NotFound,
    #[error("conflict")] Conflict,
    #[error("internal error: {0}")] Internal(String),
}

pub type RepoResult<T> = Result<T, RepoError>;

pub trait BoardRepo: Send + Sync {
    fn list_boards(&self) -> RepoResult<Vec<Board>>;
    fn create_board(&self, new: NewBoard) -> RepoResult<Board>;
    fn get_board(&self, id: Id) -> RepoResult<Board>;
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

pub trait Repo: BoardRepo + ThreadRepo + ReplyRepo {}

impl<T> Repo for T where T: BoardRepo + ThreadRepo + ReplyRepo {}

#[cfg(feature = "inmem-store")]
pub mod inmem {
    use super::*;
    use serde::{Serialize, Deserialize};   // NEW
    use serde_json;                       // NEW

    const SNAPSHOT_PATH: &str = "data/state.json";  // NEW

    #[derive(Default, Serialize, Deserialize)]      // + Serialize/Deserialize
    struct State {
        boards:  HashMap<Id, Board>,
        threads: HashMap<Id, Thread>,
        replies: HashMap<Id, Reply>,
        images:  HashMap<Id, Image>, // new
        next_id: Id,
    }

    #[derive(Clone)]
    pub struct InMemRepo {
        state: Arc<RwLock<State>>,
    }

    impl InMemRepo {
        // NEW: load snapshot or start with empty state
        fn load_state() -> State {
            std::fs::read(SNAPSHOT_PATH)
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or_default()
        }

        // NEW: persist current state; best-effort (ignore errors)
        fn persist(&self) {
            if let Ok(s) = serde_json::to_vec_pretty(&*self.state.read().unwrap()) {
                let _ = std::fs::create_dir_all("data");
                let _ = std::fs::write(SNAPSHOT_PATH, s);
            }
        }

        pub fn new() -> Self {
            Self { state: Arc::new(RwLock::new(Self::load_state())) }
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
        fn get_board(&self, id: Id) -> RepoResult<Board> {
            let s = self.state.read().unwrap();
            s.boards.get(&id).cloned().ok_or(RepoError::NotFound)
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
                created_at: now,                                    // NEW
                bump_time:  now,                                    // unchanged
                image_hash: new.image_hash.clone(), // new
                mime: new.mime.clone(),             // new
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
}
