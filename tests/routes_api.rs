use actix_web::{test, App};
use rib::auth::{create_jwt, Role};
use rib::models::{Board, Thread};
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

fn token(id: &str, role: Role) -> String {
    std::env::set_var("JWT_SECRET", "testsecretabcdefghijklmnopqrstuvwxyz012345");
    create_jwt(id, id, vec![role]).expect("test token")
}

async fn test_repo() -> PgRepo {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL required for integration tests");
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("connect test database");
    let repo = PgRepo::new(pool);
    repo.set_subject_role("discord:validation-user", Role::User)
        .await
        .expect("allowlist validation user");
    repo
}

#[actix_web::test]
#[serial_test::serial]
async fn api_rejects_invalid_board_post_and_attachment_inputs() {
    let app = test::init_service(
        App::new()
            .app_data(actix_web::web::Data::new(AppState {
                repo: Arc::new(test_repo().await),
                image_store: Arc::new(MockImageStore),
                rate_limiter: None,
            }))
            .configure(config),
    )
    .await;
    let admin = token("validation-admin", Role::Admin);
    let user = token("validation-user", Role::User);

    let request = test::TestRequest::post()
        .uri("/api/v1/boards")
        .insert_header(("Authorization", format!("Bearer {admin}")))
        .set_json(json!({"slug": "Bad Slug", "title": "Invalid"}))
        .to_request();
    assert_eq!(test::call_service(&app, request).await.status(), 400);

    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let request = test::TestRequest::post()
        .uri("/api/v1/boards")
        .insert_header(("Authorization", format!("Bearer {admin}")))
        .set_json(json!({"slug": format!("valid{}", &suffix[..8]), "title": "Valid"}))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), 201);
    let board: Board = serde_json::from_slice(&test::read_body(response).await).unwrap();

    let request = test::TestRequest::post()
        .uri("/api/v1/threads")
        .insert_header(("Authorization", format!("Bearer {user}")))
        .set_json(json!({"board_id": board.id, "subject": "x".repeat(201), "body": "body"}))
        .to_request();
    assert_eq!(test::call_service(&app, request).await.status(), 400);

    let request = test::TestRequest::post()
        .uri("/api/v1/threads")
        .insert_header(("Authorization", format!("Bearer {user}")))
        .set_json(json!({
            "board_id": board.id,
            "subject": "bad attachment",
            "body": "body",
            "image_hash": "a".repeat(64)
        }))
        .to_request();
    assert_eq!(test::call_service(&app, request).await.status(), 400);

    let request = test::TestRequest::post()
        .uri("/api/v1/threads")
        .insert_header(("Authorization", format!("Bearer {user}")))
        .set_json(json!({"board_id": board.id, "subject": "valid", "body": "body"}))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), 201);
    let thread: Thread = serde_json::from_slice(&test::read_body(response).await).unwrap();

    let request = test::TestRequest::post()
        .uri("/api/v1/replies")
        .insert_header(("Authorization", format!("Bearer {user}")))
        .set_json(json!({"thread_id": thread.id, "content": ""}))
        .to_request();
    assert_eq!(test::call_service(&app, request).await.status(), 400);

    let request = test::TestRequest::delete()
        .uri("/api/v1/admin/roles/discord%3Avalidation-user")
        .insert_header(("Authorization", format!("Bearer {admin}")))
        .to_request();
    assert_eq!(test::call_service(&app, request).await.status(), 204);

    let request = test::TestRequest::post()
        .uri("/api/v1/threads")
        .insert_header(("Authorization", format!("Bearer {user}")))
        .set_json(json!({"board_id": board.id, "subject": "revoked", "body": "body"}))
        .to_request();
    assert_eq!(test::call_service(&app, request).await.status(), 403);
}
