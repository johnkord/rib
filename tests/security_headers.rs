use actix_web::{test, App, web, HttpResponse};
use rib::{config, SecurityHeaders, AppState};
use rib::storage::FsImageStore;

#[cfg(feature = "inmem-store")]
use rib::repo::inmem::InMemRepo;
use std::sync::Arc;

#[actix_web::test]
#[serial_test::serial]
async fn test_security_headers_present() {
    std::env::remove_var("ENABLE_HSTS");
    let repo = InMemRepo::new();
    let image_store = FsImageStore::new();
    let app = test::init_service(
        App::new()
            .wrap(SecurityHeaders::from_env())
            .app_data(actix_web::web::Data::new(AppState { repo: Arc::new(repo), image_store: Arc::new(image_store) }))
            .configure(config)
    ).await;
    let req = test::TestRequest::get().uri("/api/v1/boards").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let headers = resp.headers();
    assert!(headers.get("content-security-policy").is_some());
    assert_eq!(headers.get("referrer-policy").unwrap(), "no-referrer");
    assert!(headers.get("strict-transport-security").is_none()); // not enabled
}

#[actix_web::test]
#[serial_test::serial]
async fn test_hsts_enabled_via_env() {
    let repo = InMemRepo::new();
    let image_store = FsImageStore::new();
    let sec = SecurityHeaders::from_env().with_hsts(true);
    let app = test::init_service(
        App::new()
            .wrap(sec)
            .app_data(actix_web::web::Data::new(AppState { repo: Arc::new(repo), image_store: Arc::new(image_store) }))
            .configure(config)
    ).await;
    let req = test::TestRequest::get().uri("/api/v1/boards").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let headers = resp.headers();
    assert!(headers.get("strict-transport-security").is_some(), "HSTS header missing");
    // no env cleanup needed when using builder
}

// NEW: ensure ENABLE_HSTS env alone enables header with from_env()
#[actix_web::test]
#[serial_test::serial]
async fn test_env_var_enables_hsts_without_builder_override() {
    std::env::set_var("ENABLE_HSTS", "1");
    let repo = InMemRepo::new();
    let image_store = FsImageStore::new();
    let app = test::init_service(
        App::new()
            .wrap(SecurityHeaders::from_env())
            .app_data(actix_web::web::Data::new(AppState { repo: Arc::new(repo), image_store: Arc::new(image_store) }))
            .configure(config)
    ).await;
    let req = test::TestRequest::get().uri("/api/v1/boards").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    assert!(resp.headers().get("strict-transport-security").is_some());
    std::env::remove_var("ENABLE_HSTS");
}

// NEW: builder override disables HSTS even if env set
#[actix_web::test]
#[serial_test::serial]
async fn test_builder_can_disable_hsts_even_when_env_set() {
    std::env::set_var("ENABLE_HSTS", "true");
    let repo = InMemRepo::new();
    let image_store = FsImageStore::new();
    let app = test::init_service(
        App::new()
            .wrap(SecurityHeaders::from_env().with_hsts(false))
            .app_data(actix_web::web::Data::new(AppState { repo: Arc::new(repo), image_store: Arc::new(image_store) }))
            .configure(config)
    ).await;
    let req = test::TestRequest::get().uri("/api/v1/boards").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    assert!(resp.headers().get("strict-transport-security").is_none());
    std::env::remove_var("ENABLE_HSTS");
}

// NEW: existing CSP header should not be overwritten by middleware
#[actix_web::test]
#[serial_test::serial]
async fn test_existing_csp_header_preserved() {
    let repo = InMemRepo::new();
    let image_store = FsImageStore::new();
    let app = test::init_service(
        App::new()
            .wrap(SecurityHeaders::from_env())
            .app_data(actix_web::web::Data::new(AppState { repo: Arc::new(repo), image_store: Arc::new(image_store) }))
            .route("/custom", web::get().to(|| async {
                HttpResponse::Ok()
                    .insert_header((actix_web::http::header::CONTENT_SECURITY_POLICY, "custom-src 'none'"))
                    .finish()
            }))
    ).await;
    let req = test::TestRequest::get().uri("/custom").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let csp = resp.headers().get("content-security-policy").unwrap().to_str().unwrap();
    assert_eq!(csp, "custom-src 'none'");
}
