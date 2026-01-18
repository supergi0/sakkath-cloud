use axum::{
    extract::State,
    Json,
};
use serde::Serialize;

#[derive(Serialize, sqlx::FromRow)]
pub struct Field {
    pub id: i64,
    pub name: String,
    pub hints: Option<String>,
    pub map_link: Option<String>,
}

#[derive(Serialize)]
pub struct Stats {
    pub teams: i64,
    pub players: i64,
    pub points: i64,
    pub games: i64,
    pub fields: i64,
}

// Get fields
pub async fn get_fields(State(state): State<crate::AppState>) -> Json<Vec<Field>> {
    let fields = sqlx::query_as::<_, Field>(
        "SELECT id, name, hints, map_link FROM fields ORDER BY id"
    ).fetch_all(&state.db).await.unwrap_or_default();
    Json(fields)
}

// Get tournament stats
pub async fn get_stats(State(state): State<crate::AppState>) -> Json<Stats> {
    let stats: (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT 
            (SELECT COUNT(*) FROM teams WHERE deleted_at IS NULL),
            (SELECT COUNT(*) FROM users WHERE deleted_at IS NULL AND team_id IS NOT NULL),
            (SELECT COALESCE(SUM(t1_score + t2_score), 0) FROM matches WHERE deleted_at IS NULL),
            (SELECT COUNT(*) FROM matches WHERE deleted_at IS NULL AND t1_score > 0),
            (SELECT COUNT(*) FROM fields)"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or((0, 0, 0, 0, 0));

    Json(Stats {
        teams: stats.0,
        players: stats.1,
        points: stats.2,
        games: stats.3,
        fields: stats.4,
    })
}
