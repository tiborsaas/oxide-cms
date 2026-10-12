use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::json;

use crate::{
    auth::{AdminUser, AppState, OptionalAdminUser},
    error::AppError,
    models::{CreatePostDto, PostQuery, UpdatePostDto},
    services,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/posts", get(list_posts).post(create_post))
        .route(
            "/posts/{id_or_slug}",
            get(get_post).put(update_post).delete(delete_post),
        )
}

async fn list_posts(
    optional_admin: OptionalAdminUser,
    State(state): State<AppState>,
    Query(query): Query<PostQuery>,
) -> Result<impl IntoResponse, AppError> {
    let is_admin = optional_admin.0.is_some();
    let posts = services::posts::list_posts(&state.pool, &query, is_admin).await?;
    Ok(Json(posts))
}

async fn get_post(
    optional_admin: OptionalAdminUser,
    State(state): State<AppState>,
    Path(id_or_slug): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let is_admin = optional_admin.0.is_some();
    let post = services::posts::get_post_by_id_or_slug(&state.pool, &id_or_slug, is_admin).await?;

    match post {
        Some(p) => Ok(Json(p)),
        None => Err(AppError::NotFound(format!("Post '{id_or_slug}' not found"))),
    }
}

async fn create_post(
    _admin: AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<CreatePostDto>,
) -> Result<impl IntoResponse, AppError> {
    let post = services::posts::create_post(&state.pool, payload).await?;
    Ok((StatusCode::CREATED, Json(post)))
}

async fn update_post(
    _admin: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdatePostDto>,
) -> Result<impl IntoResponse, AppError> {
    let updated = services::posts::update_post(&state.pool, id, payload).await?;
    Ok(Json(updated))
}

async fn delete_post(
    _admin: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    services::posts::delete_post(&state.pool, id).await?;
    Ok(Json(json!({
        "message": format!("Post {id} successfully deleted"),
        "id": id
    })))
}
