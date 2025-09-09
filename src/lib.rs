pub mod auth;
pub mod error;
pub mod models;
pub mod openapi;
pub mod repo;
pub mod routes;
pub mod security;
pub mod storage; // expose storage for routes
pub mod rate_limit; // in-memory rate limiting

// Re-export commonly used items for tests / external users
pub use routes::{config, AppState};
pub use routes::btc_test_insert_challenge;
pub use security::SecurityHeaders;
