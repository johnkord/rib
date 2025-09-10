use actix_web::{test, App};
use rib::config;
use rib::repo::pg::PgRepo;
use rib::routes::AppState;
use rib::storage::{ImageStore, ImageStoreError};
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

// ---------------- In-memory Mock ImageStore (tests only) ----------------
#[derive(Default)]
struct MockImageStore {
    inner: Mutex<HashMap<String, (Vec<u8>, String)>>,
}

#[async_trait::async_trait]
impl ImageStore for MockImageStore {
    async fn save(&self, hash: &str, mime: &str, bytes: &[u8]) -> Result<(), ImageStoreError> {
        let mut map = self.inner.lock().unwrap();
        if map.contains_key(hash) {
            return Err(ImageStoreError::Duplicate);
        }
        map.insert(hash.to_string(), (bytes.to_vec(), mime.to_string()));
        Ok(())
    }
    async fn load(&self, hash: &str) -> Result<(Vec<u8>, String), ImageStoreError> {
        let map = self.inner.lock().unwrap();
        map.get(hash).cloned().ok_or(ImageStoreError::NotFound)
    }
    async fn delete(&self, hash: &str) -> Result<(), ImageStoreError> {
        let mut map = self.inner.lock().unwrap();
        map.remove(hash);
        Ok(())
    }
}

// Helper to build a multipart body with provided bytes and filename
fn build_multipart(file_name: &str, bytes: &[u8], boundary: &str) -> (String, Vec<u8>) {
    let mut body: Vec<u8> = Vec::new();
    let disp = format!("--{}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\nContent-Type: application/octet-stream\r\n\r\n", boundary, file_name);
    body.extend_from_slice(disp.as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());
    (format!("multipart/form-data; boundary={}", boundary), body)
}

// Minimal 1x1 PNG (transparent)
fn sample_png() -> Vec<u8> {
    // Pre-generated 1x1 PNG file bytes
    vec![
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, // signature
        0x00, 0x00, 0x00, 0x0D, b'I', b'H', b'D', b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
        0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, b'I',
        b'D', b'A', b'T', 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A,
        0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82,
    ]
}

// Invalid (plain text) bytes - should now be accepted as text/plain
fn sample_txt() -> Vec<u8> {
    b"hello world".to_vec()
}

// Sample PDF header bytes 
fn sample_pdf() -> Vec<u8> {
    // Minimal PDF header that should be detected as application/pdf
    b"%PDF-1.4\n1 0 obj\n<<\n/Type /Catalog\n/Pages 2 0 R\n>>\nendobj\n2 0 obj\n<<\n/Type /Pages\n/Kids [3 0 R]\n/Count 1\n>>\nendobj\n3 0 obj\n<<\n/Type /Page\n/Parent 2 0 R\n/MediaBox [0 0 612 792]\n>>\nendobj\nxref\n0 4\n0000000000 65535 f \n0000000009 00000 n \n0000000074 00000 n \n0000000120 00000 n \ntrailer\n<<\n/Size 4\n/Root 1 0 R\n>>\nstartxref\n179\n%%EOF".to_vec()
}

// Sample ZIP header
fn sample_zip() -> Vec<u8> {
    // Minimal ZIP file with one text file entry
    vec![
        0x50, 0x4B, 0x03, 0x04, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, b't', b'e',
        b's', b't', b'.', b't', b'x', b't', b'h', b'e', b'l', b'l', b'o', 0x50, 0x4B, 0x01,
        0x02, 0x14, 0x00, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x01, 0x00, 0x00, 0x00, 0x00, b't', b'e', b's',
        b't', b'.', b't', b'x', b't', 0x50, 0x4B, 0x05, 0x06, 0x00, 0x00, 0x00, 0x00, 0x01,
        0x00, 0x01, 0x00, 0x36, 0x00, 0x00, 0x00, 0x2B, 0x00, 0x00, 0x00, 0x00, 0x00
    ]
}

