use actix_web::{test, App};
use rib::{config, SecurityHeaders, AppState};

#[cfg(feature = "inmem-store")]
use rib::repo::inmem::InMemRepo;
use std::sync::Arc;

#[actix_web::test]
async fn test_security_headers_present() {
    std::env::remove_var("ENABLE_HSTS");
    let repo = InMemRepo::new();
    let app = test::init_service(
        App::new()
            .wrap(SecurityHeaders::from_env())
            .app_data(actix_web::web::Data::new(AppState { repo: Arc::new(repo) }))
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
async fn test_hsts_enabled_via_env() {
    let repo = InMemRepo::new();
    let sec = SecurityHeaders::from_env().with_hsts(true);
    let app = test::init_service(
        App::new()
            .wrap(sec)
            .app_data(actix_web::web::Data::new(AppState { repo: Arc::new(repo) }))
            .configure(config)
    ).await;
    let req = test::TestRequest::get().uri("/api/v1/boards").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let headers = resp.headers();
    assert!(headers.get("strict-transport-security").is_some(), "HSTS header missing");
    // no env cleanup needed when using builder
}
