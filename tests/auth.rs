use actix_web::{dev::Payload, test, App, FromRequest};
use rib::{
    auth::{
        consume_oauth_transaction, create_bitcoin_jwt, create_jwt, create_oauth_transaction,
        session_cookie, Auth, Claims, Role,
    },
    require_role,
    routes::auth_me,
};
use std::env;

// Helper that guarantees a sufficiently long secret for tests.
fn set_secret() {
    env::set_var("JWT_SECRET", "test-secret-must-be-32-bytes-long!!");
}

#[actix_web::test]
async fn jwt_roundtrip_ok() {
    set_secret();
    let token = create_jwt("42", "tester", vec![Role::User]).expect("token");
    // The Auth extractor is the public way to validate, so use it here.
    let req = test::TestRequest::default()
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_http_request();
    let mut pl = Payload::None;
    let auth = Auth::from_request(&req, &mut pl).await.expect("extract");
    assert_eq!(auth.0.sub, "42:tester");
    assert!(auth.0.roles.contains(&Role::User));
}

#[actix_web::test]
async fn extractor_rejects_invalid_token() {
    set_secret();
    let req = test::TestRequest::default()
        .insert_header(("Authorization", "Bearer notatoken"))
        .to_http_request();
    let mut pl = Payload::None;
    assert!(Auth::from_request(&req, &mut pl).await.is_err());
}

#[actix_web::test]
async fn extractor_accepts_http_only_session_cookie() {
    set_secret();
    let token = create_jwt("42", "cookie-user", vec![Role::User]).expect("token");
    let cookie = session_cookie(&token);
    assert!(cookie.http_only().unwrap_or(false));

    let req = test::TestRequest::default()
        .cookie(cookie)
        .to_http_request();
    let mut payload = Payload::None;
    let auth = Auth::from_request(&req, &mut payload)
        .await
        .expect("cookie auth");
    assert_eq!(auth.0.sub, "42:cookie-user");
}

#[actix_web::test]
async fn auth_me_returns_null_for_anonymous_session() {
    set_secret();
    let app =
        test::init_service(App::new().route("/api/v1/auth/me", actix_web::web::get().to(auth_me)))
            .await;

    let request = test::TestRequest::get().uri("/api/v1/auth/me").to_request();
    let response = test::call_service(&app, request).await;

    assert!(response.status().is_success());
    let body: serde_json::Value = test::read_body_json(response).await;
    assert!(body.is_null());
}

#[actix_web::test]
async fn auth_me_returns_cookie_session_user() {
    set_secret();
    let token = create_jwt("42", "cookie-user", vec![Role::Moderator]).expect("token");
    let app =
        test::init_service(App::new().route("/api/v1/auth/me", actix_web::web::get().to(auth_me)))
            .await;

    let request = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .cookie(session_cookie(&token))
        .to_request();
    let response = test::call_service(&app, request).await;

    assert!(response.status().is_success());
    let body: serde_json::Value = test::read_body_json(response).await;
    assert_eq!(body["username"], "cookie-user");
    assert_eq!(body["discord_id"], "42");
    assert_eq!(body["role"], "moderator");
}

#[actix_web::test]
async fn oauth_transaction_roundtrip_returns_pkce_verifier() {
    set_secret();
    let transaction = create_oauth_transaction().expect("transaction");
    assert!(transaction.cookie.http_only().unwrap_or(false));
    assert_eq!(transaction.code_challenge.len(), 43);

    let verifier = consume_oauth_transaction(transaction.cookie.value(), &transaction.state)
        .expect("valid transaction");
    assert_eq!(verifier.len(), 43);
}

#[actix_web::test]
async fn oauth_transaction_rejects_mismatched_state() {
    set_secret();
    let transaction = create_oauth_transaction().expect("transaction");
    assert!(consume_oauth_transaction(transaction.cookie.value(), "wrong-state").is_err());
}

#[actix_web::test]
async fn require_role_macro_enforces_roles() {
    // Build Auth instances manually with different roles.
    let admin = Auth(Claims {
        sub: "1:a".into(),
        exp: usize::MAX,
        roles: vec![Role::Admin],
    });
    let user = Auth(Claims {
        sub: "2:u".into(),
        exp: usize::MAX,
        roles: vec![Role::User],
    });

    // Admin passes the guard.
    fn guarded(a: Auth) -> actix_web::Result<()> {
        require_role!(a, Role::Admin | Role::Moderator);
        Ok(())
    }
    assert!(guarded(admin).is_ok());
    assert!(guarded(user).is_err());
}

#[actix_web::test]
async fn bitcoin_jwt_subject_and_roles() {
    set_secret();
    let addr = "1BoatSLRHtKNngkdXEeobR76b53LETtpyT"; // classic example address
    let token = create_bitcoin_jwt(addr, vec![Role::User]).expect("token");
    let req = test::TestRequest::default()
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_http_request();
    let mut pl = Payload::None;
    let auth = Auth::from_request(&req, &mut pl).await.expect("extract");
    assert_eq!(auth.0.sub, format!("btc:{}", addr));
    assert!(auth.0.roles.contains(&Role::User));
}

#[actix_web::test]
async fn create_jwt_preserves_pre_colon_user_id() {
    set_secret();
    // user_id already composite (e.g., btc:addr) - should not append :username again
    let composite = "btc:xyz";
    let token = create_jwt(composite, "ignored_username", vec![Role::Moderator]).expect("token");
    let req = test::TestRequest::default()
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_http_request();
    let mut pl = Payload::None;
    let auth = Auth::from_request(&req, &mut pl).await.expect("extract");
    assert_eq!(
        auth.0.sub, composite,
        "create_jwt should not duplicate colon subjects"
    );
    assert!(auth.0.roles.contains(&Role::Moderator));
}
