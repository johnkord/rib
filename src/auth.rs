use actix_web::cookie::{time::Duration as CookieDuration, Cookie, SameSite};
use actix_web::{dev::Payload, Error, FromRequest, HttpRequest};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use base64::Engine;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;
use std::future::{ready, Ready};

pub const AUTH_COOKIE_NAME: &str = "rib_session";
pub const OAUTH_TRANSACTION_COOKIE_NAME: &str = "rib_oauth_transaction";
const OAUTH_TRANSACTION_TTL_MINUTES: i64 = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Moderator,
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub roles: Vec<Role>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OAuthTransactionClaims {
    state: String,
    pkce_verifier: String,
    exp: usize,
}

pub struct OAuthTransactionStart {
    pub state: String,
    pub code_challenge: String,
    pub cookie: Cookie<'static>,
}

fn jwt_secret() -> String {
    env::var("JWT_SECRET").expect("JWT_SECRET not set")
}

fn cookies_secure() -> bool {
    env::var("COOKIE_SECURE")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or_else(|_| {
            env::var("FRONTEND_URL")
                .map(|url| url.starts_with("https://"))
                .unwrap_or(false)
        })
}

fn random_urlsafe(bytes: usize) -> String {
    let mut value = vec![0_u8; bytes];
    rand::thread_rng().fill_bytes(&mut value);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value)
}

pub fn create_oauth_transaction() -> Result<OAuthTransactionStart, jsonwebtoken::errors::Error> {
    let state = random_urlsafe(32);
    let pkce_verifier = random_urlsafe(32);
    let code_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(Sha256::digest(pkce_verifier.as_bytes()));
    let exp = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::minutes(OAUTH_TRANSACTION_TTL_MINUTES))
        .expect("valid OAuth transaction timestamp")
        .timestamp() as usize;
    let transaction_token = encode(
        &Header::default(),
        &OAuthTransactionClaims {
            state: state.clone(),
            pkce_verifier,
            exp,
        },
        &EncodingKey::from_secret(jwt_secret().as_bytes()),
    )?;

    let cookie = Cookie::build(OAUTH_TRANSACTION_COOKIE_NAME, transaction_token)
        .http_only(true)
        .secure(cookies_secure())
        .same_site(SameSite::Lax)
        .path("/api/v1/auth/discord/callback")
        .max_age(CookieDuration::minutes(OAUTH_TRANSACTION_TTL_MINUTES))
        .finish();

    Ok(OAuthTransactionStart {
        state,
        code_challenge,
        cookie,
    })
}

pub fn consume_oauth_transaction(
    transaction_token: &str,
    returned_state: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    let transaction = decode::<OAuthTransactionClaims>(
        transaction_token,
        &DecodingKey::from_secret(jwt_secret().as_bytes()),
        &validation,
    )?
    .claims;

    if transaction.state != returned_state {
        return Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into());
    }

    Ok(transaction.pkce_verifier)
}

pub fn session_cookie(token: &str) -> Cookie<'static> {
    Cookie::build(AUTH_COOKIE_NAME, token.to_owned())
        .http_only(true)
        .secure(cookies_secure())
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(CookieDuration::hours(24))
        .finish()
}

pub fn clear_session_cookie() -> Cookie<'static> {
    Cookie::build(AUTH_COOKIE_NAME, "")
        .http_only(true)
        .secure(cookies_secure())
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(CookieDuration::ZERO)
        .finish()
}

pub fn clear_oauth_transaction_cookie() -> Cookie<'static> {
    Cookie::build(OAUTH_TRANSACTION_COOKIE_NAME, "")
        .http_only(true)
        .secure(cookies_secure())
        .same_site(SameSite::Lax)
        .path("/api/v1/auth/discord/callback")
        .max_age(CookieDuration::ZERO)
        .finish()
}

/// Validate a JWT and return its claims.
fn decode_jwt(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let secret = jwt_secret();
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )?;
    Ok(data.claims)
}

/// Extractor yielding validated `Claims`.
pub struct Auth(pub Claims);

impl FromRequest for Auth {
    type Error = Error;
    type Future = Ready<Result<Self, Error>>;

    fn from_request(req: &HttpRequest, pl: &mut Payload) -> Self::Future {
        // Delegate to BearerAuth to parse the header.
        if let Ok(bearer) = BearerAuth::from_request(req, pl).into_inner() {
            match decode_jwt(bearer.token()) {
                Ok(claims) => return ready(Ok(Auth(claims))),
                Err(_) => return ready(Err(actix_web::error::ErrorUnauthorized("Invalid JWT"))),
            }
        }
        if let Some(cookie) = req.cookie(AUTH_COOKIE_NAME) {
            return match decode_jwt(cookie.value()) {
                Ok(claims) => ready(Ok(Auth(claims))),
                Err(_) => ready(Err(actix_web::error::ErrorUnauthorized("Invalid session"))),
            };
        }
        ready(Err(actix_web::error::ErrorUnauthorized(
            "Authorization required",
        )))
    }
}

/// Helper macro for role-guarding handlers.
#[macro_export]
macro_rules! require_role {
    ($auth:expr, $role:pat) => {
        if !$auth.0.roles.iter().any(|r| matches!(r, $role)) {
            return Err(actix_web::error::ErrorForbidden("Insufficient role"));
        }
    };
}

/// Create a JWT for a user
pub fn create_jwt(
    user_id: &str,
    username: &str,
    roles: Vec<Role>,
) -> Result<String, jsonwebtoken::errors::Error> {
    let secret = jwt_secret();
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        // If user_id already contains a colon we assume caller provided a composite subject (e.g. "btc:addr")
        sub: if user_id.contains(':') {
            user_id.to_string()
        } else {
            format!("{}:{}", user_id, username)
        },
        exp: expiration,
        roles,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

/// Convenience for Bitcoin auth where we just have an address (no username) and want provider prefix
pub fn create_bitcoin_jwt(
    address: &str,
    roles: Vec<Role>,
) -> Result<String, jsonwebtoken::errors::Error> {
    // Subject shape: "btc:<address>"
    create_jwt(&format!("btc:{}", address), address, roles)
}
