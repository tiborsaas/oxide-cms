pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod markdown;
pub mod models;
pub mod routes;
pub mod services;

pub use auth::AppState;
pub use config::AppConfig;
pub use db::init_db;
pub use error::AppError;
pub use markdown::markdown_to_html;
pub use models::{
    Category, CreateCategoryDto, CreatePostDto, Post, PostDetail, PostQuery, UpdateCategoryDto,
    UpdatePostDto,
};
pub use routes::create_api_router;

use axum::{routing::get, Json, Router};
use serde_json::json;
use sqlx::SqlitePool;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

/// Returns the complete REST API Axum router configured for Oxide CMS.
/// Can be nested inside a parent application (e.g., `app.nest("/api", oxide_cms::api_router())`)
pub fn api_router() -> Router<AppState> {
    routes::create_api_router()
}

/// Creates a standalone Axum Router with CORS, JSON info root, and all API routes mounted.
pub fn create_app(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .nest("/api", routes::create_api_router())
        .route(
            "/api",
            get(|| async {
                Json(json!({
                    "name": "oxide-cms",
                    "description": "Headless CMS Library REST API",
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
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// ============================================================================
// Data Loading Interfaces
// ============================================================================

/// Data Loading Interface: List all published articles with optional limit and pagination offset.
pub async fn list_articles(
    pool: &SqlitePool,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<Post>, AppError> {
    let query = PostQuery {
        limit,
        offset,
        ..Default::default()
    };
    services::posts::list_posts(pool, &query, false).await
}

/// Data Loading Interface: Retrieve a single published article by its slug or numeric ID.
pub async fn get_article_by_slug(
    pool: &SqlitePool,
    slug_or_id: &str,
) -> Result<Option<PostDetail>, AppError> {
    services::posts::get_post_by_id_or_slug(pool, slug_or_id, false).await
}

/// Data Loading Interface: List published articles belonging to a specific category slug, with limit and offset.
pub async fn list_articles_by_category(
    pool: &SqlitePool,
    category_slug: &str,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<Post>, AppError> {
    let query = PostQuery {
        category_slug: Some(category_slug.to_string()),
        limit,
        offset,
        ..Default::default()
    };
    services::posts::list_posts(pool, &query, false).await
}

/// Data Loading Interface: List all available categories.
pub async fn list_categories(pool: &SqlitePool) -> Result<Vec<Category>, AppError> {
    services::categories::list_categories(pool).await
}

/// Data Loading Interface: Retrieve a single category by its slug or numeric ID.
pub async fn get_category_by_slug(
    pool: &SqlitePool,
    slug_or_id: &str,
) -> Result<Option<Category>, AppError> {
    services::categories::get_category_by_id_or_slug(pool, slug_or_id).await
}
