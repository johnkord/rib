use actix_web::{test, App};
use rib::{config, AppState};
use rib::repo::pg::PgRepo;
use rib::storage::{ImageStore, ImageStoreError};
use sqlx::postgres::PgPoolOptions;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use rib::auth::{create_jwt, Role};
use rib::models::{Board, Thread, Reply};
use serde_json::json;

#[derive(Default)]
struct MockImageStore { inner: Mutex<HashMap<String,(Vec<u8>,String)>> }
#[async_trait::async_trait]
impl ImageStore for MockImageStore {
    async fn save(&self, hash:&str, mime:&str, bytes:&[u8]) -> Result<(), ImageStoreError> {
        let mut m=self.inner.lock().unwrap();
        if m.contains_key(hash){ return Err(ImageStoreError::Duplicate); }
        m.insert(hash.to_string(), (bytes.to_vec(), mime.to_string())); Ok(())
    }
    async fn load(&self, hash:&str)->Result<(Vec<u8>,String), ImageStoreError>{
        let m=self.inner.lock().unwrap(); m.get(hash).cloned().ok_or(ImageStoreError::NotFound)
    }
}
async fn pg_repo()->Option<PgRepo>{
    let url=std::env::var("DATABASE_URL").ok()?;
    let pool=PgPoolOptions::new().max_connections(1).acquire_timeout(std::time::Duration::from_secs(5)).connect(&url).await.ok()?;
    Some(PgRepo::new(pool))
}
fn ensure_secret(){ if std::env::var("JWT_SECRET").is_err(){ std::env::set_var("JWT_SECRET", "testsecret"); }}
fn admin_token()->String{ ensure_secret(); create_jwt("admin","admin", vec![Role::Admin]).unwrap() }
fn user_token()->String{ ensure_secret(); create_jwt("user","user", vec![Role::User]).unwrap() }
fn uniq(prefix:&str)->String{ use std::time::{SystemTime,UNIX_EPOCH}; let ns=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(); format!("{prefix}{ns}") }

// Test 1 & 2: Board soft delete and restore visibility
#[actix_web::test]
#[serial_test::serial]
async fn test_board_soft_delete_and_restore(){
    let Some(repo)=pg_repo().await else { eprintln!("skip: no DATABASE_URL"); return }; 
    let app_state = AppState { repo: Arc::new(repo), image_store: Arc::new(MockImageStore::default()) };
    let mut app = test::init_service(App::new().app_data(actix_web::web::Data::new(app_state)).configure(config)).await;
    let admin=admin_token(); let user=user_token(); let slug=uniq("bd-");

    // create board
    let req = test::TestRequest::post().uri("/api/v1/boards")
        .insert_header(("Authorization", format!("Bearer {admin}")))
        .set_json(&json!({"slug":slug,"title":"Temp"}))
        .to_request();
    let resp = test::call_service(&mut app, req).await; assert_eq!(resp.status(),201);
    let board:Board = serde_json::from_slice(&test::read_body(resp).await).unwrap();

    // visible to user
    let req = test::TestRequest::get().uri("/api/v1/boards").insert_header(("Authorization", format!("Bearer {user}"))).to_request();
    let resp = test::call_service(&mut app, req).await; assert!(resp.status().is_success());
    let boards:Vec<Board>=serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert!(boards.iter().any(|b| b.id==board.id));

    // soft delete
    let req = test::TestRequest::post().uri(&format!("/api/v1/admin/boards/{}/soft-delete", board.id)).insert_header(("Authorization", format!("Bearer {admin}"))).to_request();
    let resp = test::call_service(&mut app, req).await; assert_eq!(resp.status(),200);

    // user list should hide
    let req = test::TestRequest::get().uri("/api/v1/boards").insert_header(("Authorization", format!("Bearer {user}"))).to_request();
    let resp = test::call_service(&mut app, req).await; let boards:Vec<Board>=serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert!(!boards.iter().any(|b| b.id==board.id));

    // admin include_deleted sees with deleted_at
    let req = test::TestRequest::get().uri("/api/v1/boards?include_deleted=1").insert_header(("Authorization", format!("Bearer {admin}"))).to_request();
    let resp = test::call_service(&mut app, req).await; let boards:Vec<Board>=serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert!(boards.iter().find(|b| b.id==board.id).unwrap().deleted_at.is_some());

    // restore
    let req = test::TestRequest::post().uri(&format!("/api/v1/admin/boards/{}/restore", board.id)).insert_header(("Authorization", format!("Bearer {admin}"))).to_request();
    let resp = test::call_service(&mut app, req).await; assert_eq!(resp.status(),200);

    // user sees again
    let req = test::TestRequest::get().uri("/api/v1/boards").insert_header(("Authorization", format!("Bearer {user}"))).to_request();
    let resp = test::call_service(&mut app, req).await; let boards:Vec<Board>=serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert!(boards.iter().any(|b| b.id==board.id));
}

