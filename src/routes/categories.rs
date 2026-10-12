use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::json;

use crate::{
    auth::{AdminUser, AppState},
    error::AppError,
    models::{CreateCategoryDto, UpdateCategoryDto},
    services,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/categories", get(list_categories).post(create_category))
        .route(
            "/categories/{id_or_slug}",
            get(get_category)
                .put(update_category)
                .delete(delete_category),
        )
}

async fn list_categories(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let categories = services::categories::list_categories(&state.pool).await?;
    Ok(Json(categories))
}

async fn get_category(
    State(state): State<AppState>,
    Path(id_or_slug): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let category = services::categories::get_category_by_id_or_slug(&state.pool, &id_or_slug).await?;
    match category {
        Some(cat) => Ok(Json(cat)),
        None => Err(AppError::NotFound(format!("Category '{id_or_slug}' not found"))),
    }
}

async fn create_category(
    _admin: AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateCategoryDto>,
) -> Result<impl IntoResponse, AppError> {
    let category = services::categories::create_category(&state.pool, payload).await?;
    Ok((StatusCode::CREATED, Json(category)))
}

async fn update_category(
    _admin: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateCategoryDto>,
) -> Result<impl IntoResponse, AppError> {
    let updated = services::categories::update_category(&state.pool, id, payload).await?;
    Ok(Json(updated))
}

async fn delete_category(
    _admin: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    services::categories::delete_category(&state.pool, id).await?;
    Ok(Json(json!({
        "message": format!("Category {id} successfully deleted"),
        "id": id
    })))
}
