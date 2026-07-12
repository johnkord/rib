use actix_web::{test, App};
use rib::auth::{create_jwt, Role};
use rib::models::{Board, Reply, Thread};
use rib::repo::pg::PgRepo;
use rib::repo::RoleRepo;
use rib::storage::{ImageStore, ImageStoreError};
use rib::{config, AppState};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

#[derive(Default)]
struct MockImageStore;

#[async_trait::async_trait]
impl ImageStore for MockImageStore {
    async fn save(&self, _hash: &str, _mime: &str, _bytes: &[u8]) -> Result<(), ImageStoreError> {
        Ok(())
    }

    async fn load(&self, _hash: &str) -> Result<(Vec<u8>, String), ImageStoreError> {
        Err(ImageStoreError::NotFound)
    }

    async fn delete(&self, _hash: &str) -> Result<(), ImageStoreError> {
        Ok(())
    }
}

fn token(id: &str, username: &str, role: Role) -> String {
    std::env::set_var("JWT_SECRET", "testsecretabcdefghijklmnopqrstuvwxyz012345");
    create_jwt(id, username, vec![role]).expect("test token")
}

#[actix_web::test]
#[serial_test::serial]
async fn moderators_can_identify_and_ban_private_post_authors() {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL required for integration tests");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("connect test database");
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let poster_id = format!("poster-{}", &suffix[..8]);
    let subject = format!("discord:{poster_id}");
    let repo = PgRepo::new(pool);
    repo.set_subject_role(&subject, Role::User)
        .await
        .expect("allowlist poster");
    let state = AppState {
        repo: Arc::new(repo),
        image_store: Arc::new(MockImageStore),
        rate_limiter: None,
    };
    let app = test::init_service(
        App::new()
            .app_data(actix_web::web::Data::new(state))
            .configure(config),
    )
    .await;

    let poster = token(&poster_id, "poster", Role::User);
    let moderator = token("moderator-id", "moderator", Role::Moderator);
    let admin = token("admin-id", "admin", Role::Admin);

    let request = test::TestRequest::post()
        .uri("/api/v1/boards")
        .insert_header(("Authorization", format!("Bearer {admin}")))
        .set_json(json!({
            "slug": format!("mod{}", &suffix[..8]),
            "title": "Moderation identity"
        }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), 201);
    let board: Board = serde_json::from_slice(&test::read_body(response).await).unwrap();

    let request = test::TestRequest::post()
        .uri("/api/v1/threads")
        .insert_header(("Authorization", format!("Bearer {poster}")))
        .set_json(json!({
            "board_id": board.id,
            "subject": "hello",
            "body": "body",
            "author_name": "Alice",
            "tripcode_password": "correct horse"
        }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), 201);
    let body = test::read_body(response).await;
    let public_thread: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(public_thread.get("created_by").is_none());
    assert!(public_thread.get("tripcode_password").is_none());
    assert_eq!(public_thread["author_name"], "Alice");
    assert!(public_thread["tripcode"]
        .as_str()
        .is_some_and(|tripcode| tripcode.starts_with('!')));
    let thread: Thread = serde_json::from_slice(&body).unwrap();

    let request = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/threads/{}/author", thread.id))
        .insert_header(("Authorization", format!("Bearer {poster}")))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), 403);

    let request = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/threads/{}/author", thread.id))
        .insert_header(("Authorization", format!("Bearer {moderator}")))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), 200);
    let attribution: serde_json::Value =
        serde_json::from_slice(&test::read_body(response).await).unwrap();
    assert_eq!(attribution["subject"], subject);
    assert_eq!(attribution["details"]["username"], "poster");

    let request = test::TestRequest::post()
        .uri("/api/v1/admin/bans")
        .insert_header(("Authorization", format!("Bearer {moderator}")))
        .set_json(json!({"subject": subject, "reason": "integration test"}))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), 201);

    let request = test::TestRequest::post()
        .uri("/api/v1/replies")
        .insert_header(("Authorization", format!("Bearer {poster}")))
        .set_json(json!({"thread_id": thread.id, "content": "blocked"}))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), 403);

    let request = test::TestRequest::delete()
        .uri(&format!(
            "/api/v1/admin/bans/{}",
            urlencoding::encode(&subject)
        ))
        .insert_header(("Authorization", format!("Bearer {moderator}")))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), 204);

    let request = test::TestRequest::post()
        .uri("/api/v1/replies")
        .insert_header(("Authorization", format!("Bearer {poster}")))
        .set_json(json!({
            "thread_id": thread.id,
            "content": "allowed",
            "author_name": "Alice",
            "tripcode_password": "correct horse"
        }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), 201);
    let reply: Reply = serde_json::from_slice(&test::read_body(response).await).unwrap();
    assert_eq!(reply.author_name.as_deref(), Some("Alice"));
    assert_eq!(reply.tripcode, thread.tripcode);

    let request = test::TestRequest::get()
        .uri(&format!("/api/v1/admin/replies/{}/author", reply.id))
        .insert_header(("Authorization", format!("Bearer {moderator}")))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), 200);
    let attribution: serde_json::Value =
        serde_json::from_slice(&test::read_body(response).await).unwrap();
    assert_eq!(attribution["subject"], format!("discord:{poster_id}"));
}
