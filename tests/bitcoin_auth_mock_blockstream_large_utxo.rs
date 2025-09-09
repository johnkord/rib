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

// Uses large provided UTXO set to ensure summation handles many entries & large values.
#[actix_web::test]
#[serial_test::serial]
async fn bitcoin_auth_mocked_large_utxo_balance() {
    let Some(repo) = pg_repo().await else { eprintln!("skip: no DATABASE_URL"); return; };
    ensure_secret();
    // Granular control now; leave sig skipped explicitly below
    std::env::remove_var("BTC_AUTH_TEST_BALANCE_OVERRIDE");
    std::env::set_var("BTC_AUTH_TEST_SKIP_BALANCE", "0");
    std::env::set_var("BTC_AUTH_TEST_SKIP_SIG", "1"); // skip expensive signature path, focus on balance

    let mock_server = MockServer::start().await;
    std::env::set_var("BTC_BLOCKSTREAM_API_BASE", mock_server.uri());

    let address = "bc1qryhgpmfv03qjhhp2dj8nw8g4ewg08jzmgy3cyx";
    // Insert dummy challenge (not validating signature)
    let challenge = format!("Prove you own Bitcoin address {} (nonce testnonce)", address);
    rib::btc_test_insert_challenge(address, &challenge).await;

    // Provided UTXO JSON
    let utxos_json = serde_json::json!([
        {"txid":"8109c8f71f7c5c37f10717274535d0265138b8d73e346980ba81e65f9d10381a","vout":1,"status":{"confirmed":true,"block_height":913832,"block_hash":"00000000000000000001921dd592a5cc4afd3f0fa54d5b44c81db6cbcf80194a","block_time":1757382583},"value":13983},
        {"txid":"564e378c18c35f950b135378ea98ef3577d92d805672b7e86d6b7118231eba13","vout":1,"status":{"confirmed":true,"block_height":913702,"block_hash":"000000000000000000000a0813b0a1930f49f52e9551907612c3d32688f4ddba","block_time":1757318026},"value":8047},
        {"txid":"12e184cb5dae51a4a45423538ce2522dacff98bfb0d2c750357932db868626fa","vout":1,"status":{"confirmed":false},"value":275976284},
        {"txid":"5689437671209e8a3a132c645b73898aa82158f10b96bdf008dde59a826f8ba8","vout":1,"status":{"confirmed":false},"value":366885304},
        {"txid":"8d36d6643ade950ee938f3c38defb8b75cc0afad7ae1eacbdede2170f95a2d68","vout":0,"status":{"confirmed":false},"value":381707750},
        {"txid":"626cbbbc276464764b2c62db4cfb0f23430fc1effaf027aa027a822a7d657d7d","vout":1,"status":{"confirmed":false},"value":438814976},
        {"txid":"d77ab2ea4aa8c2a871c381451f9ac2f9af46909150edd2c753f4b761217a3474","vout":0,"status":{"confirmed":false},"value":476013329},
        {"txid":"5bf35b2db603fc7be31b1119a1f6e6ed744ecdedb3ad9905d02acd099f60bc29","vout":1,"status":{"confirmed":false},"value":758705840},
        {"txid":"231858ffd44ac8767df5cee3396f9b4c4275939cc00f14cfb407c7c2515f8239","vout":0,"status":{"confirmed":false},"value":816463814},
        {"txid":"2a14d70bd954f2879b79f68ee9ba94f684b5403141f1e30f6e75d075e20d0d69","vout":0,"status":{"confirmed":false},"value":971491974},
        {"txid":"5902e9d854feafff91cc9a64d8d252c611a43964ef358bd884d7ce429d4f63c4","vout":0,"status":{"confirmed":false},"value":1076600487},
        {"txid":"47143343fe8f58af44823d5ea2a143f12307c29f6081a7c22397760a8cb7913c","vout":1,"status":{"confirmed":false},"value":3861230529u64},
        {"txid":"f2a0f78016f32e3a2349259e7495c2125c8119915d694b85db7942aebdbbb9c9","vout":0,"status":{"confirmed":false},"value":4906126519u64},
        {"txid":"4a087b924ab36692af4c82658947ee2729671b46f0ce41ad1274baa2d702549c","vout":1,"status":{"confirmed":false},"value":13651842},
        {"txid":"d7e5d00129efef6bbc5036beeb7d6e617030ccf51b81d9d59caa26c8a68e84bf","vout":0,"status":{"confirmed":false},"value":200803871},
        {"txid":"7e52f56d659cfa426320608490148890fbbea1a7bd7766c1c3fe7633d40ca922","vout":0,"status":{"confirmed":false},"value":271986102}
    ]);

    Mock::given(method("GET")).and(path(format!("/address/{}/utxo", address)))
        .respond_with(ResponseTemplate::new(200).set_body_json(utxos_json))
        .mount(&mock_server).await;

    let state = AppState { repo: Arc::new(repo), image_store: Arc::new(MockImageStore::default()), rate_limiter: None };
    let mut app = test::init_service(App::new().app_data(actix_web::web::Data::new(state)).configure(config)).await;

    // Perform verify (signature skipped, balance enforced) - use dummy signature placeholder
    let req = test::TestRequest::post().uri("/api/v1/auth/bitcoin/verify")
        .set_json(&json!({"address": address, "signature": "dummysig"}))
        .to_request();
    let resp = test::call_service(&mut app, req).await;
    assert_eq!(resp.status(), 200, "should succeed because aggregated mocked balance >> min threshold");
    let body: serde_json::Value = serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert!(body.get("token").and_then(|v| v.as_str()).unwrap_or("").len() > 10);
}
