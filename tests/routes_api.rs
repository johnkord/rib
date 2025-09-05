#![cfg(feature = "inmem-store")]

use actix_web::{test, App};
use rib::{routes::{config, AppState}, security::SecurityHeaders, auth::{create_jwt, Role}};
use rib::repo::inmem::InMemRepo;
use rib::storage::FsImageStore;
use std::sync::Arc;
use serial_test::serial;

// Helper to ensure JWT secret present & unique temp data dir per test
fn setup_env() {
    std::env::set_var("JWT_SECRET", "test-secret-must-be-32-bytes-long!!");
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("RIB_DATA_DIR", tmp.path().to_str().unwrap());
}

fn admin_token() -> String { create_jwt("1", "admin", vec![Role::Admin]).unwrap() }
fn user_token() -> String { create_jwt("2", "user", vec![Role::User]).unwrap() }


#[actix_web::test]
#[serial]
async fn test_board_thread_reply_flow_routes() {
    setup_env();
    let repo = InMemRepo::new();
    let image_store = FsImageStore::new();
    let app = test::init_service(
        App::new()
            .wrap(SecurityHeaders::from_env())
            .app_data(actix_web::web::Data::new(AppState { repo: Arc::new(repo), image_store: Arc::new(image_store) }))
            .configure(config)
    ).await;

    // list boards empty
    let req = test::TestRequest::get().uri("/api/v1/boards").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body = test::read_body(resp).await;
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 0);

    // create board (admin)
    let req = test::TestRequest::post()
        .uri("/api/v1/boards")
        .insert_header(("Authorization", format!("Bearer {}", admin_token())))
        .set_json(&serde_json::json!({"slug":"tech","title":"Technology"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let board: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let board_id = board["id"].as_u64().unwrap();

    // create thread (user)
    let req = test::TestRequest::post()
        .uri("/api/v1/threads")
        .insert_header(("Authorization", format!("Bearer {}", user_token())))
        .set_json(&serde_json::json!({
            "board_id": board_id,
            "subject": "First",
            "body": "OP body",
            "image_hash": null,
            "mime": null
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let thread: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let thread_id = thread["id"].as_u64().unwrap();

    // list threads
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/boards/{}/threads", board_id))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let threads: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(threads.as_array().unwrap().len(), 1);

    // get thread
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/threads/{}", thread_id))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // create reply (user)
    let req = test::TestRequest::post()
        .uri("/api/v1/replies")
        .insert_header(("Authorization", format!("Bearer {}", user_token())))
        .set_json(&serde_json::json!({
            "thread_id": thread_id,
            "content": "hi",
            "image_hash": null,
            "mime": null
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);

    // list replies
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/threads/{}/replies", thread_id))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let replies: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(replies.as_array().unwrap().len(), 1);

    // update board
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/boards/{}", board_id))
        .insert_header(("Authorization", format!("Bearer {}", admin_token())))
        .set_json(&serde_json::json!({"slug":"gadg","title":"Gadgets"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let upd: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(upd["slug"], "gadg");
}

#[actix_web::test]
#[serial]
async fn test_auth_me_and_refresh() {
    setup_env();
    let repo = InMemRepo::new();
    let image_store = FsImageStore::new();
    let app = test::init_service(
        App::new()
            .wrap(SecurityHeaders::from_env())
            .app_data(actix_web::web::Data::new(AppState { repo: Arc::new(repo), image_store: Arc::new(image_store) }))
            .configure(config)
    ).await;

    let token = user_token();

    // auth/me
    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let me: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert_eq!(me["role"], "user");

    // refresh
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let refreshed: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert!(refreshed["token"].as_str().unwrap().len() > 10);
}

#[actix_web::test]
#[serial]
async fn test_set_discord_role_endpoint() {
    setup_env();
    let repo = InMemRepo::new();
    let image_store = FsImageStore::new();
    let app = test::init_service(
        App::new()
            .wrap(SecurityHeaders::from_env())
            .app_data(actix_web::web::Data::new(AppState { repo: Arc::new(repo), image_store: Arc::new(image_store) }))
            .configure(config)
    ).await;

    // set role for discord id 123 as moderator via admin
    let req = test::TestRequest::post()
        .uri("/api/v1/admin/discord-roles")
        .insert_header(("Authorization", format!("Bearer {}", admin_token())))
        .set_json(&serde_json::json!({"discord_id":"123","role":"moderator"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

// Minimal test for get_image after upload (PNG bytes)
#[actix_web::test]
#[serial]
async fn test_get_image_after_upload() {
    setup_env();
    let repo = InMemRepo::new();
    let image_store = FsImageStore::new();
    let app = test::init_service(
        App::new()
            .wrap(SecurityHeaders::from_env())
            .app_data(actix_web::web::Data::new(AppState { repo: Arc::new(repo), image_store: Arc::new(image_store) }))
            .configure(config)
    ).await;

    let boundary = "BOUNDARYHASH";
    let png: Vec<u8> = vec![
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A,
        0x00,0x00,0x00,0x0D, b'I', b'H', b'D', b'R',
        0x00,0x00,0x00,0x01, 0x00,0x00,0x00,0x01, 0x08, 0x06, 0x00,0x00,0x00, 0x1F,0x15,0xC4,0x89,
        0x00,0x00,0x00,0x0A, b'I', b'D', b'A', b'T', 0x78,0x9C, 0x63,0x00,0x01,0x00,0x00,0x05,0x00,0x01, 0x0D,0x0A,0x2D,0xB4,
        0x00,0x00,0x00,0x00, b'I', b'E', b'N', b'D', 0xAE,0x42,0x60,0x82,
    ];
    // build multipart
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(format!("--{}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.png\"\r\nContent-Type: application/octet-stream\r\n\r\n", boundary).as_bytes());
    body.extend_from_slice(&png);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let req = test::TestRequest::post()
        .uri("/api/v1/images")
        .insert_header(("Content-Type", format!("multipart/form-data; boundary={}", boundary)))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let uploaded: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let hash = uploaded["hash"].as_str().unwrap();

    // fetch image
    let req = test::TestRequest::get().uri(&format!("/images/{}", hash)).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert_eq!(ct, "image/png");
}
