use axum::http::{HeaderMap, StatusCode};
use bcrypt::{BcryptError, DEFAULT_COST};
use jsonwebtoken::{DecodingKey, Validation, decode};

pub fn ensure_jwt_secret_configured() -> Result<(), String> {
    match std::env::var("JWT_SECRET") {
        Ok(secret) if !secret.trim().is_empty() => Ok(()),
        _ => Err("JWT_SECRET is required and must not be empty".to_string()),
    }
}

pub fn jwt_secret() -> Result<String, StatusCode> {
    match std::env::var("JWT_SECRET") {
        Ok(secret) if !secret.trim().is_empty() => Ok(secret),
        _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub fn extract_bearer_token(headers: &HeaderMap) -> Result<&str, StatusCode> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|header| header.to_str().ok());

    match auth_header {
        Some(value) if value.starts_with("Bearer ") => Ok(value.trim_start_matches("Bearer ")),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

pub fn decode_token_claims(token: &str) -> Result<crate::controllers::user::Claims, StatusCode> {
    let secret = jwt_secret()?;
    let token_data = decode::<crate::controllers::user::Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?;

    Ok(token_data.claims)
}

pub fn extract_claims(headers: &HeaderMap) -> Result<crate::controllers::user::Claims, StatusCode> {
    decode_token_claims(extract_bearer_token(headers)?)
}

pub fn extract_email(headers: &HeaderMap) -> Result<String, StatusCode> {
    Ok(extract_claims(headers)?.email)
}

pub fn hash_password(password: &str) -> Result<String, BcryptError> {
    bcrypt::hash(password, DEFAULT_COST)
}

pub fn verify_password(password: &str, password_hash: &str) -> bool {
    bcrypt::verify(password, password_hash).unwrap_or(false)
}
