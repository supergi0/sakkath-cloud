use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};

// Auth middleware - validates JWT token from Authorization header
pub async fn auth_middleware(request: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let token = crate::helpers::auth::extract_bearer_token(request.headers())?;
    crate::helpers::auth::decode_token_claims(token)?;
    Ok(next.run(request).await)
}
