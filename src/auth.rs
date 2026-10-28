use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use base64::prelude::*;
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::{config::AppConfig, error::AppError};

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::SqlitePool,
    pub config: AppConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdminClaims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

pub fn create_jwt_token(username: &str, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now().timestamp() as usize;
    let exp = now + (24 * 60 * 60); // 24 hours

    let claims = AdminClaims {
        sub: username.to_string(),
        iat: now,
        exp,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

pub fn verify_jwt_token(token: &str, secret: &str) -> Result<AdminClaims, jsonwebtoken::errors::Error> {
    let token_data = decode::<AdminClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn validate_auth_header(auth_header: &str, config: &AppConfig) -> Result<String, AppError> {
    if let Some(token) = auth_header.strip_prefix("Bearer ") {
        let claims = verify_jwt_token(token.trim(), &config.jwt_secret)
            .map_err(|_| AppError::Unauthorized("Invalid or expired Bearer token".to_string()))?;
        if claims.sub == config.admin_username {
            return Ok(claims.sub);
        } else {
            return Err(AppError::Unauthorized("Invalid user identity in token".to_string()));
        }
    }

    if let Some(basic_b64) = auth_header.strip_prefix("Basic ") {
        let decoded = BASE64_STANDARD
            .decode(basic_b64.trim())
            .map_err(|_| AppError::Unauthorized("Malformed Basic auth base64 encoding".to_string()))?;
        let cred_str = String::from_utf8(decoded)
            .map_err(|_| AppError::Unauthorized("Basic auth contains non-UTF8 characters".to_string()))?;

        if let Some((user, pass)) = cred_str.split_once(':') {
            let user_matches = constant_time_eq(user.as_bytes(), config.admin_username.as_bytes());
            let pass_matches = constant_time_eq(pass.as_bytes(), config.admin_password.as_bytes());

            if user_matches && pass_matches {
                return Ok(user.to_string());
            }
        }
        return Err(AppError::Unauthorized("Invalid Basic authentication credentials".to_string()));
    }

    Err(AppError::Unauthorized(
        "Missing or unsupported Authorization header (use Bearer <token> or Basic <base64>)".to_string(),
    ))
}

#[derive(Debug, Clone)]
pub struct AdminUser {
    pub username: String,
}

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|val| val.to_str().ok())
            .ok_or_else(|| {
                AppError::Unauthorized("Missing Authorization header".to_string())
            })?;

        let username = validate_auth_header(auth_header, &state.config)?;
        Ok(AdminUser { username })
    }
}

#[derive(Debug, Clone)]
pub struct OptionalAdminUser(pub Option<AdminUser>);

impl FromRequestParts<AppState> for OptionalAdminUser {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|val| val.to_str().ok());

        if let Some(header) = auth_header {
            if let Ok(username) = validate_auth_header(header, &state.config) {
                return Ok(OptionalAdminUser(Some(AdminUser { username })));
            }
        }

        Ok(OptionalAdminUser(None))
    }
}
