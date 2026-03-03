use axum::{
    extract::{State, Path, Query},
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct DivisionQuery {
    pub division: Option<i32>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Team {
    pub id: i64,
    pub name: String,
    pub division: i64,
    pub location: Option<String>,
    pub init_rank: Option<i64>,
    pub full_logo: Option<String>,
    pub small_logo: Option<String>,
}

#[derive(Serialize)]
pub struct TeamDetail {
    pub id: i64,
    pub name: String,
    pub location: String,
    pub division: i64,
    pub init_rank: i64,
    pub players: i64,
    pub games_played: i64,
    pub wins: i64,
    pub losses: i64,
    pub spirit_avg: f64,
    pub spirit_rank: i64,
    pub current_rank: i64,
    pub full_logo: Option<String>,
    pub small_logo: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TeamStanding {
    pub id: i64,
    pub name: String,
    pub location: String,
    pub init_rank: i64,
    pub wins: i64,
    pub losses: i64,
    pub points_for: i64,
    pub points_against: i64,
    pub spirit_avg: f64,
    pub small_logo: Option<String>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct PlayerStat {
    pub id: i64,
    pub name: String,
    pub team_id: i64,
    pub team_name: String,
    pub division: i64,
    pub goals: i64,
    pub assists: i64,
    pub blocks: i64,
    pub turnovers: i64,
    pub matches: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TeamPlayerStat {
    pub id: i64,
    pub name: String,
    pub goals: i64,
    pub assists: i64,
    pub blocks: i64,
    pub turnovers: i64,
    pub is_captain: bool,
    pub is_spirit_captain: bool,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct PocTeam {
    pub id: i64,
    pub name: String,
    pub location: Option<String>,
    pub full_logo: Option<String>,
    pub small_logo: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct PocPlayer {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub is_captain: bool,
    pub is_spirit_captain: bool,
}

#[derive(Deserialize)]
pub struct UpdatePlayerRequest {
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub is_captain: bool,
    pub is_spirit_captain: bool,
}

#[derive(Deserialize)]
pub struct AddPlayerRequest {
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub is_captain: bool,
    pub is_spirit_captain: bool,
}

// All teams
pub async fn get_teams(State(state): State<crate::AppState>) -> Json<Vec<Team>> {
    let teams = sqlx::query_as::<_, Team>(
        "SELECT id, name, division, location, init_rank, full_logo, small_logo FROM teams WHERE deleted_at IS NULL ORDER BY division, init_rank"
    ).fetch_all(&state.db).await.unwrap_or_default();
    Json(teams)
}

// Single team detail with computed stats in one query
pub async fn get_team_detail(State(state): State<crate::AppState>, Path(id): Path<i64>) -> Json<TeamDetail> {
    let result = sqlx::query_as::<_, (i64, String, Option<String>, i64, Option<i64>, i64, i64, i64, f64, i64, Option<String>, Option<String>)>(
        r#"
        SELECT 
            t.id, t.name, t.location, t.division, t.init_rank,
            (SELECT COUNT(*) FROM users WHERE team_id = t.id AND deleted_at IS NULL) as players,
            (SELECT COUNT(*) FROM matches WHERE deleted_at IS NULL AND possession >= 3 AND ((t1_id = t.id AND t1_score > t2_score) OR (t2_id = t.id AND t2_score > t1_score))) as wins,
            (SELECT COUNT(*) FROM matches WHERE deleted_at IS NULL AND possession >= 3 AND ((t1_id = t.id AND t1_score < t2_score) OR (t2_id = t.id AND t2_score < t1_score))) as losses,
            COALESCE((SELECT AVG(CASE WHEN t1_id = t.id THEN t1_spirit WHEN t2_id = t.id THEN t2_spirit END) FROM matches WHERE deleted_at IS NULL AND possession >= 3 AND (t1_id = t.id OR t2_id = t.id) AND (t1_spirit IS NOT NULL OR t2_spirit IS NOT NULL)), 0.0) as spirit_avg,
            (SELECT COUNT(*) + 1 FROM (
                SELECT tm.id, COALESCE(AVG(CASE WHEN m.t1_id = tm.id THEN m.t1_spirit WHEN m.t2_id = tm.id THEN m.t2_spirit END), 0.0) as avg_spirit
                FROM teams tm
                LEFT JOIN matches m ON (m.t1_id = tm.id OR m.t2_id = tm.id) AND m.deleted_at IS NULL AND m.possession >= 3
                WHERE tm.deleted_at IS NULL AND tm.division = t.division
                GROUP BY tm.id
            ) sub WHERE sub.avg_spirit > COALESCE((SELECT AVG(CASE WHEN t1_id = t.id THEN t1_spirit WHEN t2_id = t.id THEN t2_spirit END) FROM matches WHERE deleted_at IS NULL AND possession >= 3 AND (t1_id = t.id OR t2_id = t.id) AND (t1_spirit IS NOT NULL OR t2_spirit IS NOT NULL)), 0.0)) as spirit_rank,
            t.full_logo, t.small_logo
        FROM teams t WHERE t.id = ? AND t.deleted_at IS NULL
        "#
    ).bind(id).fetch_optional(&state.db).await;

    match result {
        Ok(Some(r)) => Json(TeamDetail {
            id: r.0,
            name: r.1,
            location: r.2.unwrap_or_default(),
            division: r.3,
            init_rank: r.4.unwrap_or(0),
            players: r.5,
            games_played: r.6 + r.7,
            wins: r.6,
            losses: r.7,
            spirit_avg: r.8,
            spirit_rank: r.9,
            current_rank: r.4.unwrap_or(0),
            full_logo: r.10,
            small_logo: r.11,
        }),
        _ => Json(TeamDetail { id: 0, name: "Not Found".to_string(), location: "".to_string(), division: 0, init_rank: 0, players: 0, games_played: 0, wins: 0, losses: 0, spirit_avg: 0.0, spirit_rank: 0, current_rank: 0, full_logo: None, small_logo: None }),
    }
}

// Team players with stats in one query
pub async fn get_team_players(State(state): State<crate::AppState>, Path(id): Path<i64>) -> Json<Vec<TeamPlayerStat>> {
    let players = sqlx::query_as::<_, TeamPlayerStat>(
        r#"
        SELECT 
            u.id, u.name,
            COALESCE(SUM(CASE WHEN me.event_type = 0 THEN 1 ELSE 0 END), 0) as goals,
            COALESCE(SUM(CASE WHEN me.event_type = 1 THEN 1 ELSE 0 END), 0) as assists,
            COALESCE(SUM(CASE WHEN me.event_type = 2 THEN 1 ELSE 0 END), 0) as blocks,
            COALESCE(SUM(CASE WHEN me.event_type = 3 THEN 1 ELSE 0 END), 0) as turnovers,
            COALESCE(u.is_captain, 0) as is_captain,
            COALESCE(u.is_spirit_captain, 0) as is_spirit_captain
        FROM users u
        LEFT JOIN match_events me ON me.player_id = u.id
        WHERE u.team_id = ? AND u.deleted_at IS NULL
        GROUP BY u.id, u.name, u.is_captain, u.is_spirit_captain
        "#
    ).bind(id).fetch_all(&state.db).await.unwrap_or_default();
    Json(players)
}

// Standings by division using helper sorting (c1-c7 tiebreakers, includes live scores)
pub async fn get_standings(State(state): State<crate::AppState>, Query(params): Query<DivisionQuery>) -> Json<Vec<TeamStanding>> {
    let division = params.division.unwrap_or(0) as i64;
    
    let sorted = crate::helpers::sorting::get_cached_intermediate_standings(&state.db, division).await;
    
    let standings: Vec<TeamStanding> = sorted.iter().map(|t| TeamStanding {
        id: t.team_id,
        name: t.name.clone(),
        location: String::new(),
        init_rank: t.init_rank,
        wins: t.wins,
        losses: t.losses,
        points_for: t.points_for,
        points_against: t.points_against,
        spirit_avg: t.spirit_avg,
        small_logo: t.small_logo.clone(),
    }).collect();
    
    Json(standings)
}

// All player stats in one query
pub async fn get_player_stats(State(state): State<crate::AppState>) -> Json<Vec<PlayerStat>> {
    if let Some(cached) = crate::helpers::cache::get_player_stats::<Vec<PlayerStat>>().await {
        return Json(cached);
    }

    let players = sqlx::query_as::<_, PlayerStat>(
        r#"
        SELECT 
            u.id, u.name, 
            COALESCE(u.team_id, 0) as team_id, 
            COALESCE(t.name, '') as team_name, 
            COALESCE(t.division, 0) as division,
            COALESCE(SUM(CASE WHEN me.event_type = 0 THEN 1 ELSE 0 END), 0) as goals,
            COALESCE(SUM(CASE WHEN me.event_type = 1 THEN 1 ELSE 0 END), 0) as assists,
            COALESCE(SUM(CASE WHEN me.event_type = 2 THEN 1 ELSE 0 END), 0) as blocks,
            COALESCE(SUM(CASE WHEN me.event_type = 3 THEN 1 ELSE 0 END), 0) as turnovers,
            COUNT(DISTINCT me.match_id) as matches
        FROM users u
        LEFT JOIN teams t ON u.team_id = t.id
        LEFT JOIN match_events me ON me.player_id = u.id
        WHERE u.deleted_at IS NULL AND u.team_id IS NOT NULL
        GROUP BY u.id, u.name, u.team_id, t.name, t.division
        "#
    ).fetch_all(&state.db).await.unwrap_or_default();

    crate::helpers::cache::set_player_stats(&players).await;
    Json(players)
}

// Get POC's team
pub async fn get_poc_team(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<PocTeam>, axum::http::StatusCode> {
    let email = extract_email(&headers)?;
    
    let team = sqlx::query_as::<_, PocTeam>(
        "SELECT t.id, t.name, t.location, t.full_logo, t.small_logo FROM teams t 
         INNER JOIN users u ON u.team_id = t.id 
         WHERE u.email = ? AND u.role = 3 AND u.deleted_at IS NULL"
    ).bind(&email).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    match team {
        Some(t) => Ok(Json(t)),
        None => Err(axum::http::StatusCode::FORBIDDEN),
    }
}

// Get POC's team players
pub async fn get_poc_players(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<PocPlayer>>, axum::http::StatusCode> {
    let email = extract_email(&headers)?;
    
    let team_id: Option<(i64,)> = sqlx::query_as(
        "SELECT team_id FROM users WHERE email = ? AND role = 3 AND deleted_at IS NULL"
    ).bind(&email).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let team_id = team_id.ok_or(axum::http::StatusCode::FORBIDDEN)?.0;
    
    let players = sqlx::query_as::<_, PocPlayer>(
        "SELECT id, name, email, phone, COALESCE(is_captain, 0) as is_captain, COALESCE(is_spirit_captain, 0) as is_spirit_captain 
         FROM users WHERE team_id = ? AND deleted_at IS NULL ORDER BY name"
    ).bind(team_id).fetch_all(&state.db).await.unwrap_or_default();
    
    Ok(Json(players))
}

// Update a player (POC can only update their own team's players)
pub async fn update_poc_player(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Path(player_id): Path<i64>,
    Json(payload): Json<UpdatePlayerRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let email = extract_email(&headers)?;
    
    // Validate required fields
    if payload.name.trim().is_empty() || payload.email.trim().is_empty() {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    
    // Verify POC owns this team and player belongs to it
    let valid: Option<(i64,)> = sqlx::query_as(
        "SELECT p.id FROM users p 
         INNER JOIN users poc ON poc.team_id = p.team_id 
         WHERE p.id = ? AND poc.email = ? AND poc.role = 3 AND p.deleted_at IS NULL"
    ).bind(player_id).bind(&email).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    if valid.is_none() {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }
    
    // Check if email already used by another user
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM users WHERE email = ? AND id != ? AND deleted_at IS NULL"
    ).bind(&payload.email).bind(player_id).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    if existing.is_some() {
        return Err(axum::http::StatusCode::CONFLICT);
    }
    
    sqlx::query(
        "UPDATE users SET name = ?, email = ?, phone = ?, is_captain = ?, is_spirit_captain = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
    ).bind(&payload.name).bind(&payload.email).bind(&payload.phone)
     .bind(payload.is_captain).bind(payload.is_spirit_captain).bind(player_id)
     .execute(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(serde_json::json!({"success": true})))
}

// Add a new player to POC's team
pub async fn add_poc_player(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<AddPlayerRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let email = extract_email(&headers)?;
    
    // Validate required fields
    if payload.name.trim().is_empty() || payload.email.trim().is_empty() {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    
    let team_id: Option<(i64,)> = sqlx::query_as(
        "SELECT team_id FROM users WHERE email = ? AND role = 3 AND deleted_at IS NULL"
    ).bind(&email).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let team_id = team_id.ok_or(axum::http::StatusCode::FORBIDDEN)?.0;
    
    // Check if email already exists
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM users WHERE email = ? AND deleted_at IS NULL"
    ).bind(&payload.email).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    if existing.is_some() {
        return Err(axum::http::StatusCode::CONFLICT);
    }
    
    let result = sqlx::query(
        "INSERT INTO users (name, email, phone, team_id, role, is_captain, is_spirit_captain) VALUES (?, ?, ?, ?, 2, ?, ?)"
    ).bind(&payload.name).bind(&payload.email).bind(&payload.phone)
     .bind(team_id).bind(payload.is_captain).bind(payload.is_spirit_captain)
     .execute(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(serde_json::json!({"success": true, "id": result.last_insert_rowid()})))
}

// Delete a player from POC's team
pub async fn delete_poc_player(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Path(player_id): Path<i64>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let email = extract_email(&headers)?;
    
    // Verify POC owns this team and player belongs to it
    let valid: Option<(i64,)> = sqlx::query_as(
        "SELECT p.id FROM users p 
         INNER JOIN users poc ON poc.team_id = p.team_id 
         WHERE p.id = ? AND poc.email = ? AND poc.role = 3 AND p.deleted_at IS NULL"
    ).bind(player_id).bind(&email).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    if valid.is_none() {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }
    
    sqlx::query("UPDATE users SET deleted_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(player_id).execute(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(serde_json::json!({"success": true})))
}

// Update team logo (POC can only update their own team's logo)
#[derive(Deserialize)]
pub struct UpdateLogoRequest {
    pub full_logo: String,
    pub small_logo: String,
}

pub async fn update_poc_team_logo(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<UpdateLogoRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let email = extract_email(&headers)?;
    
    let team_id: Option<(i64,)> = sqlx::query_as(
        "SELECT team_id FROM users WHERE email = ? AND role = 3 AND deleted_at IS NULL"
    ).bind(&email).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let team_id = team_id.ok_or(axum::http::StatusCode::FORBIDDEN)?.0;
    
    sqlx::query(
        "UPDATE teams SET full_logo = ?, small_logo = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
    ).bind(&payload.full_logo).bind(&payload.small_logo).bind(team_id)
     .execute(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(serde_json::json!({"success": true})))
}

fn extract_email(headers: &axum::http::HeaderMap) -> Result<String, axum::http::StatusCode> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok());

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
