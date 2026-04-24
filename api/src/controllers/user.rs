use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub user_id: i64,
    pub email: String,
    pub exp: usize,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    pub captcha_token: Option<String>,
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

#[derive(Deserialize)]
struct TurnstileVerifyResponse {
    success: bool,
}

fn extract_client_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("cf-connecting-ip")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string())
        .or_else(|| {
            headers
                .get("x-forwarded-for")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(',').next())
                .map(|value| value.trim().to_string())
        })
}

async fn verify_turnstile(
    headers: &HeaderMap,
    captcha_token: Option<&str>,
) -> Result<(), StatusCode> {
    let secret = match env::var("TURNSTILE_SECRET_KEY") {
        Ok(secret) if !secret.trim().is_empty() => secret,
        _ => return Ok(()),
    };

    let token = captcha_token
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or(StatusCode::FORBIDDEN)?;

    let mut form_fields = vec![("secret", secret), ("response", token.to_string())];

    if let Some(remote_ip) = extract_client_ip(headers) {
        form_fields.push(("remoteip", remote_ip));
    }

    let response = reqwest::Client::new()
        .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .form(&form_fields)
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let verify_response: TurnstileVerifyResponse =
        response.json().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    if verify_response.success {
        Ok(())
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

pub async fn login(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    verify_turnstile(&headers, payload.captcha_token.as_deref()).await?;

    let user: Option<(i64, String, i64, Option<String>)> = sqlx::query_as(
        "SELECT id, email, role, password_hash FROM users WHERE email = ? AND deleted_at IS NULL",
    )
    .bind(&payload.email)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some((user_id, email, role, password_hash)) = user {
        let Some(password_hash) = password_hash else {
            return Err(StatusCode::UNAUTHORIZED);
        };

        if !crate::helpers::auth::verify_password(&payload.password, &password_hash) {
            return Err(StatusCode::UNAUTHORIZED);
        }

        let secret = crate::helpers::auth::jwt_secret()?;
        let expiration = Utc::now()
            .checked_add_signed(Duration::hours(24))
            .expect("valid timestamp")
            .timestamp() as usize;

        let claims = Claims {
            user_id,
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
    match crate::helpers::auth::decode_token_claims(&payload.token) {
        Ok(claims) => {
            let user: Option<(i64,)> =
                sqlx::query_as("SELECT role FROM users WHERE email = ? AND deleted_at IS NULL")
                    .bind(&claims.email)
                    .fetch_optional(&state.db)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            if let Some((role,)) = user {
                Ok(Json(VerifyResponse {
                    valid: true,
                    role: Some(role),
                }))
            } else {
                Ok(Json(VerifyResponse {
                    valid: false,
                    role: None,
                }))
            }
        }
        Err(_) => Ok(Json(VerifyResponse {
            valid: false,
            role: None,
        })),
    }
}

// Get user role - protected by middleware
pub async fn get_role(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> Result<Json<RoleResponse>, StatusCode> {
    let claims = crate::helpers::auth::extract_claims(&headers)?;

    let user: Option<(i64,)> =
        sqlx::query_as("SELECT role FROM users WHERE email = ? AND deleted_at IS NULL")
            .bind(&claims.email)
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
