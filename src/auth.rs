use actix_web::{dev::Payload, Error, FromRequest, HttpRequest};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::env;
use std::future::{ready, Ready};

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

/// Validate a JWT and return its claims.
fn decode_jwt(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let secret = env::var("JWT_SECRET").expect("JWT_SECRET not set");
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
    let secret = env::var("JWT_SECRET").expect("JWT_SECRET not set");
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        // If user_id already contains a colon we assume caller provided a composite subject (e.g. "btc:addr")
        sub: if user_id.contains(':') { user_id.to_string() } else { format!("{}:{}", user_id, username) },
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
pub fn create_bitcoin_jwt(address: &str, roles: Vec<Role>) -> Result<String, jsonwebtoken::errors::Error> {
    // Subject shape: "btc:<address>"
    create_jwt(&format!("btc:{}", address), address, roles)
}
