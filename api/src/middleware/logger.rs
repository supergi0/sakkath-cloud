use axum::{
    http::{Request, Method},
    middleware::Next,
    response::Response,
    body::Body,
};

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
    crate::helpers::auth::extract_email(headers).unwrap_or_else(|_| "anonymous".to_string())
}
