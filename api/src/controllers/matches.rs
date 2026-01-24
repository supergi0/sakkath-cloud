use axum::{
    extract::{State, Path},
    Json,
};
use serde::{Serialize, Deserialize};
use sqlx::Row;

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

#[derive(Serialize, sqlx::FromRow)]
pub struct TeamMatch {
    pub id: i64,
    pub t1_id: i64,
    pub t2_id: i64,
    pub t1_name: String,
    pub t2_name: String,
    pub t1_score: i64,
    pub t2_score: i64,
    pub t1_spirit: Option<i64>,
    pub t2_spirit: Option<i64>,
    pub field_name: String,
    pub time: String,
    pub possession: Option<i64>,
    pub stream_url: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MatchEvent {
    pub id: i64,
    pub player_id: i64,
    pub player_name: String,
    pub team_id: i64,
    pub event_type: i64,
    pub created_at: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MatchPlayer {
    pub id: i64,
    pub name: String,
    pub team_id: i64,
}

#[derive(Serialize)]
pub struct MatchDetail {
    pub id: i64,
    pub t1_id: i64,
    pub t2_id: i64,
    pub t1_name: String,
    pub t2_name: String,
    pub t1_score: i64,
    pub t2_score: i64,
    pub t1_spirit: Option<i64>,
    pub t2_spirit: Option<i64>,
    pub t1_division: i64,
    pub t2_division: i64,
    pub t1_small_logo: Option<String>,
    pub t2_small_logo: Option<String>,
    pub possession: Option<i64>,
    pub field_name: String,
    pub time: String,
    pub stream_url: Option<String>,
    pub players: Vec<MatchPlayer>,
    pub events: Vec<MatchEvent>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct UpcomingMatch {
    pub id: i64,
    pub t1_id: i64,
    pub t2_id: i64,
    pub t1_name: String,
    pub t2_name: String,
    pub field_name: String,
    pub time: String,
    pub possession: Option<i64>,
    pub volunteer_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct MatchEventRequest {
    pub player_id: Option<i64>,
    pub event_type: i64,
}

#[derive(Deserialize)]
pub struct SpiritScoreRequest {
    pub spirit_score: i64,
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

// Get matches for a team
pub async fn get_team_matches(State(state): State<crate::AppState>, Path(team_id): Path<i64>) -> Json<Vec<TeamMatch>> {
    let matches = sqlx::query_as::<_, TeamMatch>(
        r#"
        SELECT m.id, m.t1_id, m.t2_id, t1.name as t1_name, t2.name as t2_name,
               m.t1_score, m.t2_score, m.t1_spirit, m.t2_spirit, 
               COALESCE(f.name, '') as field_name, COALESCE(m.time, '') as time,
               m.possession, m.stream_url
        FROM matches m
        JOIN teams t1 ON t1.id = m.t1_id
        JOIN teams t2 ON t2.id = m.t2_id
        LEFT JOIN fields f ON f.id = m.field_id
        WHERE m.deleted_at IS NULL AND (m.t1_id = ? OR m.t2_id = ?)
        ORDER BY m.time DESC
        "#
    ).bind(team_id).bind(team_id).fetch_all(&state.db).await.unwrap_or_default();
    Json(matches)
}

// Get match events (live log)
pub async fn get_match_events(State(state): State<crate::AppState>, Path(match_id): Path<i64>) -> Json<Vec<MatchEvent>> {
    let events = sqlx::query_as::<_, MatchEvent>(
        r#"
        SELECT me.id, me.player_id, 
               COALESCE(u.name, '') as player_name, 
               COALESCE(me.team_id, u.team_id) as team_id, 
               me.event_type, 
               COALESCE(me.created_at, '') as created_at
        FROM match_events me
        LEFT JOIN users u ON u.id = me.player_id
        WHERE me.match_id = ?
        ORDER BY me.created_at ASC
        "#
    ).bind(match_id).fetch_all(&state.db).await.unwrap_or_default();
    Json(events)
}

// Get match detail with players and events for live reporting
pub async fn get_match_detail(State(state): State<crate::AppState>, Path(match_id): Path<i64>) -> Json<MatchDetail> {
    let match_row = sqlx::query(
        r#"
        SELECT m.id, m.t1_id, m.t2_id, t1.name, t2.name, m.t1_score, m.t2_score, 
               m.t1_spirit, m.t2_spirit, t1.division, t2.division, t1.small_logo, t2.small_logo, m.possession,
               COALESCE(f.name, '') as field_name, COALESCE(m.time, '') as time, m.stream_url
        FROM matches m
        JOIN teams t1 ON t1.id = m.t1_id
        JOIN teams t2 ON t2.id = m.t2_id
        LEFT JOIN fields f ON f.id = m.field_id
        WHERE m.id = ? AND m.deleted_at IS NULL
        "#
    ).bind(match_id).fetch_optional(&state.db).await.unwrap_or(None);

    let (id, t1_id, t2_id, t1_name, t2_name, t1_score, t2_score, t1_spirit, t2_spirit, t1_division, t2_division, t1_small_logo, t2_small_logo, possession, field_name, time, stream_url) = match match_row {
        Some(row) => (
            row.get::<i64, _>(0),
            row.get::<i64, _>(1),
            row.get::<i64, _>(2),
            row.get::<String, _>(3),
            row.get::<String, _>(4),
            row.get::<i64, _>(5),
            row.get::<i64, _>(6),
            row.get::<Option<i64>, _>(7),
            row.get::<Option<i64>, _>(8),
            row.get::<i64, _>(9),
            row.get::<i64, _>(10),
            row.get::<Option<String>, _>(11),
            row.get::<Option<String>, _>(12),
            row.get::<Option<i64>, _>(13),
            row.get::<String, _>(14),
            row.get::<String, _>(15),
            row.get::<Option<String>, _>(16),
        ),
        None => (0, 0, 0, "".to_string(), "".to_string(), 0, 0, None, None, 1, 1, None, None, None, "".to_string(), "".to_string(), None)
    };

    let players = sqlx::query_as::<_, MatchPlayer>(
        "SELECT id, name, team_id FROM users WHERE team_id IN (?, ?) AND deleted_at IS NULL"
    ).bind(t1_id).bind(t2_id).fetch_all(&state.db).await.unwrap_or_default();

    let events = sqlx::query_as::<_, MatchEvent>(
        r#"
        SELECT me.id, me.player_id, 
               COALESCE(u.name, '') as player_name, 
               COALESCE(me.team_id, u.team_id) as team_id, 
               me.event_type,
               COALESCE(me.created_at, '') as created_at
        FROM match_events me 
        LEFT JOIN users u ON u.id = me.player_id
        WHERE me.match_id = ? ORDER BY me.created_at ASC
        "#
    ).bind(match_id).fetch_all(&state.db).await.unwrap_or_default();

    Json(MatchDetail { 
        id, t1_id, t2_id, t1_name, t2_name, t1_score, t2_score, 
        t1_spirit, t2_spirit, t1_division, t2_division, t1_small_logo, t2_small_logo, possession, 
        field_name, time, stream_url, players, events 
    })
}

// Get upcoming matches for volunteers (next 12 matches)
pub async fn get_upcoming_matches(State(state): State<crate::AppState>) -> Json<Vec<UpcomingMatch>> {
    let matches = sqlx::query_as::<_, UpcomingMatch>(
        r#"
        SELECT m.id, m.t1_id, m.t2_id, t1.name as t1_name, t2.name as t2_name,
               COALESCE(f.name, '') as field_name, COALESCE(m.time, '') as time,
               m.possession, m.volunteer_id
        FROM matches m
        JOIN teams t1 ON t1.id = m.t1_id
        JOIN teams t2 ON t2.id = m.t2_id
        LEFT JOIN fields f ON f.id = m.field_id
        WHERE m.deleted_at IS NULL AND (m.possession IS NULL OR m.possession <= 2)
        ORDER BY m.time ASC LIMIT 12
        "#
    ).fetch_all(&state.db).await.unwrap_or_default();
    Json(matches)
}

// Volunteer joins a match
pub async fn join_match(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Path(match_id): Path<i64>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let email = extract_email(&headers)?;
    
    let user: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM users WHERE email = ? AND role IN (0, 1) AND deleted_at IS NULL"
    ).bind(&email).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let user_id = user.ok_or(axum::http::StatusCode::FORBIDDEN)?.0;
    
    // Check if match already has volunteer
    let existing: Option<(Option<i64>,)> = sqlx::query_as(
        "SELECT volunteer_id FROM matches WHERE id = ? AND deleted_at IS NULL"
    ).bind(match_id).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    if let Some((Some(_),)) = existing {
        return Err(axum::http::StatusCode::CONFLICT);
    }
    
    sqlx::query("UPDATE matches SET volunteer_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(user_id).bind(match_id).execute(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(serde_json::json!({"success": true})))
}

// Start match (set possession based on request)
pub async fn start_match(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Path(match_id): Path<i64>,
    Json(payload): Json<StartMatchRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    verify_volunteer(&state, &headers, match_id).await?;
    
    let possession = payload.possession.unwrap_or(1);
    sqlx::query("UPDATE matches SET possession = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(possession).bind(match_id).execute(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(serde_json::json!({"success": true, "possession": possession})))
}

#[derive(Deserialize)]
pub struct StartMatchRequest {
    pub possession: Option<i64>,
}

// End match (set possession to 3)
pub async fn end_match(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Path(match_id): Path<i64>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    verify_volunteer(&state, &headers, match_id).await?;
    
    sqlx::query("UPDATE matches SET possession = 3, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(match_id).execute(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(serde_json::json!({"success": true})))
}

// Record match event (score/turnover/block)
pub async fn record_event(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Path(match_id): Path<i64>,
    Json(payload): Json<MatchEventRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    verify_volunteer(&state, &headers, match_id).await?;
    
    // Validate event_type (0=goal, 1=assist, 2=block, 3=turnover)
    if payload.event_type < 0 || payload.event_type > 3 {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    
    let match_info: Option<(i64, i64, i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT t1_id, t2_id, t1_score, t2_score, possession FROM matches WHERE id = ?"
    ).bind(match_id).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let (t1_id, t2_id, mut t1_score, mut t2_score, possession) = match_info
        .ok_or(axum::http::StatusCode::NOT_FOUND)?;
    
    let current_pos = possession.unwrap_or(1);
    let mut new_pos = current_pos;
    let team_id = if current_pos == 1 { t1_id } else { t2_id };
    
    // Record event
    if let Some(player_id) = payload.player_id {
        // Player specified - get their team_id
        let player_team: Option<(i64,)> = sqlx::query_as(
            "SELECT team_id FROM users WHERE id = ?"
        ).bind(player_id).fetch_optional(&state.db).await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
        
        let player_team_id = player_team.map(|t| t.0);
        
        sqlx::query("INSERT INTO match_events (match_id, player_id, team_id, event_type) VALUES (?, ?, ?, ?)")
            .bind(match_id).bind(player_id).bind(player_team_id).bind(payload.event_type)
            .execute(&state.db).await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    } else if payload.event_type == 0 {
        // Score without player - store team_id
        sqlx::query("INSERT INTO match_events (match_id, player_id, team_id, event_type) VALUES (?, NULL, ?, ?)")
            .bind(match_id).bind(team_id).bind(payload.event_type)
            .execute(&state.db).await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    
    match payload.event_type {
        0 => { // Goal - increment score for possessing team, switch possession
            if current_pos == 1 { t1_score += 1; } else { t2_score += 1; }
            new_pos = if current_pos == 1 { 2 } else { 1 };
        }
        2 => { // Block - switch possession
            new_pos = if current_pos == 1 { 2 } else { 1 };
        }
        3 => { // Turnover - switch possession
            new_pos = if current_pos == 1 { 2 } else { 1 };
        }
        _ => {}
    }
    
    sqlx::query("UPDATE matches SET t1_score = ?, t2_score = ?, possession = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(t1_score).bind(t2_score).bind(new_pos).bind(match_id)
        .execute(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(serde_json::json!({"success": true, "t1_score": t1_score, "t2_score": t2_score, "possession": new_pos})))
}

// Undo latest event
pub async fn undo_event(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Path(match_id): Path<i64>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    verify_volunteer(&state, &headers, match_id).await?;
    
    let match_info: Option<(i64, i64, i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT t1_id, t2_id, t1_score, t2_score, possession FROM matches WHERE id = ?"
    ).bind(match_id).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let (_t1_id, _t2_id, mut t1_score, mut t2_score, possession) = match_info
        .ok_or(axum::http::StatusCode::NOT_FOUND)?;
    
    let mut current_pos = possession.unwrap_or(1);
    
    // Get latest event
    let latest_event: Option<(i64, Option<i64>, i64, String)> = sqlx::query_as(
        "SELECT id, player_id, event_type, created_at FROM match_events WHERE match_id = ? ORDER BY created_at DESC, id DESC LIMIT 1"
    ).bind(match_id).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    if let Some((event_id, _player_id, event_type, created_at)) = latest_event {
        // Delete the event
        sqlx::query("DELETE FROM match_events WHERE id = ?")
            .bind(event_id).execute(&state.db).await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
        
        // For score events, check for paired assist within 1 second
        if event_type == 0 || event_type == 1 {
            let paired_event: Option<(i64, i64)> = sqlx::query_as(
                r#"SELECT id, event_type FROM match_events 
                   WHERE match_id = ? AND ABS(strftime('%s', created_at) - strftime('%s', ?)) <= 1
                   AND (event_type = 0 OR event_type = 1)
                   ORDER BY created_at DESC, id DESC LIMIT 1"#
            ).bind(match_id).bind(&created_at).fetch_optional(&state.db).await
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
            
            if let Some((paired_id, _paired_type)) = paired_event {
                sqlx::query("DELETE FROM match_events WHERE id = ?")
                    .bind(paired_id).execute(&state.db).await
                    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
            }
        }
        
        // Revert score and possession based on event type
        match event_type {
            0 => { // Goal was scored - revert score and switch possession back
                if current_pos == 1 { t2_score = (t2_score - 1).max(0); } 
                else { t1_score = (t1_score - 1).max(0); }
                current_pos = if current_pos == 1 { 2 } else { 1 };
            }
            2 | 3 => { // Block/Turnover - switch possession back
                current_pos = if current_pos == 1 { 2 } else { 1 };
            }
            _ => {}
        }
        
        sqlx::query("UPDATE matches SET t1_score = ?, t2_score = ?, possession = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(t1_score).bind(t2_score).bind(current_pos).bind(match_id)
            .execute(&state.db).await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
        
        Ok(Json(serde_json::json!({"success": true, "t1_score": t1_score, "t2_score": t2_score, "possession": current_pos})))
    } else {
        Ok(Json(serde_json::json!({"success": false, "message": "No events to undo"})))
    }
}

// POC submits spirit score for opponent
pub async fn submit_spirit_score(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Path(match_id): Path<i64>,
    Json(payload): Json<SpiritScoreRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let email = extract_email(&headers)?;
    
    // Validate spirit score range (0-20)
    if payload.spirit_score < 0 || payload.spirit_score > 20 {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    
    let poc_team: Option<(i64,)> = sqlx::query_as(
        "SELECT team_id FROM users WHERE email = ? AND role = 3 AND deleted_at IS NULL"
    ).bind(&email).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let team_id = poc_team.ok_or(axum::http::StatusCode::FORBIDDEN)?.0;
    
    let match_info: Option<(i64, i64, Option<i64>, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT t1_id, t2_id, t1_spirit, t2_spirit, possession FROM matches WHERE id = ? AND deleted_at IS NULL"
    ).bind(match_id).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let (t1_id, t2_id, t1_spirit, t2_spirit, possession) = match_info
        .ok_or(axum::http::StatusCode::NOT_FOUND)?;
    
    // Must be done (possession >= 3)
    if possession.unwrap_or(0) < 3 {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    
    // POC rates the OTHER team's spirit
    if team_id == t1_id {
        if t2_spirit.is_some() { return Err(axum::http::StatusCode::CONFLICT); }
        sqlx::query("UPDATE matches SET t2_spirit = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(payload.spirit_score).bind(match_id).execute(&state.db).await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    } else if team_id == t2_id {
        if t1_spirit.is_some() { return Err(axum::http::StatusCode::CONFLICT); }
        sqlx::query("UPDATE matches SET t1_spirit = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(payload.spirit_score).bind(match_id).execute(&state.db).await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    } else {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }
    
    Ok(Json(serde_json::json!({"success": true})))
}

// Get POC's team matches for spirit score submission
pub async fn get_poc_matches(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<TeamMatch>>, axum::http::StatusCode> {
    let email = extract_email(&headers)?;
    
    let poc_team: Option<(i64,)> = sqlx::query_as(
        "SELECT team_id FROM users WHERE email = ? AND role = 3 AND deleted_at IS NULL"
    ).bind(&email).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let team_id = poc_team.ok_or(axum::http::StatusCode::FORBIDDEN)?.0;
    
    let matches = sqlx::query_as::<_, TeamMatch>(
        r#"
        SELECT m.id, m.t1_id, m.t2_id, t1.name as t1_name, t2.name as t2_name,
               m.t1_score, m.t2_score, m.t1_spirit, m.t2_spirit,
               COALESCE(f.name, '') as field_name, COALESCE(m.time, '') as time,
               m.possession, m.stream_url
        FROM matches m
        JOIN teams t1 ON t1.id = m.t1_id
        JOIN teams t2 ON t2.id = m.t2_id
        LEFT JOIN fields f ON f.id = m.field_id
        WHERE m.deleted_at IS NULL AND (m.t1_id = ? OR m.t2_id = ?) AND m.possession >= 3
        ORDER BY m.time DESC
        "#
    ).bind(team_id).bind(team_id).fetch_all(&state.db).await.unwrap_or_default();
    
    Ok(Json(matches))
}

async fn verify_volunteer(state: &crate::AppState, headers: &axum::http::HeaderMap, match_id: i64) -> Result<i64, axum::http::StatusCode> {
    let email = extract_email(headers)?;
    
    let user: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM users WHERE email = ? AND role IN (0, 1) AND deleted_at IS NULL"
    ).bind(&email).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let user_id = user.ok_or(axum::http::StatusCode::FORBIDDEN)?.0;
    
    let match_vol: Option<(Option<i64>,)> = sqlx::query_as(
        "SELECT volunteer_id FROM matches WHERE id = ? AND deleted_at IS NULL"
    ).bind(match_id).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let vol_id = match_vol.ok_or(axum::http::StatusCode::NOT_FOUND)?.0;
    
    if vol_id != Some(user_id) {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }
    
    Ok(user_id)
}

fn extract_email(headers: &axum::http::HeaderMap) -> Result<String, axum::http::StatusCode> {
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
