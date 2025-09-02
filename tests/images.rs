use actix_web::{test, App};
use rib::config;

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
        0x00,0x00,0x00,0x0D, b'I', b'H', b'D', b'R',
        0x00,0x00,0x00,0x01, 0x00,0x00,0x00,0x01, 0x08, 0x06, 0x00,0x00,0x00, 0x1F,0x15,0xC4,0x89,
        0x00,0x00,0x00,0x0A, b'I', b'D', b'A', b'T', 0x78,0x9C, 0x63,0x00,0x01,0x00,0x00,0x05,0x00,0x01, 0x0D,0x0A,0x2D,0xB4,
        0x00,0x00,0x00,0x00, b'I', b'E', b'N', b'D', 0xAE,0x42,0x60,0x82,
    ]
}

// Invalid (plain text) bytes
fn sample_txt() -> Vec<u8> { b"hello world".to_vec() }

#[actix_web::test]
async fn test_upload_png_ok() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    let app = test::init_service(App::new().configure(config)).await;
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
async fn test_upload_unsupported_type() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    let app = test::init_service(App::new().configure(config)).await;
    let boundary = "BOUNDARYTXT";
    let (ct, body) = build_multipart("file.txt", &sample_txt(), boundary);
    let req = test::TestRequest::post()
        .uri("/api/v1/images")
        .insert_header(("Content-Type", ct))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 415);
}

#[actix_web::test]
async fn test_upload_duplicate() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    let app = test::init_service(App::new().configure(config)).await;
    let png = sample_png();
    let boundary1 = "B1";
    let (ct1, body1) = build_multipart("dup.png", &png, boundary1);
    let req1 = test::TestRequest::post().uri("/api/v1/images").insert_header(("Content-Type", ct1)).set_payload(body1).to_request();
    let resp1 = test::call_service(&app, req1).await;
    assert_eq!(resp1.status(), 201);
    let boundary2 = "B2";
    let (ct2, body2) = build_multipart("dup.png", &png, boundary2);
    let req2 = test::TestRequest::post().uri("/api/v1/images").insert_header(("Content-Type", ct2)).set_payload(body2).to_request();
    let resp2 = test::call_service(&app, req2).await;
    assert_eq!(resp2.status(), 409);
}

#[actix_web::test]
async fn test_upload_size_limit() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    let app = test::init_service(App::new().configure(config)).await;
    let mut big = sample_png();
    // Ensure we exceed 10MB limit (10 * 1024 * 1024 + 1)
    let target = 10 * 1024 * 1024 + 1;
    big.resize(target, 0xAA);
    let boundary = "BIGN";
    let (ct, body) = build_multipart("big.png", &big, boundary);
    let req = test::TestRequest::post().uri("/api/v1/images").insert_header(("Content-Type", ct)).set_payload(body).to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 413);
}
