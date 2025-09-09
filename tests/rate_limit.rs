use actix_web::{test, App};
use rib::auth::{create_jwt, Role};
use rib::models::Board;
use rib::rate_limit::{InMemoryRateLimiter, RateLimitConfig, RateLimiterFacade};
use rib::repo::pg::PgRepo;
use rib::{config, AppState};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rib::storage::{ImageStore, ImageStoreError};

#[derive(Default)]
struct MockImageStore { inner: Mutex<HashMap<String,(Vec<u8>,String)>> }
#[async_trait::async_trait]
impl ImageStore for MockImageStore {
    async fn save(&self, hash:&str, mime:&str, bytes:&[u8]) -> Result<(), ImageStoreError> {
        let mut m = self.inner.lock().unwrap();
        if m.contains_key(hash) { return Err(ImageStoreError::Duplicate); }
        m.insert(hash.to_string(), (bytes.to_vec(), mime.to_string()));
        Ok(())
    }
    async fn load(&self, hash:&str) -> Result<(Vec<u8>, String), ImageStoreError> {
        let m = self.inner.lock().unwrap();
        m.get(hash).cloned().ok_or(ImageStoreError::NotFound)
    }
    async fn delete(&self, hash:&str) -> Result<(), ImageStoreError> { let mut m = self.inner.lock().unwrap(); m.remove(hash); Ok(()) }
}

fn ensure_secret() {
    if std::env::var("JWT_SECRET").is_err() { std::env::set_var("JWT_SECRET", "testsecret-abcdefghijklmnopqrstuvwxyz012345"); }
}
fn user_token() -> String { ensure_secret(); create_jwt("user","user", vec![Role::User]).unwrap() }

async fn pg_repo() -> Option<PgRepo> {
    let url = std::env::var("DATABASE_URL").ok()?;
    let pool = PgPoolOptions::new().max_connections(1).acquire_timeout(std::time::Duration::from_secs(5)).connect(&url).await.ok()?;
    Some(PgRepo::new(pool))
}

#[actix_web::test]
#[serial_test::serial]
async fn rate_limit_thread_creation() {
    let Some(repo) = pg_repo().await else { eprintln!("skip: no DATABASE_URL"); return; };

    // limiter: only 1 thread per large window so second immediately denied
    let cfg = RateLimitConfig { thread_limit:1, thread_window: std::time::Duration::from_secs(300), reply_limit:100, reply_window: std::time::Duration::from_secs(60), image_limit:100, image_window: std::time::Duration::from_secs(3600)};
    let limiter = RateLimiterFacade::new(InMemoryRateLimiter::new(true), cfg);

    let state = AppState { repo: Arc::new(repo), image_store: Arc::new(MockImageStore::default()), rate_limiter: Some(limiter) };
    let mut app = test::init_service(App::new().app_data(actix_web::web::Data::new(state)).configure(config)).await;

    let user = user_token();

    // board creation requires Admin
    let admin = create_jwt("admin","admin", vec![Role::Admin]).unwrap();
    let slug = format!("rl-{}", chrono::Utc::now().timestamp_nanos());
    let req = test::TestRequest::post().uri("/api/v1/boards").insert_header(("Authorization", format!("Bearer {admin}"))).set_json(&json!({"slug":slug, "title":"RL"})).to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), 201, "board create should succeed");
    let board: Board = serde_json::from_slice(&test::read_body(resp).await).unwrap();

    // first thread create -> 201
    let req = test::TestRequest::post().uri("/api/v1/threads").insert_header(("Authorization", format!("Bearer {user}"))).set_json(&json!({"board_id":board.id, "subject":"S1", "body":"B1"})).to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), 201, "first thread create allowed");

    // second thread create -> 429
    let req = test::TestRequest::post().uri("/api/v1/threads").insert_header(("Authorization", format!("Bearer {user}"))).set_json(&json!({"board_id":board.id, "subject":"S2", "body":"B2"})).to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), 429, "second thread should be rate limited");
}
