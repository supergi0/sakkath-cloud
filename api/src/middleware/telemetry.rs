use axum::{body::Body, http::Request, middleware::Next, response::Response};

fn extract_client_ip(headers: &axum::http::HeaderMap) -> String {
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
        .unwrap_or_else(|| "unknown".to_string())
}

pub async fn telemetry_middleware(request: Request<Body>, next: Next) -> Response {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let ip_address = extract_client_ip(request.headers());

    let response = next.run(request).await;
    crate::telemetry::record_request(
        method,
        path,
        ip_address,
        i64::from(response.status().as_u16()),
    );
    response
}
