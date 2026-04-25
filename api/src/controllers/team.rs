use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::helpers::cache;

const MAX_TEAM_PLAYERS: i64 = 22;
const TEAM_EDITS_ROUND_KEY: i64 = 10_001;

type PocPlayerCurrentRow = (
    i64,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    i64,
    i64,
    i64,
);

#[derive(Deserialize)]
pub struct DivisionQuery {
    pub division: Option<i32>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct Team {
    pub id: i64,
    pub name: String,
    pub abbreviation: Option<String>,
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
    pub abbreviation: Option<String>,
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
    pub abbreviation: Option<String>,
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
    pub common_name: String,
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
    pub full_name: String,
    pub common_name: Option<String>,
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
    pub abbreviation: Option<String>,
    pub location: Option<String>,
    pub full_logo: Option<String>,
    pub small_logo: Option<String>,
    pub roster_moves_remaining: i64,
}

#[derive(Deserialize)]
pub struct UpdatePocTeamRequest {
    pub abbreviation: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct PocPlayer {
    pub id: i64,
    pub name: String,
    pub common_name: Option<String>,
    pub email: String,
    pub phone: Option<String>,
    pub is_captain: bool,
    pub is_spirit_captain: bool,
}

#[derive(Deserialize)]
pub struct UpdatePlayerRequest {
    pub name: String,
    pub common_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub is_captain: bool,
    pub is_spirit_captain: bool,
}

#[derive(Deserialize)]
pub struct AddPlayerRequest {
    pub name: String,
    pub common_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub is_captain: bool,
    pub is_spirit_captain: bool,
}

// All teams
pub async fn get_teams(State(state): State<crate::AppState>) -> Json<Vec<Team>> {
    if let Some(cached) = crate::helpers::cache::get_teams_list::<Vec<Team>>().await {
        return Json(cached);
    }
    let teams = sqlx::query_as::<_, Team>(
        "SELECT id, name, abbreviation, division, location, init_rank, full_logo, small_logo FROM teams WHERE deleted_at IS NULL ORDER BY division, init_rank"
    ).fetch_all(&state.db).await.unwrap_or_default();
    crate::helpers::cache::set_teams_list(&teams).await;
    Json(teams)
}

// Single team detail with computed stats in one query
pub async fn get_team_detail(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Json<TeamDetail> {
    let result = sqlx::query_as::<_, (i64, String, Option<String>, Option<String>, i64, Option<i64>, i64, i64, i64, f64, i64, Option<String>, Option<String>)>(
        r#"
        SELECT 
            t.id, t.name, t.abbreviation, t.location, t.division, t.init_rank,
            (SELECT COUNT(*) FROM users WHERE team_id = t.id AND role = 2 AND deleted_at IS NULL) as players,
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
            abbreviation: r.2,
            location: r.3.unwrap_or_default(),
            division: r.4,
            init_rank: r.5.unwrap_or(0),
            players: r.6,
            games_played: r.7 + r.8,
            wins: r.7,
            losses: r.8,
            spirit_avg: r.9,
            spirit_rank: r.10,
            current_rank: r.5.unwrap_or(0),
            full_logo: r.11,
            small_logo: r.12,
        }),
        _ => Json(TeamDetail {
            id: 0,
            name: "Not Found".to_string(),
            abbreviation: None,
            location: "".to_string(),
            division: 0,
            init_rank: 0,
            players: 0,
            games_played: 0,
            wins: 0,
            losses: 0,
            spirit_avg: 0.0,
            spirit_rank: 0,
            current_rank: 0,
            full_logo: None,
            small_logo: None,
        }),
    }
}

// Team players with stats in one query
pub async fn get_team_players(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Json<Vec<TeamPlayerStat>> {
    let players = sqlx::query_as::<_, TeamPlayerStat>(
        r#"
        SELECT 
            u.id, u.name as full_name, u.common_name,
            COALESCE(SUM(CASE WHEN me.event_type = 0 THEN 1 ELSE 0 END), 0) as goals,
            COALESCE(SUM(CASE WHEN me.event_type = 1 THEN 1 ELSE 0 END), 0) as assists,
            COALESCE(SUM(CASE WHEN me.event_type = 2 THEN 1 ELSE 0 END), 0) as blocks,
            COALESCE(SUM(CASE WHEN me.event_type = 3 THEN 1 ELSE 0 END), 0) as turnovers,
            COALESCE(u.is_captain, 0) as is_captain,
            COALESCE(u.is_spirit_captain, 0) as is_spirit_captain
        FROM users u
        LEFT JOIN match_events me ON me.player_id = u.id
        WHERE u.team_id = ? AND u.role = 2 AND u.deleted_at IS NULL
        GROUP BY u.id, u.name, u.common_name, u.is_captain, u.is_spirit_captain
        "#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    Json(players)
}

// Standings by division using helper sorting (c1-c7 tiebreakers, includes live scores)
pub async fn get_standings(
    State(state): State<crate::AppState>,
    Query(params): Query<DivisionQuery>,
) -> Json<Vec<TeamStanding>> {
    let division = params.division.unwrap_or(0) as i64;

    let sorted =
        crate::helpers::sorting::get_cached_intermediate_standings(&state.db, division).await;

    let standings: Vec<TeamStanding> = sorted
        .iter()
        .map(|t| TeamStanding {
            id: t.team_id,
            name: t.name.clone(),
            abbreviation: t.abbreviation.clone(),
            location: String::new(),
            init_rank: t.init_rank,
            wins: t.wins,
            losses: t.losses,
            points_for: t.points_for,
            points_against: t.points_against,
            spirit_avg: t.spirit_avg,
            small_logo: t.small_logo.clone(),
        })
        .collect();

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
            COALESCE(u.common_name, '') as common_name,
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
        WHERE u.deleted_at IS NULL AND u.team_id IS NOT NULL AND u.role = 2
        GROUP BY u.id, u.name, u.common_name, u.team_id, t.name, t.division
        "#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

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
        "SELECT t.id, t.name, t.abbreviation, t.location, t.full_logo, t.small_logo, t.roster_moves_remaining FROM teams t 
         INNER JOIN users u ON u.team_id = t.id 
         WHERE u.email = ? AND u.role = 3 AND u.deleted_at IS NULL"
    ).bind(&email).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    match team {
        Some(t) => Ok(Json(t)),
        None => Err(axum::http::StatusCode::FORBIDDEN),
    }
}

pub async fn update_poc_team(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<UpdatePocTeamRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let email = extract_email(&headers)?;
    ensure_team_edits_enabled(&state.db).await?;

    let team_id: Option<(i64,)> = sqlx::query_as(
        "SELECT team_id FROM users WHERE email = ? AND role = 3 AND deleted_at IS NULL",
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let team_id = team_id.ok_or(axum::http::StatusCode::FORBIDDEN)?.0;
    let abbreviation = normalize_team_abbreviation(payload.abbreviation.as_deref())?;

    sqlx::query("UPDATE teams SET abbreviation = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(&abbreviation)
        .bind(team_id)
        .execute(&state.db)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    invalidate_team_caches(&state.db, team_id).await;

    Ok(Json(
        serde_json::json!({"success": true, "abbreviation": abbreviation}),
    ))
}

// Get POC's team players
pub async fn get_poc_players(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<PocPlayer>>, axum::http::StatusCode> {
    let email = extract_email(&headers)?;

    let team_id: Option<(i64,)> = sqlx::query_as(
        "SELECT team_id FROM users WHERE email = ? AND role = 3 AND deleted_at IS NULL",
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let team_id = team_id.ok_or(axum::http::StatusCode::FORBIDDEN)?.0;

    let players = sqlx::query_as::<_, PocPlayer>(
        "SELECT id, name, common_name, COALESCE(email, '') as email, phone, COALESCE(is_captain, 0) as is_captain, COALESCE(is_spirit_captain, 0) as is_spirit_captain 
            FROM users WHERE team_id = ? AND role = 2 AND deleted_at IS NULL ORDER BY name"
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
    ensure_team_edits_enabled(&state.db).await?;
    let normalized_name = payload.name.trim();
    let normalized_common_name = payload
        .common_name
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_else(|| normalized_name.to_string());
    let normalized_email = normalize_optional_email(payload.email.as_deref());
    let normalized_phone = normalize_optional_text(payload.phone.as_deref());

    // Validate required fields
    if normalized_name.is_empty() {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    if let Some(ref email_value) = normalized_email {
        let existing: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM users WHERE email = ? AND id != ? AND deleted_at IS NULL",
        )
        .bind(email_value)
        .bind(player_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

        if existing.is_some() {
            return Err(axum::http::StatusCode::CONFLICT);
        }
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let current: Option<PocPlayerCurrentRow> = sqlx::query_as(
        "SELECT p.team_id, p.name, p.common_name, p.email, p.phone, COALESCE(p.is_captain, 0), COALESCE(p.is_spirit_captain, 0), t.roster_moves_remaining
         FROM users p
         INNER JOIN users poc ON poc.team_id = p.team_id
         INNER JOIN teams t ON t.id = p.team_id
         WHERE p.id = ? AND p.role = 2 AND poc.email = ? AND poc.role = 3 AND poc.deleted_at IS NULL AND p.deleted_at IS NULL AND t.deleted_at IS NULL"
    )
    .bind(player_id)
    .bind(&email)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let (
        team_id,
        current_name,
        current_common_name,
        current_email,
        current_phone,
        current_captain,
        current_spirit_captain,
        remaining_moves,
    ) = current.ok_or(axum::http::StatusCode::FORBIDDEN)?;

    let current_common_name = current_common_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(current_name.as_str());

    let roster_move_change = current_name != normalized_name
        || current_email != normalized_email
        || current_phone != normalized_phone
        || current_captain != payload.is_captain as i64
        || current_spirit_captain != payload.is_spirit_captain as i64;
    let common_name_changed = current_common_name != normalized_common_name;

    let updated_remaining = if roster_move_change {
        let budget_update = sqlx::query(
            "UPDATE teams SET roster_moves_remaining = roster_moves_remaining - 1, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND roster_moves_remaining > 0"
        )
        .bind(team_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

        if budget_update.rows_affected() == 0 {
            return Err(axum::http::StatusCode::CONFLICT);
        }

        remaining_moves - 1
    } else {
        remaining_moves
    };

    sqlx::query(
        "UPDATE users SET name = ?, common_name = ?, email = ?, phone = ?, is_captain = ?, is_spirit_captain = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
    ).bind(normalized_name).bind(normalized_common_name).bind(&normalized_email).bind(&normalized_phone)
     .bind(payload.is_captain).bind(payload.is_spirit_captain).bind(player_id)
     .execute(&mut *tx).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    if !roster_move_change && !common_name_changed {
        tx.commit()
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

        cache::invalidate_player_stats().await;

        return Ok(Json(
            serde_json::json!({"success": true, "roster_moves_remaining": updated_remaining}),
        ));
    }

    tx.commit()
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    cache::invalidate_player_stats().await;

    Ok(Json(
        serde_json::json!({"success": true, "roster_moves_remaining": updated_remaining}),
    ))
}

// Add a new player to POC's team
pub async fn add_poc_player(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<AddPlayerRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let email = extract_email(&headers)?;
    ensure_team_edits_enabled(&state.db).await?;
    let normalized_name = payload.name.trim();
    let normalized_common_name = payload
        .common_name
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_else(|| normalized_name.to_string());
    let normalized_email = normalize_optional_email(payload.email.as_deref());
    let normalized_phone = normalize_optional_text(payload.phone.as_deref());

    // Validate required fields
    if normalized_name.is_empty() {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    let team_id: Option<(i64,)> = sqlx::query_as(
        "SELECT team_id FROM users WHERE email = ? AND role = 3 AND deleted_at IS NULL",
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let team_id = team_id.ok_or(axum::http::StatusCode::FORBIDDEN)?.0;

    let player_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM users WHERE team_id = ? AND role = 2 AND deleted_at IS NULL",
    )
    .bind(team_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    if player_count.0 >= MAX_TEAM_PLAYERS {
        return Err(axum::http::StatusCode::CONFLICT);
    }

    if let Some(ref email_value) = normalized_email {
        let existing: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM users WHERE email = ? AND deleted_at IS NULL")
                .bind(email_value)
                .fetch_optional(&state.db)
                .await
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

        if existing.is_some() {
            return Err(axum::http::StatusCode::CONFLICT);
        }
    }

    let result = sqlx::query(
        "INSERT INTO users (name, common_name, email, phone, team_id, role, is_captain, is_spirit_captain) VALUES (?, ?, ?, ?, ?, 2, ?, ?)"
    ).bind(normalized_name).bind(normalized_common_name).bind(&normalized_email).bind(&normalized_phone)
     .bind(team_id).bind(payload.is_captain).bind(payload.is_spirit_captain)
     .execute(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    cache::invalidate_player_stats().await;

    Ok(Json(
        serde_json::json!({"success": true, "id": result.last_insert_rowid()}),
    ))
}

// Delete a player from POC's team
pub async fn delete_poc_player(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Path(player_id): Path<i64>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let email = extract_email(&headers)?;
    ensure_team_edits_enabled(&state.db).await?;

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let current: Option<(i64, i64)> = sqlx::query_as(
        "SELECT p.team_id, t.roster_moves_remaining
         FROM users p
         INNER JOIN users poc ON poc.team_id = p.team_id
         INNER JOIN teams t ON t.id = p.team_id
         WHERE p.id = ? AND p.role = 2 AND poc.email = ? AND poc.role = 3 AND poc.deleted_at IS NULL AND p.deleted_at IS NULL AND t.deleted_at IS NULL"
    )
    .bind(player_id)
    .bind(&email)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let (team_id, remaining_moves) = current.ok_or(axum::http::StatusCode::FORBIDDEN)?;

    let budget_update = sqlx::query(
        "UPDATE teams SET roster_moves_remaining = roster_moves_remaining - 1, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND roster_moves_remaining > 0"
    )
    .bind(team_id)
    .execute(&mut *tx)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    if budget_update.rows_affected() == 0 {
        return Err(axum::http::StatusCode::CONFLICT);
    }

    sqlx::query("UPDATE users SET deleted_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(player_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit()
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    cache::invalidate_player_stats().await;

    Ok(Json(
        serde_json::json!({"success": true, "roster_moves_remaining": remaining_moves - 1}),
    ))
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
    ensure_team_edits_enabled(&state.db).await?;

    let team_id: Option<(i64,)> = sqlx::query_as(
        "SELECT team_id FROM users WHERE email = ? AND role = 3 AND deleted_at IS NULL",
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let team_id = team_id.ok_or(axum::http::StatusCode::FORBIDDEN)?.0;

    sqlx::query(
        "UPDATE teams SET full_logo = ?, small_logo = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
    ).bind(&payload.full_logo).bind(&payload.small_logo).bind(team_id)
     .execute(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    invalidate_team_caches(&state.db, team_id).await;

    Ok(Json(serde_json::json!({"success": true})))
}

async fn invalidate_team_caches(db: &sqlx::SqlitePool, team_id: i64) {
    let division: Option<(i64,)> =
        sqlx::query_as("SELECT division FROM teams WHERE id = ? AND deleted_at IS NULL")
            .bind(team_id)
            .fetch_optional(db)
            .await
            .ok()
            .flatten();

    if let Some((division,)) = division {
        cache::invalidate_division(division).await;
    }
    cache::invalidate_teams_list().await;
}

async fn ensure_team_edits_enabled(db: &SqlitePool) -> Result<(), axum::http::StatusCode> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT is_enabled FROM reporting_round_settings WHERE round_key = ?",
    )
    .bind(TEAM_EDITS_ROUND_KEY)
    .fetch_optional(db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    if matches!(row, Some((0,))) {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }

    Ok(())
}

fn extract_email(headers: &axum::http::HeaderMap) -> Result<String, axum::http::StatusCode> {
    crate::helpers::auth::extract_email(headers)
}

fn normalize_optional_email(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    Some(value.to_ascii_lowercase())
}

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    Some(value.to_string())
}

fn normalize_team_abbreviation(
    value: Option<&str>,
) -> Result<Option<String>, axum::http::StatusCode> {
    let Some(value) = value.map(str::trim) else {
        return Ok(None);
    };

    if value.is_empty() {
        return Ok(None);
    }

    if value.len() > 5 || !value.chars().all(|char| char.is_ascii_alphanumeric()) {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    Ok(Some(value.to_string()))
}
