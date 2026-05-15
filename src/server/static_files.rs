use axum::Router;
use tower_http::services::{ServeDir, ServeFile};

pub fn static_routes(dist_path: String) -> Router {
    Router::new().nest_service(
        "/",
        ServeDir::new(&dist_path).fallback(ServeFile::new(format!("{}/index.html", dist_path))),
    )
}
