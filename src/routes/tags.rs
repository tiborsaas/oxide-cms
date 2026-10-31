use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::json;

use crate::{
    auth::{AdminUser, AppState},
    error::AppError,
    models::{CreateTagDto, TagQuery, UpdateTagDto},
    services,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tags", get(list_tags).post(create_tag))
        .route(
            "/tags/{id_or_slug}",
            get(get_tag)
                .put(update_tag)
                .delete(delete_tag),
        )
}

async fn list_tags(
    State(state): State<AppState>,
    Query(query): Query<TagQuery>,
) -> Result<impl IntoResponse, AppError> {
    let tags = services::tags::list_tags(&state.pool, Some(&query)).await?;
    Ok(Json(tags))
}

async fn get_tag(
    State(state): State<AppState>,
    Path(id_or_slug): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let tag = services::tags::get_tag_by_id_or_slug(&state.pool, &id_or_slug).await?;
    match tag {
        Some(t) => Ok(Json(t)),
        None => Err(AppError::NotFound(format!("Tag '{id_or_slug}' not found"))),
    }
}

async fn create_tag(
    _admin: AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateTagDto>,
) -> Result<impl IntoResponse, AppError> {
    let tag = services::tags::create_tag(&state.pool, payload).await?;
    Ok((StatusCode::CREATED, Json(tag)))
}

async fn update_tag(
    _admin: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateTagDto>,
) -> Result<impl IntoResponse, AppError> {
    let updated = services::tags::update_tag(&state.pool, id, payload).await?;
    Ok(Json(updated))
}

async fn delete_tag(
    _admin: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    services::tags::delete_tag(&state.pool, id).await?;
    Ok(Json(json!({
        "message": format!("Tag {id} successfully deleted"),
        "id": id
    })))
}
