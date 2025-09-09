use actix_web::{test, App};
use rib::auth::{create_jwt, Role};
use rib::models::{Board, Reply, Thread};
use rib::repo::pg::PgRepo;
use rib::storage::{ImageStore, ImageStoreError};
use rib::{config, AppState};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Minimal in-memory image store
#[derive(Default)]
struct MockImageStore {
    inner: Mutex<HashMap<String, (Vec<u8>, String)>>,
}
#[async_trait::async_trait]
impl ImageStore for MockImageStore {
    async fn save(&self, hash: &str, mime: &str, bytes: &[u8]) -> Result<(), ImageStoreError> {
        let mut m = self.inner.lock().unwrap();
        if m.contains_key(hash) {
            return Err(ImageStoreError::Duplicate);
        }
        m.insert(hash.to_string(), (bytes.to_vec(), mime.to_string()));
        Ok(())
    }
    async fn load(&self, hash: &str) -> Result<(Vec<u8>, String), ImageStoreError> {
        let m = self.inner.lock().unwrap();
        m.get(hash).cloned().ok_or(ImageStoreError::NotFound)
    }
    async fn delete(&self, hash: &str) -> Result<(), ImageStoreError> {
        let mut m = self.inner.lock().unwrap();
        m.remove(hash);
        Ok(())
    }
}

fn ensure_secret() {
    if std::env::var("JWT_SECRET").is_err() {
        std::env::set_var("JWT_SECRET", "testsecret-cre-by");
    }
}
fn user_token(username: &str) -> String {
    ensure_secret();
    create_jwt(username, username, vec![Role::User]).unwrap()
}
fn admin_token(username: &str) -> String {
    ensure_secret();
    create_jwt(username, username, vec![Role::Admin]).unwrap()
}

async fn repo() -> Option<PgRepo> {
    let url = std::env::var("DATABASE_URL").ok()?;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&url)
        .await
        .ok()?;
    Some(PgRepo::new(pool))
}

// Helper: query raw created_by directly; created_by is skipped in API serialization.
async fn fetch_thread_created_by(pool: &sqlx::Pool<sqlx::Postgres>, id: i64) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT created_by FROM threads WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .ok()
}
async fn fetch_reply_created_by(pool: &sqlx::Pool<sqlx::Postgres>, id: i64) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT created_by FROM replies WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .ok()
}

// Core test: ensure created_by stores username portion of JWT (post-change behavior)
#[actix_web::test]
#[serial_test::serial]
async fn test_created_by_persists_username_for_thread_and_reply() {
    let Some(repo) = repo().await else {
        eprintln!("skip: no DATABASE_URL");
        return;
    };
    // Keep underlying pool to raw query created_by
    let pool = match std::env::var("DATABASE_URL") {
        Ok(u) => PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(&u)
            .await
            .ok(),
        Err(_) => None,
    }
    .expect("pool");
    let state = AppState {
        repo: Arc::new(repo),
        image_store: Arc::new(MockImageStore::default()),
        rate_limiter: None,
    };
    let mut app = test::init_service(
        App::new()
            .app_data(actix_web::web::Data::new(state))
            .configure(config),
    )
    .await;

    let uname = "alice";
    let user_jwt = user_token(uname);

    // Create board (need admin)
    let admin_jwt = admin_token("adminuser");
    let req = test::TestRequest::post()
        .uri("/api/v1/boards")
        .insert_header(("Authorization", format!("Bearer {}", admin_jwt)))
        .set_json(&json!({"slug": format!("cb-{}", uuid::Uuid::new_v4()), "title": "CB"}))
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), 201);
    let board: Board = serde_json::from_slice(&test::read_body(resp).await).unwrap();

    // Create thread as alice
    let req = test::TestRequest::post()
        .uri("/api/v1/threads")
        .insert_header(("Authorization", format!("Bearer {}", user_jwt)))
        .set_json(&json!({"board_id": board.id, "subject": "Hello", "body": "Body"}))
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), 201);
    let thread: Thread = serde_json::from_slice(&test::read_body(resp).await).unwrap();

    // Create reply as alice
    let req = test::TestRequest::post()
        .uri("/api/v1/replies")
        .insert_header(("Authorization", format!("Bearer {}", user_jwt)))
        .set_json(&json!({"thread_id": thread.id, "content": "Hi"}))
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), 201);
    let reply: Reply = serde_json::from_slice(&test::read_body(resp).await).unwrap();

    // Validate created_by (not exposed via JSON) directly via SQL
    let th_created_by = fetch_thread_created_by(&pool, thread.id)
        .await
        .expect("thread created_by");
    let rp_created_by = fetch_reply_created_by(&pool, reply.id)
        .await
        .expect("reply created_by");
    assert_eq!(th_created_by, uname);
    assert_eq!(rp_created_by, uname);
}