#[actix_web::test]
#[serial_test::serial]
async fn test_upload_png_ok() {
    // Using in-memory mock image store; no S3 dependency
    let url = match std::env::var("DATABASE_URL") {
        Ok(u) => u,
        Err(_) => {
            eprintln!("skipping test_upload_png_ok: no DATABASE_URL set");
            return;
        }
    };
    let pool = match PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&url)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipping test_upload_png_ok: db connect failed: {e}");
            return;
        }
    };
    let repo = PgRepo::new(pool);
    let app = test::init_service(
        App::new()
            .app_data(actix_web::web::Data::new(AppState {
                repo: Arc::new(repo),
                image_store: Arc::new(MockImageStore::default()),
                rate_limiter: None,
            }))
            .configure(config),
    )
    .await;
    let boundary = "BOUNDARY123";
    let (ct, body) = build_multipart("img.png", &sample_png(), boundary);
    let req = test::TestRequest::post()
        .uri("/api/v1/images")
        .insert_header(("Content-Type", ct))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let bytes = test::read_body(resp).await;
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["mime"], "image/png");
    assert!(v["hash"].as_str().unwrap().len() == 64);
}

#[actix_web::test]
#[serial_test::serial]
async fn test_upload_text_file_ok() {
    // Using in-memory mock image store; no S3 dependency
    let url = match std::env::var("DATABASE_URL") {
        Ok(u) => u,
        Err(_) => {
            eprintln!("skipping test_upload_text_file_ok: no DATABASE_URL set");
            return;
        }
    };
    let pool = match PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&url)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipping test_upload_text_file_ok: db connect failed: {e}");
            return;
        }
    };
    let repo = PgRepo::new(pool);
    let app = test::init_service(
        App::new()
            .app_data(actix_web::web::Data::new(AppState {
                repo: Arc::new(repo),
                image_store: Arc::new(MockImageStore::default()),
                rate_limiter: None,
            }))
            .configure(config),
    )
    .await;
    let boundary = "BOUNDARY123";
    let (ct, body) = build_multipart("test.txt", &sample_txt(), boundary);
    let req = test::TestRequest::post()
        .uri("/api/v1/images")
        .insert_header(("Content-Type", ct))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let bytes = test::read_body(resp).await;
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["mime"], "text/plain");
    assert!(v["hash"].as_str().unwrap().len() == 64);
}

#[actix_web::test]
#[serial_test::serial]
async fn test_upload_pdf_file_ok() {
    let url = match std::env::var("DATABASE_URL") {
        Ok(u) => u,
        Err(_) => {
            eprintln!("skipping test_upload_pdf_file_ok: no DATABASE_URL set");
            return;
        }
    };
    let pool = match PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&url)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipping test_upload_pdf_file_ok: db connect failed: {e}");
            return;
        }
    };
    let repo = PgRepo::new(pool);
    let app = test::init_service(
        App::new()
            .app_data(actix_web::web::Data::new(AppState {
                repo: Arc::new(repo),
                image_store: Arc::new(MockImageStore::default()),
                rate_limiter: None,
            }))
            .configure(config),
    )
    .await;
    let boundary = "PDFBOUNDARY";
    let (ct, body) = build_multipart("test.pdf", &sample_pdf(), boundary);
    let req = test::TestRequest::post()
        .uri("/api/v1/images")
        .insert_header(("Content-Type", ct))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let bytes = test::read_body(resp).await;
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["mime"], "application/pdf");
    assert!(v["hash"].as_str().unwrap().len() == 64);
}

#[actix_web::test]
#[serial_test::serial]
async fn test_upload_zip_file_ok() {
    let url = match std::env::var("DATABASE_URL") {
        Ok(u) => u,
        Err(_) => {
            eprintln!("skipping test_upload_zip_file_ok: no DATABASE_URL set");
            return;
        }
    };
    let pool = match PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&url)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipping test_upload_zip_file_ok: db connect failed: {e}");
            return;
        }
    };
    let repo = PgRepo::new(pool);
    let app = test::init_service(
        App::new()
            .app_data(actix_web::web::Data::new(AppState {
                repo: Arc::new(repo),
                image_store: Arc::new(MockImageStore::default()),
                rate_limiter: None,
            }))
            .configure(config),
    )
    .await;
    let boundary = "ZIPBOUNDARY";
    let (ct, body) = build_multipart("test.zip", &sample_zip(), boundary);
    let req = test::TestRequest::post()
        .uri("/api/v1/images")
        .insert_header(("Content-Type", ct))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let bytes = test::read_body(resp).await;
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["mime"], "application/zip");
    assert!(v["hash"].as_str().unwrap().len() == 64);
}

