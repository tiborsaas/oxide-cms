use axum::Router;

use crate::auth::AppState;

pub mod auth;
pub mod categories;
pub mod posts;
pub mod tags;

pub fn create_api_router() -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(categories::router())
        .merge(posts::router())
        .merge(tags::router())
}