// Test 3 & 6: Thread soft then hard delete
#[actix_web::test]
#[serial_test::serial]
async fn test_thread_soft_then_hard_delete(){
    let Some(repo)=pg_repo().await else { eprintln!("skip: no DATABASE_URL"); return }; 
    let app_state = AppState { repo: Arc::new(repo), image_store: Arc::new(MockImageStore::default()) };
    let mut app = test::init_service(App::new().app_data(actix_web::web::Data::new(app_state)).configure(config)).await;
    let admin=admin_token(); let user=user_token();

    // board
    let req = test::TestRequest::post().uri("/api/v1/boards")
        .insert_header(("Authorization", format!("Bearer {admin}")))
        .set_json(&json!({"slug":uniq("tb-"),"title":"ThreadBoard"}))
        .to_request();
    let resp = test::call_service(&mut app, req).await; let board:Board=serde_json::from_slice(&test::read_body(resp).await).unwrap();

    // thread by user
    let req = test::TestRequest::post().uri("/api/v1/threads")
        .insert_header(("Authorization", format!("Bearer {user}")))
        .set_json(&json!({"board_id":board.id,"subject":"S","body":"B"}))
        .to_request();
    let resp = test::call_service(&mut app, req).await; assert_eq!(resp.status(),201); let thread:Thread=serde_json::from_slice(&test::read_body(resp).await).unwrap();

    // soft delete thread
    let req = test::TestRequest::post().uri(&format!("/api/v1/admin/threads/{}/soft-delete", thread.id)).insert_header(("Authorization", format!("Bearer {admin}"))).to_request();
    let resp = test::call_service(&mut app, req).await; assert_eq!(resp.status(),200);

    // user fetch -> 404
    let req = test::TestRequest::get().uri(&format!("/api/v1/threads/{}", thread.id)).insert_header(("Authorization", format!("Bearer {user}"))).to_request();
    let resp = test::call_service(&mut app, req).await; assert_eq!(resp.status(),404);

    // admin include_deleted -> 200 + deleted_at
    let req = test::TestRequest::get().uri(&format!("/api/v1/threads/{}?include_deleted=1", thread.id)).insert_header(("Authorization", format!("Bearer {admin}"))).to_request();
    let resp = test::call_service(&mut app, req).await; assert_eq!(resp.status(),200); let t:Thread=serde_json::from_slice(&test::read_body(resp).await).unwrap(); assert!(t.deleted_at.is_some());

    // hard delete
    let req = test::TestRequest::delete().uri(&format!("/api/v1/admin/threads/{}", thread.id)).insert_header(("Authorization", format!("Bearer {admin}"))).to_request();
    let resp = test::call_service(&mut app, req).await; assert_eq!(resp.status(),204);

    // admin fetch include_deleted -> 404
    let req = test::TestRequest::get().uri(&format!("/api/v1/threads/{}?include_deleted=1", thread.id)).insert_header(("Authorization", format!("Bearer {admin}"))).to_request();
    let resp = test::call_service(&mut app, req).await; assert_eq!(resp.status(),404);
}

// Test 4,7,8: Reply soft delete visibility
#[actix_web::test]
#[serial_test::serial]
async fn test_reply_soft_delete_visibility(){
    let Some(repo)=pg_repo().await else { eprintln!("skip: no DATABASE_URL"); return }; 
    let app_state = AppState { repo: Arc::new(repo), image_store: Arc::new(MockImageStore::default()) };
    let mut app = test::init_service(App::new().app_data(actix_web::web::Data::new(app_state)).configure(config)).await;
    let admin=admin_token(); let user=user_token();

    // board
    let req = test::TestRequest::post().uri("/api/v1/boards")
        .insert_header(("Authorization", format!("Bearer {admin}")))
        .set_json(&json!({"slug":uniq("rb-"),"title":"ReplyBoard"}))
        .to_request();
    let resp = test::call_service(&mut app, req).await; let board:Board=serde_json::from_slice(&test::read_body(resp).await).unwrap();

    // thread
    let req = test::TestRequest::post().uri("/api/v1/threads")
        .insert_header(("Authorization", format!("Bearer {user}")))
        .set_json(&json!({"board_id":board.id,"subject":"S","body":"B"}))
        .to_request();
    let resp = test::call_service(&mut app, req).await; let thread:Thread=serde_json::from_slice(&test::read_body(resp).await).unwrap();

    // reply
    let req = test::TestRequest::post().uri("/api/v1/replies")
        .insert_header(("Authorization", format!("Bearer {user}")))
        .set_json(&json!({"thread_id":thread.id,"content":"Hi"}))
        .to_request();
    let resp = test::call_service(&mut app, req).await; let reply:Reply=serde_json::from_slice(&test::read_body(resp).await).unwrap();

    // soft delete reply
    let req = test::TestRequest::post().uri(&format!("/api/v1/admin/replies/{}/soft-delete", reply.id)).insert_header(("Authorization", format!("Bearer {admin}"))).to_request();
    let resp = test::call_service(&mut app, req).await; assert_eq!(resp.status(),200);

    // user list replies -> not present
    let req = test::TestRequest::get().uri(&format!("/api/v1/threads/{}/replies", thread.id)).insert_header(("Authorization", format!("Bearer {user}"))).to_request();
    let resp = test::call_service(&mut app, req).await; let replies:Vec<Reply>=serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert!(replies.iter().all(|r| r.id!=reply.id));

    // admin include_deleted -> present with deleted_at
    let req = test::TestRequest::get().uri(&format!("/api/v1/threads/{}/replies?include_deleted=1", thread.id)).insert_header(("Authorization", format!("Bearer {admin}"))).to_request();
    let resp = test::call_service(&mut app, req).await; let replies:Vec<Reply>=serde_json::from_slice(&test::read_body(resp).await).unwrap();
    assert!(replies.iter().find(|r| r.id==reply.id).unwrap().deleted_at.is_some());
}

