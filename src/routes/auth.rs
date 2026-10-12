use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;

use crate::{
    auth::{create_jwt_token, AdminUser, AppState},
    error::AppError,
    models::{AuthResponse, LoginDto},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login_handler))
        .route("/auth/me", get(me_handler))
        .route("/health", get(health_handler))
}

async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<LoginDto>,
) -> Result<impl IntoResponse, AppError> {
    if payload.username == state.config.admin_username
        && payload.password == state.config.admin_password
    {
        let token = create_jwt_token(&payload.username, &state.config.jwt_secret)
            .map_err(|e| AppError::Internal(format!("Failed to sign token: {e}")))?;

        Ok((
            StatusCode::OK,
            Json(AuthResponse {
                token,
                token_type: "Bearer".to_string(),
                expires_in: 86400, // 24 hours
            }),
        ))
    } else {
        Err(AppError::Unauthorized(
            "Invalid admin credentials".to_string(),
        ))
    }
}

async fn me_handler(admin: AdminUser) -> impl IntoResponse {
    Json(json!({
        "username": admin.username,
        "authenticated": true
    }))
}

async fn health_handler(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    sqlx::query("SELECT 1")
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::Database(e))?;

    Ok(Json(json!({
        "status": "healthy",
        "database": "connected"
    })))
}
