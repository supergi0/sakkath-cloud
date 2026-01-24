use axum::{
    extract::{State, Path},
    http::HeaderMap,
    Json,
};
use serde::{Serialize, Deserialize};

#[derive(Serialize, sqlx::FromRow)]
pub struct Announcement {
    pub id: i64,
    pub title: String,
    pub message: String,
    pub priority: i64,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateAnnouncementRequest {
    pub title: String,
    pub message: String,
    pub priority: i64,
    pub expires_at: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateAnnouncementRequest {
    pub title: String,
    pub message: String,
    pub priority: i64,
    pub expires_at: Option<String>,
}

// Get announcements
pub async fn get_announcements(State(state): State<crate::AppState>) -> Json<Vec<Announcement>> {
    let announcements = sqlx::query_as::<_, Announcement>(
        "SELECT id, title, message, priority, created_at, expires_at FROM announcements 
         WHERE expires_at IS NULL OR expires_at > datetime('now') 
         ORDER BY priority ASC, created_at DESC"
    ).fetch_all(&state.db).await.unwrap_or_default();
    Json(announcements)
}

// Create announcement (SUPER only)
pub async fn create_announcement(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAnnouncementRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    verify_super(&state, &headers).await?;
    
    // Validate required fields
    if payload.title.trim().is_empty() || payload.message.trim().is_empty() {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    
    // Validate priority (0=high, 1=normal, 2=low)
    if payload.priority < 0 || payload.priority > 2 {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    
    let result = sqlx::query(
        "INSERT INTO announcements (title, message, priority, expires_at) VALUES (?, ?, ?, ?)"
    )
    .bind(&payload.title)
    .bind(&payload.message)
    .bind(payload.priority)
    .bind(&payload.expires_at)
    .execute(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(serde_json::json!({"success": true, "id": result.last_insert_rowid()})))
}

// Update announcement (SUPER only)
pub async fn update_announcement(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateAnnouncementRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    verify_super(&state, &headers).await?;
    
    // Validate required fields
    if payload.title.trim().is_empty() || payload.message.trim().is_empty() {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    
    // Validate priority (0=high, 1=normal, 2=low)
    if payload.priority < 0 || payload.priority > 2 {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    
    sqlx::query(
        "UPDATE announcements SET title = ?, message = ?, priority = ?, expires_at = ? WHERE id = ?"
    )
    .bind(&payload.title)
    .bind(&payload.message)
    .bind(payload.priority)
    .bind(&payload.expires_at)
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(serde_json::json!({"success": true})))
}

// Delete announcement (SUPER only)
pub async fn delete_announcement(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    verify_super(&state, &headers).await?;
    
    sqlx::query("DELETE FROM announcements WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(serde_json::json!({"success": true})))
}

// Verify user is SUPER (role = 0)
async fn verify_super(state: &crate::AppState, headers: &HeaderMap) -> Result<(), axum::http::StatusCode> {
    let email = extract_email(headers)?;
    
    let user: Option<(i64,)> = sqlx::query_as(
        "SELECT role FROM users WHERE email = ? AND deleted_at IS NULL"
    ).bind(&email).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let role = user.ok_or(axum::http::StatusCode::FORBIDDEN)?.0;
    
    if role != 0 {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }
    
    Ok(())
}

fn extract_email(headers: &HeaderMap) -> Result<String, axum::http::StatusCode> {
    let auth_header = headers.get("Authorization").and_then(|h| h.to_str().ok());
    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => h.trim_start_matches("Bearer "),
        _ => return Err(axum::http::StatusCode::UNAUTHORIZED),
    };
    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "default-secret-change-in-production".to_string());
    let token_data = jsonwebtoken::decode::<crate::controllers::user::Claims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_ref()),
        &jsonwebtoken::Validation::default(),
    ).map_err(|_| axum::http::StatusCode::UNAUTHORIZED)?;
    Ok(token_data.claims.email)
}