// Test 5: Creating thread blocked if board soft deleted
#[actix_web::test]
#[serial_test::serial]
async fn test_create_thread_blocked_by_soft_deleted_board(){
    let Some(repo)=pg_repo().await else { eprintln!("skip: no DATABASE_URL"); return }; 
    let app_state = AppState { repo: Arc::new(repo), image_store: Arc::new(MockImageStore::default()) };
    let mut app = test::init_service(App::new().app_data(actix_web::web::Data::new(app_state)).configure(config)).await;
    let admin=admin_token(); let user=user_token();
    // create board
    let req = test::TestRequest::post().uri("/api/v1/boards")
        .insert_header(("Authorization", format!("Bearer {admin}")))
        .set_json(&json!({"slug":uniq("sb-"),"title":"SoftBoard"}))
        .to_request();
    let resp = test::call_service(&mut app, req).await; let board:Board=serde_json::from_slice(&test::read_body(resp).await).unwrap();
    // soft delete board
    let req = test::TestRequest::post().uri(&format!("/api/v1/admin/boards/{}/soft-delete", board.id)).insert_header(("Authorization", format!("Bearer {admin}"))).to_request();
    let resp = test::call_service(&mut app, req).await; assert_eq!(resp.status(),200);
    // attempt thread creation
    let req = test::TestRequest::post().uri("/api/v1/threads")
        .insert_header(("Authorization", format!("Bearer {user}")))
        .set_json(&json!({"board_id":board.id,"subject":"S","body":"B"}))
        .to_request();
    let resp = test::call_service(&mut app, req).await; assert_eq!(resp.status(),404);
}

// Test 10: Idempotent soft delete retains timestamp
#[actix_web::test]
#[serial_test::serial]
async fn test_soft_delete_idempotent(){
    let Some(repo)=pg_repo().await else { eprintln!("skip: no DATABASE_URL"); return }; 
    let app_state = AppState { repo: Arc::new(repo), image_store: Arc::new(MockImageStore::default()) };
    let mut app = test::init_service(App::new().app_data(actix_web::web::Data::new(app_state)).configure(config)).await;
    let admin=admin_token();
    // create board
    let req = test::TestRequest::post().uri("/api/v1/boards")
        .insert_header(("Authorization", format!("Bearer {admin}")))
        .set_json(&json!({"slug":uniq("idem-"),"title":"Idem"}))
        .to_request();
    let resp = test::call_service(&mut app, req).await; let board:Board=serde_json::from_slice(&test::read_body(resp).await).unwrap();
    // first soft delete
    let req = test::TestRequest::post().uri(&format!("/api/v1/admin/boards/{}/soft-delete", board.id)).insert_header(("Authorization", format!("Bearer {admin}"))).to_request();
    let _ = test::call_service(&mut app, req).await;
    let req = test::TestRequest::get().uri("/api/v1/boards?include_deleted=1").insert_header(("Authorization", format!("Bearer {admin}"))).to_request();
    let resp = test::call_service(&mut app, req).await; let boards:Vec<Board>=serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let first_ts = boards.iter().find(|b| b.id==board.id).unwrap().deleted_at;
    assert!(first_ts.is_some());
    // second soft delete (idempotent)
    let req = test::TestRequest::post().uri(&format!("/api/v1/admin/boards/{}/soft-delete", board.id)).insert_header(("Authorization", format!("Bearer {admin}"))).to_request();
    let _ = test::call_service(&mut app, req).await;
    let req = test::TestRequest::get().uri("/api/v1/boards?include_deleted=1").insert_header(("Authorization", format!("Bearer {admin}"))).to_request();
    let resp = test::call_service(&mut app, req).await; let boards:Vec<Board>=serde_json::from_slice(&test::read_body(resp).await).unwrap();
    let second_ts = boards.iter().find(|b| b.id==board.id).unwrap().deleted_at;
    assert_eq!(first_ts, second_ts);
}
