use actix_cors::Cors;
use actix_web::{middleware::Compress, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use utoipa_swagger_ui::SwaggerUi;

use mime;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "embedded-frontend"]
struct EmbeddedFrontend;

fn embedded_file(path: &str) -> Option<(Vec<u8>, mime::Mime)> {
    let p = if path.is_empty() || path == "/" {
        "index.html"
    } else {
        &path[1..]
    }; // trim leading /
    let candidate = EmbeddedFrontend::get(p).or_else(|| {
        // For any unknown path (SPA route), fall back to index.html
        if !p.contains('.') {
            EmbeddedFrontend::get("index.html")
        } else {
            None
        }
    })?;

    // Extension-based mapping (deterministic, avoids mis-sniff for css/js)
    let mime = match std::path::Path::new(p).extension().and_then(|e| e.to_str()) {
        Some("html") | None => mime::TEXT_HTML,
        Some("css") => "text/css".parse().unwrap(),
        Some("js") => "application/javascript".parse().unwrap(),
        Some("mjs") => "application/javascript".parse().unwrap(),
        Some("json") => "application/json".parse().unwrap(),
        Some("svg") => "image/svg+xml".parse().unwrap(),
        Some("png") => "image/png".parse().unwrap(),
        Some("jpg") | Some("jpeg") => "image/jpeg".parse().unwrap(),
        Some("gif") => "image/gif".parse().unwrap(),
        Some("webp") => "image/webp".parse().unwrap(),
        Some("ico") => "image/x-icon".parse().unwrap(),
        Some("map") => "application/json".parse().unwrap(),
        Some("txt") => "text/plain".parse().unwrap(),
        _ => infer::get(&candidate.data)
            .map(|t| t.mime_type().parse().unwrap_or(mime::TEXT_HTML))
            .unwrap_or(mime::TEXT_HTML),
    };

    Some((candidate.data.to_vec(), mime))
}

async fn serve_frontend(req: HttpRequest) -> HttpResponse {
    let path = req.path();
    match embedded_file(path) {
        Some((bytes, mime)) => {
            let cache_header =
                if path.contains("/assets/") || path.ends_with(".js") || path.ends_with(".css") {
                    "public, max-age=31536000, immutable"
                } else {
                    "no-cache"
                };
            HttpResponse::Ok()
                .append_header(("Content-Type", mime.to_string()))
                .append_header(("Cache-Control", cache_header))
                .body(bytes)
        }
        None => HttpResponse::NotFound().finish(),
    }
}

use rib::auth::{Auth, Role};
use rib::require_role; // macro
use rib::openapi::ApiDoc;
use rib::routes::{config, AppState};
use rib::security::SecurityHeaders;
use rib::storage::build_image_store;
use rib::rate_limit::{RateLimitConfig, RateLimiterFacade, InMemoryRateLimiter};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use once_cell::sync::Lazy;
use tracing::{info, Level};
use tracing_actix_web::TracingLogger;
use tracing_subscriber::EnvFilter;
use utoipa::OpenApi; // bring trait into scope for ApiDoc::openapi()
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
    info!(
        "Discord OAuth configured: {}",
        std::env::var("DISCORD_CLIENT_ID").is_ok()
    );
    info!(
        "Frontend URL: {}",
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string())
    );

    // Build Postgres repository (default and only backend now) and run migrations
    let repo = {
        use sqlx::postgres::PgPoolOptions;
        use tokio::time::{sleep, Duration};
        let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let mut attempts = 0u8;
        let pool = loop {
            attempts += 1;
            match PgPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(Duration::from_secs(5))
                .connect(&db_url)
                .await
            {
                Ok(pool) => break pool,
                Err(e) => {
                    if attempts >= 8 {
                        panic!("Failed to connect Pg pool after {attempts} attempts: {e}");
                    }
                    let backoff = 2_u64.pow((attempts - 1) as u32).min(30);
                    eprintln!(
                        "Postgres not ready (attempt {attempts}): {e}; retrying in {backoff}s..."
                    );
                    sleep(Duration::from_secs(backoff)).await;
                }
            }
        };
        if let Err(e) = sqlx::migrate!().run(&pool).await {
            panic!("Database migration failed: {e}");
        }
        info!("Postgres migrations applied");
        info!("Using Postgres repository backend");
    rib::repo::pg::PgRepo::new(pool)
    };

    let openapi = ApiDoc::openapi();
    let image_store = build_image_store().await; // FS or S3 depending on feature/env
    info!("OpenAPI spec generated");

    // Pre-build shared components to move into closure cheaply
    let rl_enabled = std::env::var("RL_ENABLED").map(|v| v == "true" || v == "1").unwrap_or(false);
    let rate_limiter_global = if rl_enabled { Some(RateLimiterFacade::new(InMemoryRateLimiter::new(true), RateLimitConfig::from_env())) } else { None };
    let repo_arc = std::sync::Arc::new(repo);
    let image_store_arc = image_store.clone();
    let openapi_spec = openapi.clone();
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

        // metrics exporter handle clone per worker
        static PROM_HANDLE: Lazy<PrometheusHandle> = Lazy::new(|| {
            PrometheusBuilder::new().install_recorder().expect("install prometheus recorder")
        });
        let prometheus = PROM_HANDLE.clone();
        let mut app = App::new()
            .wrap(TracingLogger::default())
            .wrap(Compress::default())
            .wrap(SecurityHeaders::from_env().with_hsts(true)) // use helper
            .wrap(cors)
            .configure(config)
            .service(SwaggerUi::new("/docs").url("/docs/openapi.json", openapi_spec.clone()))
            .route("/mod/secret", web::get().to(moderator_only))
            .route("/metrics", web::get().to(move || {
                let handle = prometheus.clone();
                async move {
                    let body = handle.render();
                    HttpResponse::Ok().content_type("text/plain; version=0.0.4").body(body)
                }
            }));

        // Catch-all route for SPA assets *after* API & docs so they override only unknown paths.
        app = app.service(
            actix_web::web::resource("/{tail:.*}").route(actix_web::web::get().to(serve_frontend)),
        );

        // inject in-memory repository when the feature is enabled
        // Provide repo (either in-memory or Postgres)
        app = app.app_data(actix_web::web::Data::new(AppState {
            repo: repo_arc.clone(),
            image_store: image_store_arc.clone(),
            rate_limiter: rate_limiter_global.clone(),
        }));

        app
    })
    .bind(("0.0.0.0", 8080))?; // listen on all interfaces so nginx container can reach it

    info!("Listening on http://0.0.0.0:8080 (all interfaces)");

    server.run().await // <-- run the server
}

/// Validate that required environment variables are set
fn validate_env_vars() {
    use std::env;

    // Required variables that must be set
    let required = vec!["JWT_SECRET"];

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
