use actix_web::{dev::Payload, test, FromRequest};
use rib::{
    auth::{create_jwt, create_bitcoin_jwt, Auth, Claims, Role},
    require_role,
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
    assert_eq!(auth.0.sub, composite, "create_jwt should not duplicate colon subjects");
    assert!(auth.0.roles.contains(&Role::Moderator));
}
