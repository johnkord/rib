pub mod models;
pub mod repo;
pub mod error;
pub mod routes;
pub mod openapi;
pub mod security;
pub mod auth;          // NEW – needed by routes and other library code

// Re-export commonly used items for tests / external users
pub use routes::{config, AppState};
pub use security::SecurityHeaders;