use crate::helpers::cache;
use crate::helpers::sorting;
use axum::{
    Json,
    extract::Query,
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::{convert::Infallible, time::Duration};
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

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
    pub t1_abbreviation: Option<String>,
    pub t2_abbreviation: Option<String>,
    pub t1_score: i64,
    pub t2_score: i64,
    pub t1_spirit: Option<i64>,
    pub t2_spirit: Option<i64>,
    pub field_name: String,
    pub time: String,
    pub possession: Option<i64>,
    pub stream_url: Option<String>,
    pub match_type: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MatchEvent {
    pub id: i64,
    pub player_id: Option<i64>,
    pub player_name: String,
    pub team_id: i64,
    pub event_type: i64,
    pub actor_user_id: Option<i64>,
    pub created_at: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MatchPlayer {
    pub id: i64,
    pub name: String,
    pub common_name: Option<String>,
    pub team_id: i64,
}

#[derive(Serialize)]
pub struct MatchDetail {
    pub id: i64,
    pub t1_id: i64,
    pub t2_id: i64,
    pub t1_name: String,
    pub t2_name: String,
    pub t1_abbreviation: Option<String>,
    pub t2_abbreviation: Option<String>,
    pub t1_score: i64,
    pub t2_score: i64,
    pub t1_spirit: Option<i64>,
    pub t2_spirit: Option<i64>,
    pub t1_division: i64,
    pub t2_division: i64,
    pub t1_small_logo: Option<String>,
    pub t2_small_logo: Option<String>,
    pub possession: Option<i64>,
    pub match_type: i64,
    pub reporting_enabled: bool,
    pub field_name: String,
    pub time: String,
    pub stream_url: Option<String>,
    pub started_at: Option<String>,
    pub updated_at: String,
    pub server_time: String,
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
    pub t1_abbreviation: Option<String>,
    pub t2_abbreviation: Option<String>,
    pub field_name: String,
    pub time: String,
    pub possession: Option<i64>,
    pub match_type: i64,
    pub reporting_enabled: bool,
}

#[derive(Serialize)]
pub struct ReportingRoundSetting {
    pub round_key: i64,
    pub label: String,
    pub is_enabled: bool,
}

#[derive(sqlx::FromRow)]
struct ReportingRoundSettingRow {
    round_key: i64,
    label: String,
    is_enabled: i64,
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

#[derive(Deserialize)]
pub struct UpdateReportingRoundSettingRequest {
    pub is_enabled: bool,
}

#[derive(Deserialize)]
pub struct LiveUpdatesQuery {
    pub match_id: Option<i64>,
}

struct VolunteerAccess {
    user_id: i64,
    role: i64,
    team_id: Option<i64>,
}

type LegacySpiritMatchInfo = (i64, i64, Option<i64>, Option<i64>, Option<i64>);

fn reporting_round_label(match_type: i64) -> &'static str {
    match match_type {
        1 => "Round 1",
        2 => "Round 2",
        3 => "Round 3",
        4 => "Round 4",
        5 => "Round 5",
        6 => "Round 6",
        1001 => "Playoffs",
        1002 => "Finals",
        _ => "Reporting",
    }
}

fn map_reporting_round_setting(row: ReportingRoundSettingRow) -> ReportingRoundSetting {
    ReportingRoundSetting {
        round_key: row.round_key,
        label: row.label,
        is_enabled: row.is_enabled != 0,
    }
}

async fn load_match_timing_snapshot(
    db: &sqlx::SqlitePool,
    match_id: i64,
) -> Result<(Option<String>, String, String), axum::http::StatusCode> {
    let timing: Option<(Option<String>, String)> = sqlx::query_as(
        r#"
        SELECT
            CASE
                WHEN started_at IS NULL THEN NULL
                ELSE strftime('%Y-%m-%dT%H:%M:%SZ', started_at)
            END AS started_at,
            COALESCE(
                strftime('%Y-%m-%dT%H:%M:%SZ', updated_at),
                strftime('%Y-%m-%dT%H:%M:%SZ', created_at),
                ''
            ) AS updated_at
        FROM matches
        WHERE id = ? AND deleted_at IS NULL
        "#,
    )
    .bind(match_id)
    .fetch_optional(db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let (started_at, updated_at) = timing.ok_or(axum::http::StatusCode::NOT_FOUND)?;
    Ok((started_at, updated_at, chrono::Utc::now().to_rfc3339()))
}

pub async fn get_reporting_round_settings(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<ReportingRoundSetting>>, axum::http::StatusCode> {
    verify_volunteer_user(&state, &headers).await?;

    let rows = sqlx::query_as::<_, ReportingRoundSettingRow>(
        r#"SELECT round_key, label, is_enabled
           FROM reporting_round_settings
           ORDER BY round_key ASC"#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(
        rows.into_iter().map(map_reporting_round_setting).collect(),
    ))
}

pub async fn stream_live_updates(
    State(state): State<crate::AppState>,
    Query(params): Query<LiveUpdatesQuery>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.live_updates.subscribe();
    let stream_match_id = params.match_id;

    let stream = BroadcastStream::new(receiver).filter_map(move |message| {
        let update = message.ok()?;
        if let Some(match_id) = stream_match_id
            && !update.matches_match_id(match_id)
        {
            return None;
        }

        let mut event_data = serde_json::to_value(&update).ok()?;
        if let Some(object) = event_data.as_object_mut() {
            object.insert(
                "server_time".to_string(),
                serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
            );
        }

        Event::default()
            .event("update")
            .json_data(&event_data)
            .ok()
            .map(Ok)
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

pub async fn update_reporting_round_setting(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Path(round_key): Path<i64>,
    Json(payload): Json<UpdateReportingRoundSettingRequest>,
) -> Result<Json<ReportingRoundSetting>, axum::http::StatusCode> {
    verify_super_user(&state, &headers).await?;

    let result = sqlx::query(
        r#"UPDATE reporting_round_settings
           SET is_enabled = ?, updated_at = CURRENT_TIMESTAMP
           WHERE round_key = ?"#,
    )
    .bind(if payload.is_enabled { 1 } else { 0 })
    .bind(round_key)
    .execute(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(axum::http::StatusCode::NOT_FOUND);
    }

    let row = sqlx::query_as::<_, ReportingRoundSettingRow>(
        r#"SELECT round_key, label, is_enabled
           FROM reporting_round_settings
           WHERE round_key = ?"#,
    )
    .bind(round_key)
    .fetch_one(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    state.live_updates.publish_reporting_rounds_updated();

    Ok(Json(map_reporting_round_setting(row)))
}

// Get fields
pub async fn get_fields(State(state): State<crate::AppState>) -> Json<Vec<Field>> {
    let fields =
        sqlx::query_as::<_, Field>("SELECT id, name, hints, map_link FROM fields ORDER BY id")
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();
    Json(fields)
}

// Get tournament stats
pub async fn get_stats(State(state): State<crate::AppState>) -> Json<Stats> {
    let stats: (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT 
            (SELECT COUNT(*) FROM teams WHERE deleted_at IS NULL),
            (SELECT COUNT(*) FROM users WHERE deleted_at IS NULL AND team_id IS NOT NULL AND role = 2),
            (SELECT COALESCE(SUM(t1_score + t2_score), 0) FROM matches WHERE deleted_at IS NULL),
            (SELECT COUNT(*) FROM matches WHERE deleted_at IS NULL AND possession >= 3),
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
pub async fn get_team_matches(
    State(state): State<crate::AppState>,
    Path(team_id): Path<i64>,
) -> Json<Vec<TeamMatch>> {
    let matches = sqlx::query_as::<_, TeamMatch>(
        r#"
         SELECT m.id, m.t1_id, m.t2_id, t1.name as t1_name, t2.name as t2_name,
                             t1.abbreviation as t1_abbreviation, t2.abbreviation as t2_abbreviation,
               m.t1_score, m.t2_score, m.t1_spirit, m.t2_spirit, 
               COALESCE(f.name, '') as field_name, COALESCE(m.time, '') as time,
             m.possession, m.stream_url, m.type as match_type
        FROM matches m
        JOIN teams t1 ON t1.id = m.t1_id
        JOIN teams t2 ON t2.id = m.t2_id
        LEFT JOIN fields f ON f.id = m.field_id
        WHERE m.deleted_at IS NULL AND (m.t1_id = ? OR m.t2_id = ?)
        ORDER BY m.time DESC
        "#,
    )
    .bind(team_id)
    .bind(team_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    Json(matches)
}

// Get match events (live log)
pub async fn get_match_events(
    State(state): State<crate::AppState>,
    Path(match_id): Path<i64>,
) -> Json<Vec<MatchEvent>> {
    let events = sqlx::query_as::<_, MatchEvent>(
        r#"
        SELECT me.id, me.player_id, 
               COALESCE(u.name, '') as player_name, 
               COALESCE(me.team_id, u.team_id) as team_id, 
               me.event_type, 
             me.actor_user_id,
               COALESCE(me.created_at, '') as created_at
        FROM match_events me
        LEFT JOIN users u ON u.id = me.player_id
        WHERE me.match_id = ?
        ORDER BY me.created_at ASC
        "#,
    )
    .bind(match_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    Json(events)
}

// Get match detail with players and events for live reporting
pub async fn get_match_detail(
    State(state): State<crate::AppState>,
    Path(match_id): Path<i64>,
) -> Json<MatchDetail> {
    let match_row = sqlx::query(
        r#"
        SELECT m.id, m.t1_id, m.t2_id, t1.name, t2.name, t1.abbreviation, t2.abbreviation, m.t1_score, m.t2_score, 
               m.t1_spirit, m.t2_spirit, t1.division, t2.division, t1.small_logo, t2.small_logo, m.possession,
               m.type, COALESCE(rrs.is_enabled, 0) as reporting_enabled,
             COALESCE(f.name, '') as field_name, COALESCE(m.time, '') as time, m.stream_url,
             CASE WHEN m.started_at IS NULL THEN NULL ELSE strftime('%Y-%m-%dT%H:%M:%SZ', m.started_at) END as started_at,
             COALESCE(strftime('%Y-%m-%dT%H:%M:%SZ', m.updated_at), strftime('%Y-%m-%dT%H:%M:%SZ', m.created_at), '') as updated_at
        FROM matches m
        JOIN teams t1 ON t1.id = m.t1_id
        JOIN teams t2 ON t2.id = m.t2_id
        LEFT JOIN fields f ON f.id = m.field_id
        LEFT JOIN reporting_round_settings rrs ON rrs.round_key = m.type
        WHERE m.id = ? AND m.deleted_at IS NULL
        "#
    ).bind(match_id).fetch_optional(&state.db).await.unwrap_or(None);

    let (
        id,
        t1_id,
        t2_id,
        t1_name,
        t2_name,
        t1_abbreviation,
        t2_abbreviation,
        t1_score,
        t2_score,
        t1_spirit,
        t2_spirit,
        t1_division,
        t2_division,
        t1_small_logo,
        t2_small_logo,
        possession,
        match_type,
        reporting_enabled,
        field_name,
        time,
        stream_url,
        started_at,
        updated_at,
    ) = match match_row {
        Some(row) => (
            row.get::<i64, _>(0),
            row.get::<i64, _>(1),
            row.get::<i64, _>(2),
            row.get::<String, _>(3),
            row.get::<String, _>(4),
            row.get::<Option<String>, _>(5),
            row.get::<Option<String>, _>(6),
            row.get::<i64, _>(7),
            row.get::<i64, _>(8),
            row.get::<Option<i64>, _>(9),
            row.get::<Option<i64>, _>(10),
            row.get::<i64, _>(11),
            row.get::<i64, _>(12),
            row.get::<Option<String>, _>(13),
            row.get::<Option<String>, _>(14),
            row.get::<Option<i64>, _>(15),
            row.get::<i64, _>(16),
            row.get::<i64, _>(17),
            row.get::<String, _>(18),
            row.get::<String, _>(19),
            row.get::<Option<String>, _>(20),
            row.get::<Option<String>, _>(21),
            row.get::<String, _>(22),
        ),
        None => (
            0,
            0,
            0,
            "".to_string(),
            "".to_string(),
            None,
            None,
            0,
            0,
            None,
            None,
            1,
            1,
            None,
            None,
            None,
            0,
            0,
            "".to_string(),
            "".to_string(),
            None,
            None,
            "".to_string(),
        ),
    };

    let server_time = chrono::Utc::now().to_rfc3339();

    let players = sqlx::query_as::<_, MatchPlayer>(
        "SELECT id, name, common_name, team_id FROM users WHERE team_id IN (?, ?) AND role = 2 AND deleted_at IS NULL"
    ).bind(t1_id).bind(t2_id).fetch_all(&state.db).await.unwrap_or_default();

    let events = sqlx::query_as::<_, MatchEvent>(
        r#"
        SELECT me.id, me.player_id, 
               COALESCE(u.name, '') as player_name, 
               COALESCE(me.team_id, u.team_id) as team_id, 
               me.event_type,
             me.actor_user_id,
               COALESCE(me.created_at, '') as created_at
        FROM match_events me 
        LEFT JOIN users u ON u.id = me.player_id
        WHERE me.match_id = ? ORDER BY me.created_at ASC
        "#,
    )
    .bind(match_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(MatchDetail {
        id,
        t1_id,
        t2_id,
        t1_name,
        t2_name,
        t1_abbreviation,
        t2_abbreviation,
        t1_score,
        t2_score,
        t1_spirit,
        t2_spirit,
        t1_division,
        t2_division,
        t1_small_logo,
        t2_small_logo,
        possession,
        match_type,
        reporting_enabled: reporting_enabled != 0,
        field_name,
        time,
        stream_url,
        started_at,
        updated_at,
        server_time,
        players,
        events,
    })
}

// Get upcoming matches for volunteers (next 16 matches)
pub async fn get_upcoming_matches(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<UpcomingMatch>>, axum::http::StatusCode> {
    let access = verify_volunteer_user(&state, &headers).await?;

    let matches = if access.role == 3 {
        let team_id = access.team_id.ok_or(axum::http::StatusCode::FORBIDDEN)?;
        sqlx::query_as::<_, UpcomingMatch>(
            r#"
                 SELECT m.id, m.t1_id, m.t2_id, t1.name as t1_name, t2.name as t2_name,
                     t1.abbreviation as t1_abbreviation, t2.abbreviation as t2_abbreviation,
                   COALESCE(f.name, '') as field_name, COALESCE(m.time, '') as time,
                                     m.possession, m.type as match_type, COALESCE(rrs.is_enabled, 0) as reporting_enabled
            FROM matches m
            JOIN teams t1 ON t1.id = m.t1_id
            JOIN teams t2 ON t2.id = m.t2_id
            LEFT JOIN fields f ON f.id = m.field_id
                        LEFT JOIN reporting_round_settings rrs ON rrs.round_key = m.type
            WHERE m.deleted_at IS NULL
              AND (m.possession IS NULL OR m.possession <= 2)
              AND (m.t1_id = ? OR m.t2_id = ?)
            ORDER BY m.time ASC
            LIMIT 16
            "#
        ).bind(team_id).bind(team_id).fetch_all(&state.db).await.unwrap_or_default()
    } else {
        sqlx::query_as::<_, UpcomingMatch>(
            r#"
                 SELECT m.id, m.t1_id, m.t2_id, t1.name as t1_name, t2.name as t2_name,
                     t1.abbreviation as t1_abbreviation, t2.abbreviation as t2_abbreviation,
                   COALESCE(f.name, '') as field_name, COALESCE(m.time, '') as time,
                   m.possession, m.type as match_type, COALESCE(rrs.is_enabled, 0) as reporting_enabled
            FROM matches m
            JOIN teams t1 ON t1.id = m.t1_id
            JOIN teams t2 ON t2.id = m.t2_id
            LEFT JOIN fields f ON f.id = m.field_id
            LEFT JOIN reporting_round_settings rrs ON rrs.round_key = m.type
            WHERE m.deleted_at IS NULL AND (m.possession IS NULL OR m.possession <= 2)
            ORDER BY m.time ASC LIMIT 16
            "#
        ).fetch_all(&state.db).await.unwrap_or_default()
    };

    Ok(Json(matches))
}

// Start match (set possession based on request)
pub async fn start_match(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Path(match_id): Path<i64>,
    Json(payload): Json<StartMatchRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let access = verify_volunteer_user(&state, &headers).await?;
    ensure_match_access(&state, &access, match_id).await?;
    ensure_reporting_round_enabled(&state.db, &access, match_id).await?;
    ensure_match_not_finalized(&state.db, match_id).await?;

    let possession = payload.possession.unwrap_or(1);
    if possession != 1 && possession != 2 {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    let current: Option<(Option<i64>,)> =
        sqlx::query_as("SELECT possession FROM matches WHERE id = ? AND deleted_at IS NULL")
            .bind(match_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let current_possession = current.ok_or(axum::http::StatusCode::NOT_FOUND)?.0;
    if current_possession.is_some() {
        return Err(axum::http::StatusCode::CONFLICT);
    }

    sqlx::query("UPDATE matches SET possession = ?, started_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(possession)
        .bind(match_id)
        .execute(&state.db)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    state.live_updates.publish_match_updated(match_id);

    let (started_at, _updated_at, server_time) =
        load_match_timing_snapshot(&state.db, match_id).await?;

    Ok(Json(
        serde_json::json!({"success": true, "possession": possession, "started_at": started_at, "server_time": server_time}),
    ))
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
    let access = verify_volunteer_user(&state, &headers).await?;
    ensure_match_access(&state, &access, match_id).await?;
    ensure_reporting_round_enabled(&state.db, &access, match_id).await?;
    ensure_match_not_finalized(&state.db, match_id).await?;

    let info: Option<(Option<i64>, i64, i64, i64, i64)> = sqlx::query_as(
        r#"SELECT m.possession, t.division, m.type, m.t1_score, m.t2_score
           FROM matches m
           JOIN teams t ON t.id = m.t1_id
           WHERE m.id = ? AND m.deleted_at IS NULL"#,
    )
    .bind(match_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let (possession, division, match_type, t1_score, t2_score) =
        info.ok_or(axum::http::StatusCode::NOT_FOUND)?;
    if possession.is_none() || possession.unwrap_or(0) >= 3 {
        return Err(axum::http::StatusCode::CONFLICT);
    }
    if match_type >= 1000 && t1_score == t2_score {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    sqlx::query("UPDATE matches SET possession = 3, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(match_id)
        .execute(&state.db)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let auto_action =
        crate::controllers::scheduling::auto_advance_division_if_ready(&state.db, division).await;
    sorting::refresh_intermediate_standings_cache(&state.db, division).await;
    cache::invalidate_division(division).await;
    cache::invalidate_player_stats().await;
    state.live_updates.publish_match_updated(match_id);

    let (started_at, updated_at, server_time) =
        load_match_timing_snapshot(&state.db, match_id).await?;

    Ok(Json(
        serde_json::json!({"success": true, "auto_action": auto_action, "started_at": started_at, "updated_at": updated_at, "server_time": server_time}),
    ))
}

// Record match event (score/turnover/block)
pub async fn record_event(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Path(match_id): Path<i64>,
    Json(payload): Json<MatchEventRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let access = verify_volunteer_user(&state, &headers).await?;
    ensure_match_access(&state, &access, match_id).await?;
    ensure_reporting_round_enabled(&state.db, &access, match_id).await?;
    ensure_match_not_finalized(&state.db, match_id).await?;
    let actor_user_id = access.user_id;

    // Validate event_type (0=goal, 1=assist, 2=block, 3=turnover)
    if payload.event_type < 0 || payload.event_type > 3 {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    let match_info: Option<(i64, i64, i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT t1_id, t2_id, t1_score, t2_score, possession FROM matches WHERE id = ? AND deleted_at IS NULL"
    ).bind(match_id).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let (t1_id, t2_id, mut t1_score, mut t2_score, possession) =
        match_info.ok_or(axum::http::StatusCode::NOT_FOUND)?;

    let current_pos = possession.ok_or(axum::http::StatusCode::CONFLICT)?;
    if current_pos >= 3 {
        return Err(axum::http::StatusCode::CONFLICT);
    }

    let mut new_pos = current_pos;
    let offense_team_id = if current_pos == 1 { t1_id } else { t2_id };
    let defense_team_id = if current_pos == 1 { t2_id } else { t1_id };

    // Record event
    if let Some(player_id) = payload.player_id {
        // Player specified - get their team_id
        let player_team: Option<(i64,)> = sqlx::query_as(
            "SELECT team_id FROM users WHERE id = ? AND role = 2 AND deleted_at IS NULL",
        )
        .bind(player_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

        let player_team_id = player_team
            .map(|t| t.0)
            .ok_or(axum::http::StatusCode::BAD_REQUEST)?;
        if player_team_id != t1_id && player_team_id != t2_id {
            return Err(axum::http::StatusCode::BAD_REQUEST);
        }

        let expected_team_id = match payload.event_type {
            0 | 1 | 3 => offense_team_id,
            2 => defense_team_id,
            _ => return Err(axum::http::StatusCode::BAD_REQUEST),
        };

        if player_team_id != expected_team_id {
            return Err(axum::http::StatusCode::BAD_REQUEST);
        }

        sqlx::query("INSERT INTO match_events (match_id, player_id, team_id, event_type, actor_user_id) VALUES (?, ?, ?, ?, ?)")
            .bind(match_id).bind(player_id).bind(player_team_id).bind(payload.event_type).bind(actor_user_id)
            .execute(&state.db).await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    } else {
        if payload.event_type == 2 {
            return Err(axum::http::StatusCode::BAD_REQUEST);
        }

        // Team-level event (without player)
        sqlx::query("INSERT INTO match_events (match_id, player_id, team_id, event_type, actor_user_id) VALUES (?, NULL, ?, ?, ?)")
            .bind(match_id).bind(offense_team_id).bind(payload.event_type).bind(actor_user_id)
            .execute(&state.db).await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    match payload.event_type {
        0 => {
            // Goal - increment score for possessing team, switch possession
            if current_pos == 1 {
                t1_score += 1;
            } else {
                t2_score += 1;
            }
            new_pos = if current_pos == 1 { 2 } else { 1 };
        }
        2 => {
            // Block - switch possession
            new_pos = if current_pos == 1 { 2 } else { 1 };
        }
        3 => {
            // Turnover - switch possession
            new_pos = if current_pos == 1 { 2 } else { 1 };
        }
        _ => {}
    }

    sqlx::query("UPDATE matches SET t1_score = ?, t2_score = ?, possession = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(t1_score).bind(t2_score).bind(new_pos).bind(match_id)
        .execute(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let division: Option<(i64,)> =
        sqlx::query_as("SELECT division FROM teams WHERE id = ? AND deleted_at IS NULL")
            .bind(t1_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some((division,)) = division {
        sorting::refresh_intermediate_standings_cache(&state.db, division).await;
        cache::invalidate_player_stats().await;
    }

    state.live_updates.publish_match_updated(match_id);

    let (started_at, _updated_at, server_time) =
        load_match_timing_snapshot(&state.db, match_id).await?;

    Ok(Json(
        serde_json::json!({"success": true, "t1_score": t1_score, "t2_score": t2_score, "possession": new_pos, "started_at": started_at, "server_time": server_time}),
    ))
}

pub async fn switch_possession(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Path(match_id): Path<i64>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let access = verify_volunteer_user(&state, &headers).await?;
    ensure_match_access(&state, &access, match_id).await?;
    ensure_reporting_round_enabled(&state.db, &access, match_id).await?;
    ensure_match_not_finalized(&state.db, match_id).await?;

    let match_info: Option<(i64, Option<i64>)> =
        sqlx::query_as("SELECT t1_id, possession FROM matches WHERE id = ? AND deleted_at IS NULL")
            .bind(match_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let (t1_id, possession) = match_info.ok_or(axum::http::StatusCode::NOT_FOUND)?;
    let current_pos = possession.ok_or(axum::http::StatusCode::CONFLICT)?;
    if current_pos >= 3 {
        return Err(axum::http::StatusCode::CONFLICT);
    }

    let new_pos = if current_pos == 1 { 2 } else { 1 };

    sqlx::query("UPDATE matches SET possession = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(new_pos)
        .bind(match_id)
        .execute(&state.db)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let division: Option<(i64,)> =
        sqlx::query_as("SELECT division FROM teams WHERE id = ? AND deleted_at IS NULL")
            .bind(t1_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some((division,)) = division {
        sorting::refresh_intermediate_standings_cache(&state.db, division).await;
        cache::invalidate_division(division).await;
    }

    state.live_updates.publish_match_updated(match_id);

    let (started_at, _updated_at, server_time) =
        load_match_timing_snapshot(&state.db, match_id).await?;

    Ok(Json(
        serde_json::json!({"success": true, "possession": new_pos, "started_at": started_at, "server_time": server_time}),
    ))
}

// Undo latest event
pub async fn undo_event(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Path(match_id): Path<i64>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let access = verify_volunteer_user(&state, &headers).await?;
    ensure_match_access(&state, &access, match_id).await?;
    ensure_reporting_round_enabled(&state.db, &access, match_id).await?;
    ensure_match_not_finalized(&state.db, match_id).await?;

    let match_info: Option<(i64, i64, i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT t1_id, t2_id, t1_score, t2_score, possession FROM matches WHERE id = ?",
    )
    .bind(match_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let (t1_id, _t2_id, mut t1_score, mut t2_score, possession) =
        match_info.ok_or(axum::http::StatusCode::NOT_FOUND)?;

    let mut current_pos = possession.unwrap_or(1);
    if current_pos >= 3 {
        return Err(axum::http::StatusCode::CONFLICT);
    }

    // Get latest event
    let latest_event: Option<(i64, Option<i64>, i64, String)> = sqlx::query_as(
        "SELECT id, player_id, event_type, created_at FROM match_events WHERE match_id = ? ORDER BY created_at DESC, id DESC LIMIT 1"
    ).bind(match_id).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some((event_id, _player_id, event_type, created_at)) = latest_event {
        let mut deleted_goal = event_type == 0;

        // Delete the event
        sqlx::query("DELETE FROM match_events WHERE id = ?")
            .bind(event_id)
            .execute(&state.db)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

        // For score events, check for paired assist within 1 second
        if event_type == 0 || event_type == 1 {
            let paired_event: Option<(i64, i64)> = sqlx::query_as(
                r#"SELECT id, event_type FROM match_events 
                   WHERE match_id = ? AND ABS(strftime('%s', created_at) - strftime('%s', ?)) <= 1
                   AND (event_type = 0 OR event_type = 1)
                   ORDER BY created_at DESC, id DESC LIMIT 1"#,
            )
            .bind(match_id)
            .bind(&created_at)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

            if let Some((paired_id, paired_type)) = paired_event {
                deleted_goal |= paired_type == 0;
                sqlx::query("DELETE FROM match_events WHERE id = ?")
                    .bind(paired_id)
                    .execute(&state.db)
                    .await
                    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
            }
        }

        // Revert score and possession based on event type
        if deleted_goal {
            if current_pos == 1 {
                t2_score = (t2_score - 1).max(0);
            } else {
                t1_score = (t1_score - 1).max(0);
            }
            current_pos = if current_pos == 1 { 2 } else { 1 };
        } else {
            match event_type {
                2 | 3 => {
                    // Block/Turnover - switch possession back
                    current_pos = if current_pos == 1 { 2 } else { 1 };
                }
                _ => {}
            }
        }

        sqlx::query("UPDATE matches SET t1_score = ?, t2_score = ?, possession = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(t1_score).bind(t2_score).bind(current_pos).bind(match_id)
            .execute(&state.db).await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

        let division: Option<(i64,)> =
            sqlx::query_as("SELECT division FROM teams WHERE id = ? AND deleted_at IS NULL")
                .bind(t1_id)
                .fetch_optional(&state.db)
                .await
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
        if let Some((division,)) = division {
            sorting::refresh_intermediate_standings_cache(&state.db, division).await;
            cache::invalidate_player_stats().await;
        }

        state.live_updates.publish_match_updated(match_id);

        let (started_at, _updated_at, server_time) =
            load_match_timing_snapshot(&state.db, match_id).await?;

        Ok(Json(
            serde_json::json!({"success": true, "t1_score": t1_score, "t2_score": t2_score, "possession": current_pos, "started_at": started_at, "server_time": server_time}),
        ))
    } else {
        Ok(Json(
            serde_json::json!({"success": false, "message": "No events to undo"}),
        ))
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
        "SELECT team_id FROM users WHERE email = ? AND role = 3 AND deleted_at IS NULL",
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let team_id = poc_team.ok_or(axum::http::StatusCode::FORBIDDEN)?.0;

    let match_info: Option<LegacySpiritMatchInfo> = sqlx::query_as(
        "SELECT t1_id, t2_id, t1_spirit, t2_spirit, possession FROM matches WHERE id = ? AND deleted_at IS NULL"
    ).bind(match_id).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let (t1_id, t2_id, t1_spirit, t2_spirit, possession) =
        match_info.ok_or(axum::http::StatusCode::NOT_FOUND)?;

    // Must be done (possession >= 3)
    if possession.unwrap_or(0) < 3 {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    // POC rates the OTHER team's spirit
    if team_id == t1_id {
        if t2_spirit.is_some() {
            return Err(axum::http::StatusCode::CONFLICT);
        }
        sqlx::query(
            "UPDATE matches SET t2_spirit = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(payload.spirit_score)
        .bind(match_id)
        .execute(&state.db)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    } else if team_id == t2_id {
        if t1_spirit.is_some() {
            return Err(axum::http::StatusCode::CONFLICT);
        }
        sqlx::query(
            "UPDATE matches SET t1_spirit = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(payload.spirit_score)
        .bind(match_id)
        .execute(&state.db)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    } else {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }

    let division: Option<(i64,)> =
        sqlx::query_as("SELECT division FROM teams WHERE id = ? AND deleted_at IS NULL")
            .bind(t1_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some((division,)) = division {
        sorting::refresh_intermediate_standings_cache(&state.db, division).await;
    }

    state.live_updates.publish_match_updated(match_id);

    Ok(Json(serde_json::json!({"success": true})))
}

// Get POC's team matches for spirit score submission
pub async fn get_poc_matches(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<TeamMatch>>, axum::http::StatusCode> {
    let email = extract_email(&headers)?;

    let poc_team: Option<(i64,)> = sqlx::query_as(
        "SELECT team_id FROM users WHERE email = ? AND role = 3 AND deleted_at IS NULL",
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let team_id = poc_team.ok_or(axum::http::StatusCode::FORBIDDEN)?.0;

    let matches = sqlx::query_as::<_, TeamMatch>(
        r#"
        SELECT m.id, m.t1_id, m.t2_id, t1.name as t1_name, t2.name as t2_name,
                             t1.abbreviation as t1_abbreviation, t2.abbreviation as t2_abbreviation,
               m.t1_score, m.t2_score, m.t1_spirit, m.t2_spirit,
               COALESCE(f.name, '') as field_name, COALESCE(m.time, '') as time,
             m.possession, m.stream_url, m.type as match_type
        FROM matches m
        JOIN teams t1 ON t1.id = m.t1_id
        JOIN teams t2 ON t2.id = m.t2_id
        LEFT JOIN fields f ON f.id = m.field_id
        WHERE m.deleted_at IS NULL AND (m.t1_id = ? OR m.t2_id = ?)
        ORDER BY m.time DESC
        "#,
    )
    .bind(team_id)
    .bind(team_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Ok(Json(matches))
}

async fn verify_volunteer_user(
    state: &crate::AppState,
    headers: &axum::http::HeaderMap,
) -> Result<VolunteerAccess, axum::http::StatusCode> {
    let claims = extract_claims(headers)?;
    let user: Option<(i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT id, role, team_id FROM users WHERE id = ? AND email = ? AND role IN (0, 1, 3) AND deleted_at IS NULL"
    ).bind(claims.user_id).bind(&claims.email).fetch_optional(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    user.map(|u| VolunteerAccess {
        user_id: u.0,
        role: u.1,
        team_id: u.2,
    })
    .ok_or(axum::http::StatusCode::FORBIDDEN)
}

async fn verify_super_user(
    state: &crate::AppState,
    headers: &axum::http::HeaderMap,
) -> Result<VolunteerAccess, axum::http::StatusCode> {
    let access = verify_volunteer_user(state, headers).await?;
    if access.role == 0 {
        Ok(access)
    } else {
        Err(axum::http::StatusCode::FORBIDDEN)
    }
}

async fn ensure_match_access(
    state: &crate::AppState,
    access: &VolunteerAccess,
    match_id: i64,
) -> Result<(), axum::http::StatusCode> {
    if access.role == 0 || access.role == 1 {
        return Ok(());
    }

    let team_id = access.team_id.ok_or(axum::http::StatusCode::FORBIDDEN)?;
    let match_info: Option<(i64, i64)> =
        sqlx::query_as("SELECT t1_id, t2_id FROM matches WHERE id = ? AND deleted_at IS NULL")
            .bind(match_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let (t1_id, t2_id) = match_info.ok_or(axum::http::StatusCode::NOT_FOUND)?;
    if team_id == t1_id || team_id == t2_id {
        Ok(())
    } else {
        Err(axum::http::StatusCode::FORBIDDEN)
    }
}

async fn ensure_reporting_round_enabled(
    db: &sqlx::SqlitePool,
    access: &VolunteerAccess,
    match_id: i64,
) -> Result<(), axum::http::StatusCode> {
    if access.role == 0 {
        return Ok(());
    }

    let round_info: Option<(i64, i64)> = sqlx::query_as(
        r#"SELECT m.type, COALESCE(rrs.is_enabled, 0)
           FROM matches m
           LEFT JOIN reporting_round_settings rrs ON rrs.round_key = m.type
           WHERE m.id = ? AND m.deleted_at IS NULL"#,
    )
    .bind(match_id)
    .fetch_optional(db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let (match_type, is_enabled) = round_info.ok_or(axum::http::StatusCode::NOT_FOUND)?;
    if is_enabled != 0 {
        Ok(())
    } else {
        tracing::info!(
            match_id,
            match_type,
            round_label = reporting_round_label(match_type),
            "reporting blocked because round is disabled"
        );
        Err(axum::http::StatusCode::FORBIDDEN)
    }
}

fn extract_email(headers: &axum::http::HeaderMap) -> Result<String, axum::http::StatusCode> {
    crate::helpers::auth::extract_email(headers)
}

fn extract_claims(
    headers: &axum::http::HeaderMap,
) -> Result<crate::controllers::user::Claims, axum::http::StatusCode> {
    crate::helpers::auth::extract_claims(headers)
}

// WFDF spirit score submission request
#[derive(Deserialize)]
pub struct WfdfSpiritRequest {
    pub team_id: i64,
    pub submitted_by_team_id: Option<i64>,
    pub rules_knowledge: i64,
    pub fouls_contact: i64,
    pub fair_mindedness: i64,
    pub positive_attitude: i64,
    pub communication: i64,
    pub mvp_player_id: Option<i64>,
    pub msp_player_id: Option<i64>,
    pub notes: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct SpiritScoreRow {
    pub id: i64,
    pub match_id: i64,
    pub team_id: i64,
    pub rules_knowledge: i64,
    pub fouls_contact: i64,
    pub fair_mindedness: i64,
    pub positive_attitude: i64,
    pub communication: i64,
    pub total: i64,
    pub mvp_player_id: Option<i64>,
    pub msp_player_id: Option<i64>,
    pub notes: Option<String>,
    pub submitted_by_team_id: i64,
}

fn normalize_spirit_notes(notes: Option<&str>) -> Result<Option<String>, axum::http::StatusCode> {
    let trimmed = notes.map(str::trim).unwrap_or("");
    if trimmed.is_empty() {
        return Ok(None);
    }

    if trimmed.split_whitespace().count() > 250 {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    Ok(Some(trimmed.to_string()))
}

#[derive(Deserialize)]
pub struct ScoreConfirmRequest {
    pub t1_score: i64,
    pub t2_score: i64,
    pub team_id: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ScoreConfirmRow {
    pub id: i64,
    pub match_id: i64,
    pub team_id: i64,
    pub t1_score: i64,
    pub t2_score: i64,
}

// Submit WFDF spirit scores for the OTHER team
pub async fn submit_wfdf_spirit(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Path(match_id): Path<i64>,
    Json(payload): Json<WfdfSpiritRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let email = extract_email(&headers)?;

    // Validate each category 0-4
    for &v in &[
        payload.rules_knowledge,
        payload.fouls_contact,
        payload.fair_mindedness,
        payload.positive_attitude,
        payload.communication,
    ] {
        if !(0..=4).contains(&v) {
            return Err(axum::http::StatusCode::BAD_REQUEST);
        }
    }

    let my_team_id =
        resolve_post_match_team(&state.db, &email, payload.submitted_by_team_id).await?;

    let match_info: Option<(i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT t1_id, t2_id, possession FROM matches WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(match_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let (t1_id, t2_id, possession) = match_info.ok_or(axum::http::StatusCode::NOT_FOUND)?;
    if possession.unwrap_or(0) < 3 {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    if my_team_id != t1_id && my_team_id != t2_id {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }

    // Validate target team_id is one of the two teams in the match
    if payload.team_id != t1_id && payload.team_id != t2_id {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    let notes = normalize_spirit_notes(payload.notes.as_deref())?;
    if payload.team_id == my_team_id && notes.is_some() {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    validate_spirit_player(&state.db, payload.mvp_player_id, payload.team_id).await?;
    validate_spirit_player(&state.db, payload.msp_player_id, payload.team_id).await?;

    let total = payload.rules_knowledge
        + payload.fouls_contact
        + payload.fair_mindedness
        + payload.positive_attitude
        + payload.communication;

    sqlx::query(
        r#"INSERT INTO spirit_scores (match_id, team_id, rules_knowledge, fouls_contact, fair_mindedness, positive_attitude, communication, total, mvp_player_id, msp_player_id, notes, submitted_by_team_id)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(match_id, team_id, submitted_by_team_id) DO UPDATE SET
             rules_knowledge=excluded.rules_knowledge, fouls_contact=excluded.fouls_contact,
             fair_mindedness=excluded.fair_mindedness, positive_attitude=excluded.positive_attitude,
             communication=excluded.communication, total=excluded.total,
             mvp_player_id=excluded.mvp_player_id, msp_player_id=excluded.msp_player_id,
             notes=excluded.notes"#
    )
    .bind(match_id).bind(payload.team_id)
    .bind(payload.rules_knowledge).bind(payload.fouls_contact).bind(payload.fair_mindedness)
    .bind(payload.positive_attitude).bind(payload.communication).bind(total)
        .bind(payload.mvp_player_id).bind(payload.msp_player_id).bind(notes).bind(my_team_id)
    .execute(&state.db).await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    // Update legacy t1_spirit/t2_spirit when rating the OTHER team (not self)
    if payload.team_id != my_team_id {
        if payload.team_id == t1_id {
            sqlx::query(
                "UPDATE matches SET t1_spirit = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(total)
            .bind(match_id)
            .execute(&state.db)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
        } else {
            sqlx::query(
                "UPDATE matches SET t2_spirit = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(total)
            .bind(match_id)
            .execute(&state.db)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    let division: Option<(i64,)> =
        sqlx::query_as("SELECT division FROM teams WHERE id = ? AND deleted_at IS NULL")
            .bind(t1_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some((division,)) = division {
        sorting::refresh_intermediate_standings_cache(&state.db, division).await;
        cache::invalidate_division(division).await;
    }

    state.live_updates.publish_match_updated(match_id);

    Ok(Json(serde_json::json!({"success": true})))
}

// Get spirit scores for a match
pub async fn get_match_spirits(
    State(state): State<crate::AppState>,
    Path(match_id): Path<i64>,
) -> Result<Json<Vec<SpiritScoreRow>>, axum::http::StatusCode> {
    let rows = sqlx::query_as::<_, SpiritScoreRow>(
        "SELECT id, match_id, team_id, rules_knowledge, fouls_contact, fair_mindedness, positive_attitude, communication, total, mvp_player_id, msp_player_id, notes, submitted_by_team_id FROM spirit_scores WHERE match_id = ?"
    ).bind(match_id).fetch_all(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(rows))
}

// Submit score confirmation from a team
pub async fn confirm_score(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Path(match_id): Path<i64>,
    Json(payload): Json<ScoreConfirmRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let email = extract_email(&headers)?;
    if payload.t1_score < 0 || payload.t2_score < 0 {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    let my_team_id = resolve_post_match_team(&state.db, &email, payload.team_id).await?;

    let match_info: Option<(i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT t1_id, t2_id, possession FROM matches WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(match_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let (t1_id, t2_id, possession) = match_info.ok_or(axum::http::StatusCode::NOT_FOUND)?;
    if possession.unwrap_or(0) < 3 {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    if my_team_id != t1_id && my_team_id != t2_id {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }
    ensure_match_not_finalized(&state.db, match_id).await?;

    sqlx::query(
        "INSERT INTO score_confirmations (match_id, team_id, t1_score, t2_score) VALUES (?, ?, ?, ?) ON CONFLICT(match_id, team_id) DO UPDATE SET t1_score=excluded.t1_score, t2_score=excluded.t2_score"
    ).bind(match_id).bind(my_team_id).bind(payload.t1_score).bind(payload.t2_score)
    .execute(&state.db).await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let finalized = match_scores_are_finalized(&state.db, match_id)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    state.live_updates.publish_match_updated(match_id);

    Ok(Json(
        serde_json::json!({"success": true, "finalized": finalized}),
    ))
}

// Get score confirmations for a match
pub async fn get_score_confirmations(
    State(state): State<crate::AppState>,
    Path(match_id): Path<i64>,
) -> Result<Json<Vec<ScoreConfirmRow>>, axum::http::StatusCode> {
    let rows = sqlx::query_as::<_, ScoreConfirmRow>(
        "SELECT id, match_id, team_id, t1_score, t2_score FROM score_confirmations WHERE match_id = ?"
    ).bind(match_id).fetch_all(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(rows))
}

// Get opponent players (for MVP/MSP dropdown)
pub async fn get_opponent_players(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Path(match_id): Path<i64>,
) -> Result<Json<Vec<MatchPlayer>>, axum::http::StatusCode> {
    let email = extract_email(&headers)?;

    let user: Option<(Option<i64>,)> =
        sqlx::query_as("SELECT team_id FROM users WHERE email = ? AND deleted_at IS NULL")
            .bind(&email)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let my_team_id = user
        .ok_or(axum::http::StatusCode::FORBIDDEN)?
        .0
        .ok_or(axum::http::StatusCode::FORBIDDEN)?;

    let match_info: Option<(i64, i64)> =
        sqlx::query_as("SELECT t1_id, t2_id FROM matches WHERE id = ? AND deleted_at IS NULL")
            .bind(match_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let (t1_id, t2_id) = match_info.ok_or(axum::http::StatusCode::NOT_FOUND)?;
    let opponent_team_id = if my_team_id == t1_id {
        t2_id
    } else if my_team_id == t2_id {
        t1_id
    } else {
        return Err(axum::http::StatusCode::FORBIDDEN);
    };

    let players = sqlx::query_as::<_, MatchPlayer>(
        "SELECT id, name, common_name, team_id FROM users WHERE team_id = ? AND role = 2 AND deleted_at IS NULL ORDER BY name ASC"
    ).bind(opponent_team_id).fetch_all(&state.db).await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(players))
}

async fn resolve_post_match_team(
    db: &sqlx::SqlitePool,
    email: &str,
    requested_team_id: Option<i64>,
) -> Result<i64, axum::http::StatusCode> {
    let user: Option<(i64, Option<i64>)> =
        sqlx::query_as("SELECT role, team_id FROM users WHERE email = ? AND deleted_at IS NULL")
            .bind(email)
            .fetch_optional(db)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let (role, team_id) = user.ok_or(axum::http::StatusCode::FORBIDDEN)?;

    match role {
        0 | 1 => requested_team_id
            .or(team_id)
            .ok_or(axum::http::StatusCode::BAD_REQUEST),
        3 => {
            let team_id = team_id.ok_or(axum::http::StatusCode::FORBIDDEN)?;
            if let Some(requested_team_id) = requested_team_id
                && requested_team_id != team_id
            {
                return Err(axum::http::StatusCode::FORBIDDEN);
            }
            Ok(team_id)
        }
        _ => Err(axum::http::StatusCode::FORBIDDEN),
    }
}

async fn validate_spirit_player(
    db: &sqlx::SqlitePool,
    player_id: Option<i64>,
    expected_team_id: i64,
) -> Result<(), axum::http::StatusCode> {
    let Some(player_id) = player_id else {
        return Ok(());
    };

    let player_team: Option<(i64,)> = sqlx::query_as(
        "SELECT team_id FROM users WHERE id = ? AND role = 2 AND deleted_at IS NULL",
    )
    .bind(player_id)
    .fetch_optional(db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    if player_team.map(|row| row.0) != Some(expected_team_id) {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    Ok(())
}

async fn ensure_match_not_finalized(
    db: &sqlx::SqlitePool,
    match_id: i64,
) -> Result<(), axum::http::StatusCode> {
    if match_scores_are_finalized(db, match_id)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(axum::http::StatusCode::CONFLICT);
    }

    Ok(())
}

async fn match_scores_are_finalized(
    db: &sqlx::SqlitePool,
    match_id: i64,
) -> Result<bool, sqlx::Error> {
    let match_row: Option<(i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT t1_score, t2_score, possession FROM matches WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(match_id)
    .fetch_optional(db)
    .await?;

    let Some((t1_score, t2_score, possession)) = match_row else {
        return Ok(false);
    };

    if possession.unwrap_or(0) < 3 {
        return Ok(false);
    }

    let confirmed: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT team_id) FROM score_confirmations WHERE match_id = ? AND t1_score = ? AND t2_score = ?",
    )
    .bind(match_id)
    .bind(t1_score)
    .bind(t2_score)
    .fetch_one(db)
    .await?;

    Ok(confirmed.0 >= 2)
}
