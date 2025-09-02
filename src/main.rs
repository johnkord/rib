use actix_web::{App, HttpServer, middleware::Compress};
use actix_cors::Cors;
use utoipa_swagger_ui::SwaggerUi;

mod models;
mod repo;
mod error;
mod routes;
mod openapi;
mod security;

#[cfg(feature = "inmem-store")]
use repo::inmem::InMemRepo;
use routes::{config, AppState};
use security::SecurityHeaders;
use openapi::ApiDoc;
use utoipa::OpenApi; // bring trait into scope for ApiDoc::openapi()
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::EnvFilter;
use tracing_actix_web::TracingLogger;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Structured logging initialisation
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(Level::INFO.into()))
        .init();

    info!("Bootstrapping RIB server");

    #[cfg(feature = "inmem-store")]
    let repo = InMemRepo::new();
    #[cfg(feature = "inmem-store")]
    info!("Using in-memory repository backend");

    let openapi = ApiDoc::openapi();
    info!("OpenAPI spec generated");

    let server = HttpServer::new(move || {
        // base application
        let cors = {
            let c = Cors::default()
                // during local dev allow React/Vite default ports
                .allowed_origin("http://localhost:5173")
                .allowed_origin("http://127.0.0.1:5173")
                // allow Swagger UI if served from same origin (actix itself)
                .allow_any_header()
                .allowed_methods(["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"]) // adjust as needed
                .supports_credentials()
                .max_age(3600);
            // Accept Authorization & Content-Type explicitly when not using allow_any_header
            // c = c.allowed_headers(vec![header::CONTENT_TYPE, header::AUTHORIZATION]);
            c
        };

        let mut app = App::new()
            .wrap(TracingLogger::default())
            .wrap(Compress::default())
            .wrap(SecurityHeaders::from_env())
            .wrap(cors)
            .configure(config)
            .service(SwaggerUi::new("/docs").url("/docs/openapi.json", openapi.clone()));

        // inject in-memory repository when the feature is enabled
        #[cfg(feature = "inmem-store")]
        {
            app = app.app_data(actix_web::web::Data::new(AppState {
                repo: Arc::new(repo.clone()),
            }));
        }

        app
    })
    .bind(("127.0.0.1", 8080))?;           // <-- keep result in a variable

    info!("Listening on http://127.0.0.1:8080");

    server.run().await                     // <-- run the server
}