#[actix_web::test]
#[serial_test::serial]
async fn test_upload_unsupported_type() {
    // Test with a file type that's intentionally not in ALLOWED_MIME
    let url = match std::env::var("DATABASE_URL") {
        Ok(u) => u,
        Err(_) => {
            eprintln!("skipping test_upload_unsupported_type: no DATABASE_URL set");
            return;
        }
    };
    let pool = match PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&url)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipping test_upload_unsupported_type: db connect failed: {e}");
            return;
        }
    };
    let repo = PgRepo::new(pool);
    let app = test::init_service(
        App::new()
            .app_data(actix_web::web::Data::new(AppState {
                repo: Arc::new(repo),
                image_store: Arc::new(MockImageStore::default()),
                rate_limiter: None,
            }))
            .configure(config),
    )
    .await;
    // Create a fake executable file that should be rejected
    let exe_bytes = vec![0x4D, 0x5A, 0x90, 0x00]; // DOS MZ header 
    let boundary = "EXEBOUNDARY";
    let (ct, body) = build_multipart("test.exe", &exe_bytes, boundary);
    let req = test::TestRequest::post()
        .uri("/api/v1/images")
        .insert_header(("Content-Type", ct))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 415);
}

#[actix_web::test]
#[serial_test::serial]
async fn test_upload_duplicate() {
    // Using in-memory mock image store; no S3 dependency
    let url = match std::env::var("DATABASE_URL") {
        Ok(u) => u,
        Err(_) => {
            eprintln!("skipping test_upload_duplicate: no DATABASE_URL set");
            return;
        }
    };
    let pool = match PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&url)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipping test_upload_duplicate: db connect failed: {e}");
            return;
        }
    };
    let repo = PgRepo::new(pool);
    let app = test::init_service(
        App::new()
            .app_data(actix_web::web::Data::new(AppState {
                repo: Arc::new(repo),
                image_store: Arc::new(MockImageStore::default()),
                rate_limiter: None,
            }))
            .configure(config),
    )
    .await;
    let png = sample_png();
    let boundary1 = "B1";
    let (ct1, body1) = build_multipart("dup.png", &png, boundary1);
    let req1 = test::TestRequest::post()
        .uri("/api/v1/images")
        .insert_header(("Content-Type", ct1))
        .set_payload(body1)
        .to_request();
    let resp1 = test::call_service(&app, req1).await;
    assert_eq!(resp1.status(), 201);
    let boundary2 = "B2";
    let (ct2, body2) = build_multipart("dup.png", &png, boundary2);
    let req2 = test::TestRequest::post()
        .uri("/api/v1/images")
        .insert_header(("Content-Type", ct2))
        .set_payload(body2)
        .to_request();
    let resp2 = test::call_service(&app, req2).await;
    assert_eq!(
        resp2.status(),
        200,
        "duplicate should return 200 OK with payload"
    );
    let body_dup = test::read_body(resp2).await;
    let v: serde_json::Value = serde_json::from_slice(&body_dup).expect("json");
    assert_eq!(v["duplicate"], true, "duplicate flag should be true");
}

#[actix_web::test]
#[serial_test::serial]
async fn test_upload_size_limit() {
    // Using in-memory mock image store; no S3 dependency
    let url = match std::env::var("DATABASE_URL") {
        Ok(u) => u,
        Err(_) => {
            eprintln!("skipping test_upload_size_limit: no DATABASE_URL set");
            return;
        }
    };
    let pool = match PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&url)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipping test_upload_size_limit: db connect failed: {e}");
            return;
        }
    };
    let repo = PgRepo::new(pool);
    let app = test::init_service(
        App::new()
            .app_data(actix_web::web::Data::new(AppState {
                repo: Arc::new(repo),
                image_store: Arc::new(MockImageStore::default()),
                rate_limiter: None,
            }))
            .configure(config),
    )
    .await;
    let mut big = sample_png();
    // Ensure we exceed 25MB limit (25 * 1024 * 1024 + 1)
    let target = 25 * 1024 * 1024 + 1;
    big.resize(target, 0xAA);
    let boundary = "BIGN";
    let (ct, body) = build_multipart("big.png", &big, boundary);
    let req = test::TestRequest::post()
        .uri("/api/v1/images")
        .insert_header(("Content-Type", ct))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 413);
}
