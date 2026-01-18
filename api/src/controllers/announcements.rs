use axum::{
    extract::State,
    Json,
};
use serde::Serialize;

#[derive(Serialize, sqlx::FromRow)]
pub struct Announcement {
    pub id: i64,
    pub title: String,
    pub message: String,
    pub priority: i64,
    pub created_at: String,
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
