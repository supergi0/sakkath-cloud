use axum::{
    extract::State,
    http::{StatusCode, HeaderMap},
    Json,
};
use serde::{Deserialize, Serialize};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use chrono::{Utc, Duration};
use std::env;

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub email: String,
    pub exp: usize,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub role: i64,
}

#[derive(Deserialize)]
pub struct VerifyRequest {
    pub token: String,
}

#[derive(Serialize)]
pub struct VerifyResponse {
    pub valid: bool,
    pub role: Option<i64>,
}

#[derive(Serialize)]
pub struct RoleResponse {
    pub role: i64,
    pub role_name: String,
}

pub async fn login(
    State(state): State<crate::AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    let password_hash = format!("{:x}", md5::compute(&payload.password));
    
    let user: Option<(String, i64)> = sqlx::query_as(
        "SELECT email, role FROM users WHERE email = ? AND password_hash = ? AND deleted_at IS NULL"
    )
    .bind(&payload.email)
    .bind(&password_hash)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    if let Some((email, role)) = user {
        let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "default-secret-change-in-production".to_string());
        let expiration = Utc::now()
            .checked_add_signed(Duration::hours(24))
            .expect("valid timestamp")
            .timestamp() as usize;
        
        let claims = Claims {
            email: email.clone(),
            exp: expiration,
        };
        
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_ref()),
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        Ok(Json(LoginResponse { token, role }))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

pub async fn verify(
    State(state): State<crate::AppState>,
    Json(payload): Json<VerifyRequest>,
) -> Result<Json<VerifyResponse>, StatusCode> {
    let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "default-secret-change-in-production".to_string());
    
    let token_data = decode::<Claims>(
        &payload.token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    );
    
    match token_data {
        Ok(data) => {
            let user: Option<(i64,)> = sqlx::query_as(
                "SELECT role FROM users WHERE email = ? AND deleted_at IS NULL"
            )
            .bind(&data.claims.email)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            if let Some((role,)) = user {
                Ok(Json(VerifyResponse { valid: true, role: Some(role) }))
            } else {
                Ok(Json(VerifyResponse { valid: false, role: None }))
            }
        }
        Err(_) => Ok(Json(VerifyResponse { valid: false, role: None })),
    }
}

// Get user role - protected by middleware
pub async fn get_role(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> Result<Json<RoleResponse>, StatusCode> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => h.trim_start_matches("Bearer "),
        _ => return Err(StatusCode::UNAUTHORIZED),
    };

    let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "default-secret-change-in-production".to_string());

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    ).map_err(|_| StatusCode::UNAUTHORIZED)?;

    let user: Option<(i64,)> = sqlx::query_as(
        "SELECT role FROM users WHERE email = ? AND deleted_at IS NULL"
    )
    .bind(&token_data.claims.email)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some((role,)) = user {
        let role_name = match role {
            0 => "SUPER".to_string(),
            1 => "ADMIN".to_string(),
            3 => "POC".to_string(),
            _ => "USER".to_string(),
        };
        Ok(Json(RoleResponse { role, role_name }))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}