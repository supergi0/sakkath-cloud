use axum::{
    http::{Request, Method},
    middleware::Next,
    response::Response,
    body::Body,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use std::env;
use crate::controllers::user::Claims;

// Logger middleware - logs POST requests (except login and verify) with user email
pub async fn logger_middleware(
    request: Request<Body>,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    
    // Only log POST requests (except login and verify)
    if method == Method::POST && !path.ends_with("/auth/login") && !path.ends_with("/auth/verify") {
        let email = extract_email_from_headers(request.headers());
        tracing::info!(">> REQUEST | {} | {} | {}", method, path, email);
    }
    
    next.run(request).await
}

fn extract_email_from_headers(headers: &axum::http::HeaderMap) -> String {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok());
    
    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => h.trim_start_matches("Bearer "),
        _ => return "anonymous".to_string(),
    };
    
    let secret = env::var("JWT_SECRET")
        .unwrap_or_else(|_| "default-secret-change-in-production".to_string());
    
    match decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    ) {
        Ok(token_data) => token_data.claims.email,
        Err(_) => "anonymous".to_string(),
    }
}
