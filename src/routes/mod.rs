use axum::Router;

use crate::auth::AppState;

pub mod auth;
pub mod categories;
pub mod posts;

pub fn create_api_router() -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(categories::router())
        .merge(posts::router())
}
