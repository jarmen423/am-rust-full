use std::path::PathBuf;

use axum::Router;
use tower_http::services::{ServeDir, ServeFile};

/// Static UI + SPA fallback (`index.html`) for unknown paths under `dist_path`.
///
/// Uses [`Router::fallback_service`] instead of nesting at `/`, which Axum 0.8+
/// rejects (`Nesting at the root is no longer supported`).
pub fn static_routes(dist_path: String) -> Router {
    let index = PathBuf::from(&dist_path).join("index.html");
    Router::new().fallback_service(
        ServeDir::new(&dist_path).fallback(ServeFile::new(index)),
    )
}
