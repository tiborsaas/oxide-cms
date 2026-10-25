pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod markdown;
pub mod models;
pub mod routes;
pub mod services;

use axum::{routing::get, Json, Router};
use serde_json::json;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};

pub fn create_app(state: auth::AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let static_service = ServeDir::new("static");

    Router::new()
        .nest_service("/static", static_service)
        .nest("/api", routes::create_api_router())
        .route(
            "/api",
            get(|| async {
                Json(json!({
                    "name": "oxide-cms",
                    "description": "Headless CMS REST API",
                    "endpoints": {
                        "health": "/api/health",
                        "auth_login": "/api/auth/login",
                        "auth_me": "/api/auth/me",
                        "categories": "/api/categories",
                        "posts": "/api/posts"
                    }
                }))
            }),
        )
        .merge(routes::create_web_router())
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
