use std::sync::Arc;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
#[cfg(not(feature = "ladybug"))]
mod lbug_shim;
mod observability;
mod routes;
mod services;
mod static_files;
mod store;

use config::ServerConfig;
use routes::WorkspaceState;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = ServerConfig::from_env();
    tracing::info!("Workspace server starting on port {}", config.port);

    // Ensure directories exist
    std::fs::create_dir_all(&config.store_path).ok();
    std::fs::create_dir_all(&config.vault_path).ok();

    // Attempt to open LadybugDB connection (optional — graceful fallback if missing)
    let ladybug_db = store::ladybug::open_ladybug_db(&config.store_path);
    if let Some(ref lb) = ladybug_db {
        tracing::info!(db_path = %lb.path.display(), "LadybugDB graph integration active");
    } else {
        tracing::info!("LadybugDB not found — graph routes will return local-only data");
    }

    let state = Arc::new(WorkspaceState {
        config: config.clone(),
        ladybug_db,
        attempt_store: store::attempt::AttemptStore::new(),
    });

    let api_routes = routes::create_routes(state.clone());
    let static_routes = static_files::static_routes(config.dist_path);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .merge(api_routes)
        .merge(static_routes)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.port))
        .await
        .expect("Failed to bind port");
    tracing::info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.expect("Server failed");
}
