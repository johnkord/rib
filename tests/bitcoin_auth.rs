use actix_web::{test, App};
use serde_json::json;
use rib::{config, AppState};
use rib::repo::pg::PgRepo;
use rib::storage::{ImageStore, ImageStoreError};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

#[derive(Default)]
struct MockImageStore { inner: Mutex<HashMap<String,(Vec<u8>,String)>> }
#[async_trait::async_trait]
impl ImageStore for MockImageStore {
    async fn save(&self, hash:&str, mime:&str, bytes:&[u8]) -> Result<(), ImageStoreError> { let mut m = self.inner.lock().unwrap(); if m.contains_key(hash){return Err(ImageStoreError::Duplicate);} m.insert(hash.to_string(), (bytes.to_vec(), mime.to_string())); Ok(()) }
    async fn load(&self, hash:&str) -> Result<(Vec<u8>, String), ImageStoreError> { let m = self.inner.lock().unwrap(); m.get(hash).cloned().ok_or(ImageStoreError::NotFound) }
    async fn delete(&self, hash:&str) -> Result<(), ImageStoreError> { let mut m = self.inner.lock().unwrap(); m.remove(hash); Ok(()) }
}

async fn pg_repo() -> Option<PgRepo> { let url = std::env::var("DATABASE_URL").ok()?; let pool = sqlx::postgres::PgPoolOptions::new().max_connections(1).acquire_timeout(std::time::Duration::from_secs(5)).connect(&url).await.ok()?; Some(PgRepo::new(pool)) }

fn ensure_secret() { if std::env::var("JWT_SECRET").is_err() { std::env::set_var("JWT_SECRET", "testsecret-abcdefghijklmnopqrstuvwxyz012345"); } }

#[actix_web::test]
#[serial_test::serial]
async fn bitcoin_auth_happy_path_with_test_bypass() {
    // Skip if no DB (requires migrations for repo init)
    let Some(repo) = pg_repo().await else { eprintln!("skip: no DATABASE_URL"); return; };
    ensure_secret();
    // Bypass both signature + balance by setting both granular skips
    std::env::set_var("BTC_AUTH_TEST_SKIP_SIG", "1");
    std::env::set_var("BTC_AUTH_TEST_SKIP_BALANCE", "1");
    let state = AppState { repo: Arc::new(repo), image_store: Arc::new(MockImageStore::default()), rate_limiter: None };
    let mut app = test::init_service(App::new().app_data(actix_web::web::Data::new(state)).configure(config)).await;

    let address = "1BoatSLRHtKNngkdXEeobR76b53LETtpyT"; // deterministic test address
    // Request challenge
    let req = test::TestRequest::post().uri("/api/v1/auth/bitcoin/challenge").set_json(&json!({"address": address})).to_request();
    let resp = test::call_service(&mut app, req).await; assert_eq!(resp.status(), 200);
    let body: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let challenge = body.get("challenge").and_then(|v| v.as_str()).expect("challenge str");
    assert!(challenge.contains(address));

    // Verify (signature bypassed, send dummy)
    let req = test::TestRequest::post().uri("/api/v1/auth/bitcoin/verify").set_json(&json!({"address": address, "signature": "dummy"})).to_request();
    let resp = test::call_service(&mut app, req).await; assert_eq!(resp.status(), 200, "verify should succeed with bypass");
    let body: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let token = body.get("token").and_then(|v| v.as_str()).expect("token");
    assert!(token.len() > 20);
}

#[actix_web::test]
#[serial_test::serial]
async fn bitcoin_auth_verify_bech32_real_signature() {
    // Skip if no DB
    let Some(repo) = pg_repo().await else { eprintln!("skip: no DATABASE_URL"); return; };
    ensure_secret();
    // Ensure we exercise signature path (not bypass) but skip external balance HTTP
    std::env::remove_var("BTC_AUTH_TEST_SKIP_SIG");
    std::env::set_var("BTC_AUTH_TEST_SKIP_BALANCE", "1"); // skip external balance HTTP
    let state = AppState { repo: Arc::new(repo), image_store: Arc::new(MockImageStore::default()), rate_limiter: None };
    let mut app = test::init_service(App::new().app_data(actix_web::web::Data::new(state)).configure(config)).await;

    let address = "bc1qs39xhnvs4fapud7hteh6anyr8dl09e5e8km875";
    let challenge = "Prove you own Bitcoin address bc1qs39xhnvs4fapud7hteh6anyr8dl09e5e8km875 (nonce 3b3820e39138fb903e7e8b3af23039d14e30d0fb4091fdd028aa3eca18fd588c)";
    let signature = "IHzOd42nCJc5yUAWkyh7oHpcL/faTQjE1xEKxsNBBk5hLdk/4h4q6XZA0NhyXnR9qG1ixbxUFpZu0PiAZchANuE=";

    // Insert deterministic challenge into server state
    rib::btc_test_insert_challenge(address, challenge).await;

    // Call verify directly
    let req = test::TestRequest::post().uri("/api/v1/auth/bitcoin/verify").set_json(&json!({"address": address, "signature": signature})).to_request();
    let resp = test::call_service(&mut app, req).await; assert_eq!(resp.status(), 200, "verify should succeed for provided bech32 signature");
    let body: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let token = body.get("token").and_then(|v| v.as_str()).expect("token");
    assert!(token.starts_with("ey")); // JWT header base64
}

#[actix_web::test]
#[serial_test::serial]
async fn bitcoin_auth_verify_bech32_real_signature_case2() {
    // Skip if no DB
    let Some(repo) = pg_repo().await else { eprintln!("skip: no DATABASE_URL"); return; };
    ensure_secret();
    // Exercise real signature path, skip external balance HTTP
    std::env::remove_var("BTC_AUTH_TEST_SKIP_SIG");
    std::env::set_var("BTC_AUTH_TEST_SKIP_BALANCE", "1"); // skip external balance HTTP
    let state = AppState { repo: Arc::new(repo), image_store: Arc::new(MockImageStore::default()), rate_limiter: None };
    let mut app = test::init_service(App::new().app_data(actix_web::web::Data::new(state)).configure(config)).await;

    let address = "bc1qxt49tjg3qyd0dfcesvdkzgy0c62yh0kclpw5gt";
    let challenge = "Prove you own Bitcoin address bc1qxt49tjg3qyd0dfcesvdkzgy0c62yh0kclpw5gt (nonce 18f30f31d65c2ee53bfb73ebf2cf90e9d793090cdc3666e7a837a98618650ec6)";
    let signature = "H28QECJu7lU/lnlfrQ7unqxgg8OzrLg7EePTK4/qi4gTOUCrfKQxgA9Dt09Eyxi313b6MBMpMlSKvFSYg0ldg2I=";

    // Insert deterministic challenge into server state to match signature provided
    rib::btc_test_insert_challenge(address, challenge).await;

    // Call verify directly
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/bitcoin/verify")
        .set_json(&json!({"address": address, "signature": signature}))
        .to_request();
    let resp = test::call_service(&mut app, req).await; assert_eq!(resp.status(), 200, "verify should succeed for provided bech32 signature case2");
    let body: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let token = body.get("token").and_then(|v| v.as_str()).expect("token");
    assert!(token.starts_with("ey")); // JWT header base64
}
