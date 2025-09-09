use actix_web::{test, web, App, HttpResponse};
use rib::repo::pg::PgRepo;
use rib::storage::{ImageStore, ImageStoreError};
use rib::{config, AppState, SecurityHeaders};
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

// --- In-memory mock image store (shared with images tests) ---
#[derive(Default)]
struct MockImageStore {
    inner: Mutex<HashMap<String, (Vec<u8>, String)>>,
}

#[async_trait::async_trait]
impl ImageStore for MockImageStore {
    async fn save(&self, hash: &str, mime: &str, bytes: &[u8]) -> Result<(), ImageStoreError> {
        let mut m = self.inner.lock().unwrap();
        if m.contains_key(hash) {
            return Err(ImageStoreError::Duplicate);
        }
        m.insert(hash.to_string(), (bytes.to_vec(), mime.to_string()));
        Ok(())
    }
    async fn load(&self, hash: &str) -> Result<(Vec<u8>, String), ImageStoreError> {
        let m = self.inner.lock().unwrap();
        m.get(hash).cloned().ok_or(ImageStoreError::NotFound)
    }
    async fn delete(&self, hash: &str) -> Result<(), ImageStoreError> {
        let mut m = self.inner.lock().unwrap();
        m.remove(hash);
        Ok(())
    }
}

async fn test_repo() -> Option<PgRepo> {
    let url = match std::env::var("DATABASE_URL") {
        Ok(u) => u,
        Err(_) => return None,
    };
    let pool = match PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&url)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skip: db connect failed: {e}");
            return None;
        }
    };
    Some(PgRepo::new(pool))
}

#[actix_web::test]
#[serial_test::serial]
async fn test_security_headers_present() {
    std::env::remove_var("ENABLE_HSTS");
    let Some(repo) = test_repo().await else {
        eprintln!("skip: no DATABASE_URL");
        return;
    };
    let image_store = Arc::new(MockImageStore::default());
    let app = test::init_service(
        App::new()
            .wrap(SecurityHeaders::from_env())
            .app_data(actix_web::web::Data::new(AppState {
                repo: Arc::new(repo),
                image_store: image_store,
                rate_limiter: None,
            }))
            .configure(config),
    )
    .await;
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
    let Some(repo) = test_repo().await else {
        eprintln!("skip: no DATABASE_URL");
        return;
    };
    let image_store = Arc::new(MockImageStore::default());
    let sec = SecurityHeaders::from_env().with_hsts(true);
    let app = test::init_service(
        App::new()
            .wrap(sec)
            .app_data(actix_web::web::Data::new(AppState {
                repo: Arc::new(repo),
                image_store: image_store,
                rate_limiter: None,
            }))
            .configure(config),
    )
    .await;
    let req = test::TestRequest::get().uri("/api/v1/boards").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let headers = resp.headers();
    assert!(
        headers.get("strict-transport-security").is_some(),
        "HSTS header missing"
    );
    // no env cleanup needed when using builder
}

// Ensure ENABLE_HSTS env alone enables header with from_env()
#[actix_web::test]
#[serial_test::serial]
async fn test_env_var_enables_hsts_without_builder_override() {
    std::env::set_var("ENABLE_HSTS", "1");
    let Some(repo) = test_repo().await else {
        eprintln!("skip: no DATABASE_URL");
        return;
    };
    let image_store = Arc::new(MockImageStore::default());
    let app = test::init_service(
        App::new()
            .wrap(SecurityHeaders::from_env())
            .app_data(actix_web::web::Data::new(AppState {
                repo: Arc::new(repo),
                image_store: image_store,
                rate_limiter: None,
            }))
            .configure(config),
    )
    .await;
    let req = test::TestRequest::get().uri("/api/v1/boards").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    assert!(resp.headers().get("strict-transport-security").is_some());
    std::env::remove_var("ENABLE_HSTS");
}

// Builder override disables HSTS even if env set
#[actix_web::test]
#[serial_test::serial]
async fn test_builder_can_disable_hsts_even_when_env_set() {
    std::env::set_var("ENABLE_HSTS", "true");
    let Some(repo) = test_repo().await else {
        eprintln!("skip: no DATABASE_URL");
        return;
    };
    let image_store = Arc::new(MockImageStore::default());
    let app = test::init_service(
        App::new()
            .wrap(SecurityHeaders::from_env().with_hsts(false))
            .app_data(actix_web::web::Data::new(AppState {
                repo: Arc::new(repo),
                image_store: image_store,
                rate_limiter: None,
            }))
            .configure(config),
    )
    .await;
    let req = test::TestRequest::get().uri("/api/v1/boards").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    assert!(resp.headers().get("strict-transport-security").is_none());
    std::env::remove_var("ENABLE_HSTS");
}

// Existing CSP header should not be overwritten by middleware
#[actix_web::test]
#[serial_test::serial]
async fn test_existing_csp_header_preserved() {
    let Some(repo) = test_repo().await else {
        eprintln!("skip: no DATABASE_URL");
        return;
    };
    let image_store = Arc::new(MockImageStore::default());
    let app = test::init_service(
        App::new()
            .wrap(SecurityHeaders::from_env())
            .app_data(actix_web::web::Data::new(AppState {
                repo: Arc::new(repo),
                image_store: image_store,
                rate_limiter: None,
            }))
            .route(
                "/custom",
                web::get().to(|| async {
                    HttpResponse::Ok()
                        .insert_header((
                            actix_web::http::header::CONTENT_SECURITY_POLICY,
                            "custom-src 'none'",
                        ))
                        .finish()
                }),
            ),
    )
    .await;
    let req = test::TestRequest::get().uri("/custom").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let csp = resp
        .headers()
        .get("content-security-policy")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(csp, "custom-src 'none'");
}
