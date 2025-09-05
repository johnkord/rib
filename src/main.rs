use actix_web::{web, App, HttpServer, Responder, middleware::Compress};
use actix_cors::Cors;
use utoipa_swagger_ui::SwaggerUi;

mod models;
mod repo;
mod error;
mod routes;
mod openapi;
mod security;
mod auth;
mod storage;

#[cfg(feature = "inmem-store")]
use repo::inmem::InMemRepo;
use routes::{config, AppState};
use storage::build_image_store;
use security::SecurityHeaders;
use openapi::ApiDoc;
use utoipa::OpenApi; // bring trait into scope for ApiDoc::openapi()
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::EnvFilter;
use tracing_actix_web::TracingLogger;
use auth::{Auth, Role};
// `require_role!` is exported to the crate root by auth.rs

async fn moderator_only(auth: Auth) -> actix_web::Result<impl Responder> {
    require_role!(auth, Role::Moderator | Role::Admin);
    // ...handler logic...
    Ok("secret moderator data")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Environment variables must be set externally (VS Code launch.json, shell, systemd, Docker, etc.)
    // Load .env automatically only in debug builds to reduce manual setup overhead.
    if cfg!(debug_assertions) {
        let _ = dotenv::dotenv();
    }
    
    // Validate required environment variables
    validate_env_vars();
    
    // Structured logging initialisation
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(Level::INFO.into()))
        .init();

    info!("Bootstrapping RIB server");
    
    // Log loaded configuration (non-sensitive)
    info!("Discord OAuth configured: {}", 
        std::env::var("DISCORD_CLIENT_ID").is_ok());
    info!("Frontend URL: {}", 
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string()));

    #[cfg(all(feature = "inmem-store", not(feature = "postgres-store")))]
    let repo = InMemRepo::new();
    #[cfg(all(feature = "inmem-store", not(feature = "postgres-store")))]
    info!("Using in-memory repository backend");

    #[cfg(feature = "postgres-store")]
    let repo = {
        use sqlx::postgres::PgPoolOptions;
        let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for postgres-store");
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_lazy(&db_url)
            .expect("Failed to create Pg pool");
        info!("Using Postgres repository backend");
    crate::repo::pg::PgRepo::new(pool)
    };

    let openapi = ApiDoc::openapi();
    let image_store = build_image_store().await; // FS or S3 depending on feature/env
    info!("OpenAPI spec generated");

    let server = HttpServer::new(move || {
        // base application
        let cors = {
            let mut c = Cors::default()
                // during local dev allow React/Vite default ports
                .allowed_origin("http://localhost:5173")
                .allowed_origin("http://127.0.0.1:5173")
                // containerized nginx frontend (served on 3000)
                .allowed_origin("http://localhost:3000")
                .allowed_origin("http://127.0.0.1:3000")
                // allow Swagger UI if served from same origin (actix itself)
                .allow_any_header()
                .allowed_methods(["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"]) // adjust as needed
                .supports_credentials()
                .max_age(3600);
            // If FRONTEND_URL env var is provided and not already covered, add it.
            if let Ok(front) = std::env::var("FRONTEND_URL") {
                c = c.allowed_origin(&front);
            }
            // Accept Authorization & Content-Type explicitly when not using allow_any_header
            // c = c.allowed_headers(vec![header::CONTENT_TYPE, header::AUTHORIZATION]);
            c
        };

        let mut app = App::new()
            .wrap(TracingLogger::default())
            .wrap(Compress::default())
            .wrap(SecurityHeaders::from_env().with_hsts(true)) // use helper
            .wrap(cors)
            .configure(config)
            .service(SwaggerUi::new("/docs").url("/docs/openapi.json", openapi.clone()))
            .route("/mod/secret", web::get().to(moderator_only));

        // inject in-memory repository when the feature is enabled
        // Provide repo (either in-memory or Postgres)
        app = app.app_data(actix_web::web::Data::new(AppState {
            repo: Arc::new(repo.clone()),
            image_store: image_store.clone(),
        }));

        app
    })
    .bind(("0.0.0.0", 8080))?;           // listen on all interfaces so nginx container can reach it

    info!("Listening on http://0.0.0.0:8080 (all interfaces)");

    server.run().await                     // <-- run the server
}

/// Validate that required environment variables are set
fn validate_env_vars() {
    use std::env;
    
    // Required variables that must be set
    let required = vec![
        "JWT_SECRET",
    ];
    
    let mut missing = Vec::new();
    for var in required {
        if env::var(var).is_err() {
            missing.push(var);
        }
    }
    
    if !missing.is_empty() {
        eprintln!("Missing required environment variables: {:?}", missing);
        eprintln!("Please copy .env.example to .env and configure it");
        std::process::exit(1);
    }
    
    // Validate JWT_SECRET is sufficiently long
    if let Ok(secret) = env::var("JWT_SECRET") {
        if secret.len() < 32 {
            eprintln!("JWT_SECRET must be at least 32 characters long for security");
            std::process::exit(1);
        }
    }
    
    // Warn about optional variables for Discord OAuth
    if env::var("DISCORD_CLIENT_ID").is_err() || env::var("DISCORD_CLIENT_SECRET").is_err() {
        eprintln!("Warning: Discord OAuth not configured (DISCORD_CLIENT_ID/DISCORD_CLIENT_SECRET missing)");
        eprintln!("Discord login will not work without these variables");
    }
}
