use actix_web::{test, App};
use serde_json::json;
use rib::{config, AppState};
use rib::repo::pg::PgRepo;
use rib::storage::{ImageStore, ImageStoreError};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

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

// This test mocks the Blockstream UTXO endpoint to return two UTXOs totaling 1_500_000 sats, exceeding default min (1_000_000)
// and exercises the real balance HTTP path (no override env, no skip).
#[actix_web::test]
#[serial_test::serial]
async fn bitcoin_auth_mocked_blockstream_balance_allows_login() {
    let Some(repo) = pg_repo().await else { eprintln!("skip: no DATABASE_URL"); return; };
    ensure_secret();
    // Ensure we hit real code paths
    std::env::remove_var("BTC_AUTH_TEST_SKIP_SIG"); // ensure signature path
    std::env::remove_var("BTC_AUTH_TEST_BALANCE_OVERRIDE");
    std::env::set_var("BTC_AUTH_TEST_SKIP_BALANCE", "0");
    std::env::remove_var("BTC_BLOCKSTREAM_API_BASE"); // will set after mock server starts

    // Spin up mock server
    let mock_server = MockServer::start().await;
    // Point code to mock
    std::env::set_var("BTC_BLOCKSTREAM_API_BASE", mock_server.uri());

    // Address / signature pair from existing passing test (segwit v0)
    let address = "bc1qs39xhnvs4fapud7hteh6anyr8dl09e5e8km875";
    let challenge = "Prove you own Bitcoin address bc1qs39xhnvs4fapud7hteh6anyr8dl09e5e8km875 (nonce 3b3820e39138fb903e7e8b3af23039d14e30d0fb4091fdd028aa3eca18fd588c)";
    let signature = "IHzOd42nCJc5yUAWkyh7oHpcL/faTQjE1xEKxsNBBk5hLdk/4h4q6XZA0NhyXnR9qG1ixbxUFpZu0PiAZchANuE=";

    // Mock UTXO response (two entries 1_000_000 + 500_000)
    let utxos = serde_json::json!([
        {"txid":"abcd","vout":0,"value":1000000},
        {"txid":"efgh","vout":1,"value":500000}
    ]);
    Mock::given(method("GET"))
        .and(path(format!("/address/{}/utxo", address)))
        .respond_with(ResponseTemplate::new(200).set_body_json(utxos))
        .mount(&mock_server)
        .await;

    let state = AppState { repo: Arc::new(repo), image_store: Arc::new(MockImageStore::default()), rate_limiter: None };
    let mut app = test::init_service(App::new().app_data(actix_web::web::Data::new(state)).configure(config)).await;

    // Insert challenge into in-memory map
    rib::btc_test_insert_challenge(address, challenge).await;

    // Perform verify request (should succeed since mocked balance >= min)
    let req = test::TestRequest::post().uri("/api/v1/auth/bitcoin/verify")
        .set_json(&json!({"address": address, "signature": signature}))
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), 200, "verify should succeed with mocked sufficient balance");
    let body: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert!(body.get("token").and_then(|v| v.as_str()).unwrap_or("").len() > 10);
}
