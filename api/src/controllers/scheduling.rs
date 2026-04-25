use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::helpers::{cache, rounds, sorting};

const FIELD_COUNT: usize = 4;
const SWISS_MATCH_MINUTES: i64 = 60;
const SWISS_BREAK_MINUTES: i64 = 15;
const PLAYOFF_MATCH_MINUTES: i64 = 75;
const PLAYOFF_BREAK_MINUTES: i64 = 15;
const ROW_OVERRIDE_CACHE_KEY: &str = "schedule:row_overrides";
const ROW_OVERRIDE_TTL_SECONDS: u64 = 60 * 60 * 24 * 30;

#[derive(Serialize)]
pub struct TournamentState {
    pub division: i64,
    pub phase: String,
    pub current_round: i64,
    pub total_rounds: i64,
    pub round_status: Vec<RoundStatus>,
    pub standings: Vec<TeamStanding>,
}

#[derive(Serialize)]
pub struct RoundStatus {
    pub round: i64,
    pub total: i64,
    pub completed: i64,
    pub in_progress: i64,
    pub scheduled: i64,
}

#[derive(Serialize, Clone)]
pub struct TeamStanding {
    pub team_id: i64,
    pub name: String,
    pub wins: i64,
    pub losses: i64,
    pub points_for: i64,
    pub points_against: i64,
    pub h2h_diff: i64,
    pub spirit_avg: f64,
    pub small_logo: Option<String>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct ScheduleMatch {
    pub id: i64,
    pub t1_id: i64,
    pub t2_id: i64,
    pub t1_name: String,
    pub t2_name: String,
    pub t1_score: i64,
    pub t2_score: i64,
    pub t1_spirit: Option<i64>,
    pub t2_spirit: Option<i64>,
    pub t1_small_logo: Option<String>,
    pub t2_small_logo: Option<String>,
    pub field_name: String,
    pub time: String,
    pub possession: Option<i64>,
    pub stream_url: Option<String>,
    pub match_type: i64,
}

#[derive(Serialize, Deserialize)]
pub struct ScheduleGridResponse {
    pub rows: Vec<ScheduleGridRow>,
}

#[derive(Serialize, Deserialize)]
pub struct ScheduleTeamsResponse {
    pub teams: Vec<ScheduleGridTeam>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct ScheduleGridTeam {
    pub id: i64,
    pub name: String,
    pub abbreviation: Option<String>,
    pub division: i64,
    pub small_logo: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ScheduleGridRow {
    pub key: String,
    pub day_key: String,
    pub day_label: String,
    pub label: String,
    pub start_time: String,
    pub end_time: String,
    pub cells: Vec<ScheduleGridCell>,
}

#[derive(Serialize, Deserialize)]
pub struct ScheduleGridCell {
    pub field_index: i64,
    pub field_label: String,
    pub field_name: String,
    pub slot_code: Option<String>,
    pub division: Option<i64>,
    pub match_type: Option<i64>,
    pub match_id: Option<i64>,
    pub data: Option<[i64; 5]>,
    pub seed_ranks: Option<[i64; 2]>,
    pub stream_url: Option<String>,
    pub possession: Option<i64>,
    pub status: String,
    pub clickable: bool,
    pub movable: bool,
}

#[derive(Deserialize)]
pub struct ScheduleQuery {
    pub division: Option<i64>,
    pub round: Option<i64>,
    pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct DivisionQuery {
    pub division: i64,
}

#[derive(Deserialize)]
pub struct UpdateScheduleRowRequest {
    pub start_time: String,
    pub end_time: String,
}

#[derive(Deserialize)]
pub struct MoveScheduleMatchRequest {
    pub row_key: String,
    pub field_index: i64,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct RowTimeOverride {
    start_time: String,
    end_time: String,
}

#[derive(Clone)]
struct SlotTemplate {
    field_index: usize,
    division: i64,
    slot_code: String,
}

#[derive(Clone)]
struct RowTemplate {
    key: String,
    day_key: &'static str,
    day_label: &'static str,
    label: String,
    match_type: i64,
    date: NaiveDate,
    day_start: NaiveTime,
    day_end: NaiveTime,
    duration_minutes: i64,
    start_at: NaiveDateTime,
    end_at: NaiveDateTime,
    slots: Vec<SlotTemplate>,
}

#[derive(Clone, sqlx::FromRow)]
struct ScheduleGridMatchRecord {
    id: i64,
    t1_id: i64,
    t2_id: i64,
    division: i64,
    t1_score: i64,
    t2_score: i64,
    field_id: Option<i64>,
    time: String,
    possession: Option<i64>,
    stream_url: Option<String>,
    match_type: i64,
}

#[derive(Clone)]
struct FieldSlot {
    id: i64,
    label: String,
    name: String,
}

type MoveMatchRecord = (Option<String>, Option<i64>, Option<i64>, i64, i64);

pub async fn auto_advance_division_if_ready(
    db: &sqlx::SqlitePool,
    division: i64,
) -> Option<String> {
    let total_rounds = if division == 0 {
        crate::OPEN_ROUNDS
    } else {
        crate::WOMEN_ROUNDS
    };

    for round in 1..=total_rounds {
        let stats: (i64, i64) = sqlx::query_as(
            r#"SELECT COUNT(*), COALESCE(SUM(CASE WHEN possession >= 3 THEN 1 ELSE 0 END), 0)
               FROM matches m JOIN teams t ON m.t1_id = t.id
               WHERE m.type = ? AND t.division = ? AND m.deleted_at IS NULL"#,
        )
        .bind(round)
        .bind(division)
        .fetch_one(db)
        .await
        .unwrap_or((0, 0));

        let (total, completed) = stats;

        if total == 0
            && (round == 1
                || {
                    let prev: (i64, i64) = sqlx::query_as(
                    r#"SELECT COUNT(*), COALESCE(SUM(CASE WHEN possession >= 3 THEN 1 ELSE 0 END), 0)
                       FROM matches m JOIN teams t ON m.t1_id = t.id
                       WHERE m.type = ? AND t.division = ? AND m.deleted_at IS NULL"#,
                )
                .bind(round - 1)
                .bind(division)
                .fetch_one(db)
                .await
                .unwrap_or((0, 0));
                    prev.0 > 0 && prev.0 == prev.1
                })
        {
            if generate_next_round_internal(db, division, round)
                .await
                .is_ok()
            {
                return Some(format!("generated_round_{round}"));
            }
            return None;
        }

        if total == 0 {
            return None;
        }

        if completed < total {
            return None;
        }
    }

    if create_playoffs_if_needed(db, division)
        .await
        .unwrap_or(false)
    {
        return Some("generated_playoff_1".to_string());
    }

    if create_finals_if_needed(db, division).await.unwrap_or(false) {
        return Some("generated_playoff_2".to_string());
    }

    None
}

pub async fn auto_generate_initial_rounds(db: &sqlx::SqlitePool) {
    for division in 0..=1 {
        let match_count: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM matches m
               JOIN teams t ON m.t1_id = t.id
               WHERE t.division = ? AND m.deleted_at IS NULL"#,
        )
        .bind(division)
        .fetch_one(db)
        .await
        .unwrap_or((0,));

        if match_count.0 == 0
            && let Err(err) = generate_r1_from_seeding(db, division).await
        {
            tracing::error!("Failed to generate R1 for division {}: {}", division, err);
        }
    }
}

async fn generate_r1_from_seeding(db: &sqlx::SqlitePool, division: i64) -> Result<(), sqlx::Error> {
    let teams: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, init_rank FROM teams WHERE division = ? AND deleted_at IS NULL ORDER BY init_rank ASC",
    )
    .bind(division)
    .fetch_all(db)
    .await?;

    if teams.is_empty() {
        return Ok(());
    }

    let half = teams.len() / 2;
    let mut pairings = Vec::with_capacity(half);
    for index in 0..half {
        pairings.push(rounds::Pairing {
            t1: teams[index].0,
            t2: teams[index + half].0,
        });
    }

    insert_pairings_into_slots(db, division, 1, &pairings).await
}

pub async fn read_tournament_state(
    State(state): State<crate::AppState>,
    Query(params): Query<DivisionQuery>,
) -> Json<TournamentState> {
    let division = params.division;
    let total_rounds = if division == 0 {
        crate::OPEN_ROUNDS
    } else {
        crate::WOMEN_ROUNDS
    };

    let mut round_status = Vec::new();
    for round in 1..=total_rounds {
        let stats: (i64, i64, i64, i64) = sqlx::query_as(
            r#"SELECT
                COUNT(*) as total,
                COALESCE(SUM(CASE WHEN possession >= 3 THEN 1 ELSE 0 END), 0) as completed,
                COALESCE(SUM(CASE WHEN possession IS NOT NULL AND possession < 3 THEN 1 ELSE 0 END), 0) as in_progress,
                COALESCE(SUM(CASE WHEN possession IS NULL THEN 1 ELSE 0 END), 0) as scheduled
            FROM matches m
            JOIN teams t1 ON m.t1_id = t1.id
            WHERE m.type = ? AND t1.division = ? AND m.deleted_at IS NULL"#,
        )
        .bind(round)
        .bind(division)
        .fetch_one(&state.db)
        .await
        .unwrap_or((0, 0, 0, 0));

        round_status.push(RoundStatus {
            round,
            total: stats.0,
            completed: stats.1,
            in_progress: stats.2,
            scheduled: stats.3,
        });
    }

    let current_round = round_status
        .iter()
        .find(|round| round.completed < round.total || round.total == 0)
        .map(|round| round.round)
        .unwrap_or(total_rounds + 1);

    let playoff_one: (i64, i64) = sqlx::query_as(
        r#"SELECT COUNT(*), COALESCE(SUM(CASE WHEN m.possession >= 3 THEN 1 ELSE 0 END), 0)
           FROM matches m JOIN teams t ON m.t1_id = t.id
           WHERE m.type = 1001 AND t.division = ? AND m.deleted_at IS NULL"#,
    )
    .bind(division)
    .fetch_one(&state.db)
    .await
    .unwrap_or((0, 0));

    let playoff_two: (i64, i64) = sqlx::query_as(
        r#"SELECT COUNT(*), COALESCE(SUM(CASE WHEN m.possession >= 3 THEN 1 ELSE 0 END), 0)
           FROM matches m JOIN teams t ON m.t1_id = t.id
           WHERE m.type = 1002 AND t.division = ? AND m.deleted_at IS NULL"#,
    )
    .bind(division)
    .fetch_one(&state.db)
    .await
    .unwrap_or((0, 0));

    let phase = if current_round <= total_rounds {
        format!("swiss_R{current_round}")
    } else if playoff_two.0 > 0 {
        if playoff_two.1 < playoff_two.0 {
            "playoff_2".to_string()
        } else {
            "complete".to_string()
        }
    } else if playoff_one.0 > 0 {
        if playoff_one.1 < playoff_one.0 {
            "playoff_1".to_string()
        } else {
            "playoff_2_pending".to_string()
        }
    } else {
        "playoff_1_pending".to_string()
    };

    let standings = compute_intermediate_standings(&state.db, division).await;

    Json(TournamentState {
        division,
        phase,
        current_round,
        total_rounds,
        round_status,
        standings,
    })
}

async fn compute_standings(db: &sqlx::SqlitePool, division: i64) -> Vec<TeamStanding> {
    let sorted = sorting::get_sorted_standings(db, division).await;
    sorted
        .iter()
        .map(|team| TeamStanding {
            team_id: team.team_id,
            name: team.name.clone(),
            wins: team.wins,
            losses: team.losses,
            points_for: team.points_for,
            points_against: team.points_against,
            h2h_diff: 0,
            spirit_avg: team.spirit_avg,
            small_logo: team.small_logo.clone(),
        })
        .collect()
}

async fn compute_intermediate_standings(db: &sqlx::SqlitePool, division: i64) -> Vec<TeamStanding> {
    let sorted = sorting::get_cached_intermediate_standings(db, division).await;
    sorted
        .iter()
        .map(|team| TeamStanding {
            team_id: team.team_id,
            name: team.name.clone(),
            wins: team.wins,
            losses: team.losses,
            points_for: team.points_for,
            points_against: team.points_against,
            h2h_diff: 0,
            spirit_avg: team.spirit_avg,
            small_logo: team.small_logo.clone(),
        })
        .collect()
}

pub async fn get_schedule_matches(
    State(state): State<crate::AppState>,
    Query(params): Query<ScheduleQuery>,
) -> Json<Vec<ScheduleMatch>> {
    let division = params.division.unwrap_or(0);
    let status_key = params
        .status
        .as_deref()
        .unwrap_or("all")
        .to_ascii_lowercase();
    let should_cache = status_key == "upcoming" || status_key == "done";

    if should_cache
        && let Some(cached) = cache::get_schedule::<Vec<ScheduleMatch>>(
            division,
            params.round,
            Some(status_key.as_str()),
        )
        .await
    {
        return Json(cached);
    }

    let mut query = String::from(
        r#"SELECT m.id, m.t1_id, m.t2_id, t1.name as t1_name, t2.name as t2_name,
               m.t1_score, m.t2_score, m.t1_spirit, m.t2_spirit,
               t1.small_logo as t1_small_logo, t2.small_logo as t2_small_logo,
               COALESCE(f.name, '') as field_name, COALESCE(m.time, '') as time,
               m.possession, m.stream_url, m.type as match_type
        FROM matches m
        JOIN teams t1 ON t1.id = m.t1_id
        JOIN teams t2 ON t2.id = m.t2_id
        LEFT JOIN fields f ON f.id = m.field_id
        WHERE m.deleted_at IS NULL AND t1.division = ?"#,
    );

    if let Some(round) = params.round {
        query.push_str(&format!(" AND m.type = {round}"));
    }

    if let Some(ref status) = params.status {
        match status.as_str() {
            "live" => query.push_str(" AND m.possession IS NOT NULL AND m.possession < 3"),
            "upcoming" => query.push_str(" AND m.possession IS NULL"),
            "done" => query.push_str(" AND m.possession >= 3"),
            _ => {}
        }
    }

    query.push_str(
        r#" ORDER BY m.time ASC,
           CASE
               WHEN m.possession IS NOT NULL AND m.possession < 3 THEN 1
               WHEN m.possession IS NULL THEN 2
               ELSE 3
           END,
           m.field_id ASC"#,
    );

    let matches = sqlx::query_as::<_, ScheduleMatch>(&query)
        .bind(division)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    if should_cache {
        cache::set_schedule(division, params.round, Some(status_key.as_str()), &matches).await;
    }

    Json(matches)
}

pub async fn get_schedule_grid(State(state): State<crate::AppState>) -> Json<ScheduleGridResponse> {
    if let Some(cached) = cache::get_schedule_grid_cache::<ScheduleGridResponse>().await {
        return Json(cached);
    }
    let overrides = load_row_overrides().await;
    let rows = build_schedule_rows(&overrides);
    let fields = fetch_field_slots(&state.db).await;
    let grid_matches = fetch_grid_matches(&state.db).await;
    let rank_snapshots = build_schedule_rank_snapshots(&state.db, &rows).await;
    let response = ScheduleGridResponse {
        rows: materialize_grid(rows, fields, grid_matches, &rank_snapshots),
    };
    cache::set_schedule_grid_cache(&response).await;
    Json(response)
}

pub async fn get_schedule_teams(
    State(state): State<crate::AppState>,
) -> Json<ScheduleTeamsResponse> {
    if let Some(cached) = cache::get_schedule_teams_cache::<ScheduleTeamsResponse>().await {
        return Json(cached);
    }
    let teams = sqlx::query_as::<_, ScheduleGridTeam>(
        r#"SELECT id, name, abbreviation, division, small_logo
           FROM teams
           WHERE deleted_at IS NULL
           ORDER BY division ASC, init_rank ASC, id ASC"#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    let response = ScheduleTeamsResponse { teams };
    cache::set_schedule_teams_cache(&response).await;
    Json(response)
}

pub async fn update_schedule_row(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(row_key): Path<String>,
    Json(payload): Json<UpdateScheduleRowRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    verify_super(&state, &headers).await?;

    let fields = fetch_field_slots(&state.db).await;
    if fields.len() < FIELD_COUNT {
        return Err(axum::http::StatusCode::CONFLICT);
    }

    let mut overrides = load_row_overrides().await;
    let rows = build_schedule_rows(&overrides);
    let row = rows
        .iter()
        .find(|candidate| candidate.key == row_key)
        .cloned()
        .ok_or(axum::http::StatusCode::NOT_FOUND)?;

    let new_start = parse_clock(&payload.start_time).ok_or(axum::http::StatusCode::BAD_REQUEST)?;
    let new_end = parse_clock(&payload.end_time).ok_or(axum::http::StatusCode::BAD_REQUEST)?;

    if minutes_between(new_start, new_end) != row.duration_minutes {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    let new_start_at = NaiveDateTime::new(row.date, new_start);
    let new_end_at = NaiveDateTime::new(row.date, new_end);

    if new_start < row.day_start || new_end > row.day_end {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    for sibling in rows
        .iter()
        .filter(|candidate| candidate.day_key == row.day_key && candidate.key != row.key)
    {
        if new_start_at < sibling.end_at && new_end_at > sibling.start_at {
            return Err(axum::http::StatusCode::CONFLICT);
        }
    }

    overrides.insert(
        row_key.clone(),
        RowTimeOverride {
            start_time: payload.start_time.clone(),
            end_time: payload.end_time.clone(),
        },
    );
    save_row_overrides(&overrides).await;

    let old_time = row.start_at.format("%Y-%m-%d %H:%M:%S").to_string();
    let new_time = new_start_at.format("%Y-%m-%d %H:%M:%S").to_string();

    let field_ids: Vec<i64> = fields.iter().map(|field| field.id).collect();
    let placeholders = vec!["?"; field_ids.len()].join(",");
    let query = format!(
        "UPDATE matches SET time = ?, updated_at = CURRENT_TIMESTAMP WHERE deleted_at IS NULL AND type = ? AND time = ? AND field_id IN ({placeholders})"
    );
    let mut update_query = sqlx::query(&query)
        .bind(&new_time)
        .bind(row.match_type)
        .bind(&old_time);
    for field_id in field_ids {
        update_query = update_query.bind(field_id);
    }
    update_query
        .execute(&state.db)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    cache::invalidate_all().await;

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn move_schedule_match(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(match_id): Path<i64>,
    Json(payload): Json<MoveScheduleMatchRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    verify_super(&state, &headers).await?;

    let rows = build_schedule_rows(&load_row_overrides().await);
    let target_row = rows
        .iter()
        .find(|row| row.key == payload.row_key)
        .cloned()
        .ok_or(axum::http::StatusCode::NOT_FOUND)?;

    let fields = fetch_field_slots(&state.db).await;
    let field_index =
        usize::try_from(payload.field_index).map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
    let target_field = fields
        .iter()
        .find(|field| field.label == format!("G{field_index}"))
        .cloned()
        .ok_or(axum::http::StatusCode::BAD_REQUEST)?;

    let moving_match: Option<MoveMatchRecord> = sqlx::query_as(
        r#"SELECT m.time, m.field_id, m.possession, m.type, t.division
           FROM matches m
           JOIN teams t ON t.id = m.t1_id
           WHERE m.id = ? AND m.deleted_at IS NULL"#,
    )
    .bind(match_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let (source_time, source_field_id, source_possession, match_type, division) =
        moving_match.ok_or(axum::http::StatusCode::NOT_FOUND)?;

    if source_possession.is_some() {
        return Err(axum::http::StatusCode::CONFLICT);
    }

    let target_slot = target_row
        .slots
        .iter()
        .find(|slot| slot.field_index == field_index && slot.division == division)
        .ok_or(axum::http::StatusCode::BAD_REQUEST)?;

    if target_row.match_type != match_type || target_slot.division != division {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    let target_time = target_row.start_at.format("%Y-%m-%d %H:%M:%S").to_string();
    let source_time = source_time.ok_or(axum::http::StatusCode::CONFLICT)?;
    let source_field_id = source_field_id.ok_or(axum::http::StatusCode::CONFLICT)?;

    if source_time == target_time && source_field_id == target_field.id {
        return Ok(Json(serde_json::json!({ "success": true })));
    }

    let occupant: Option<(i64, Option<i64>, i64, i64)> = sqlx::query_as(
        r#"SELECT m.id, m.possession, m.type, t.division
           FROM matches m
           JOIN teams t ON t.id = m.t1_id
           WHERE m.deleted_at IS NULL AND m.time = ? AND m.field_id = ? AND m.id != ?"#,
    )
    .bind(&target_time)
    .bind(target_field.id)
    .bind(match_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some((occupant_id, occupant_possession, occupant_type, occupant_division)) = occupant {
        if occupant_possession.is_some()
            || occupant_type != match_type
            || occupant_division != division
        {
            return Err(axum::http::StatusCode::CONFLICT);
        }

        sqlx::query(
            "UPDATE matches SET time = ?, field_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(&source_time)
        .bind(source_field_id)
        .bind(occupant_id)
        .execute(&state.db)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    sqlx::query(
        "UPDATE matches SET time = ?, field_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(&target_time)
    .bind(target_field.id)
    .bind(match_id)
    .execute(&state.db)
    .await
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    cache::invalidate_all().await;

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn generate_next_round(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(division): Path<i64>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    verify_admin(&state, &headers).await?;

    let total_rounds = if division == 0 {
        crate::OPEN_ROUNDS
    } else {
        crate::WOMEN_ROUNDS
    };

    let next_round: i64 = sqlx::query_as::<_, (i64,)>(
        r#"SELECT COALESCE(MAX(m.type), 0) + 1
           FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE t.division = ? AND m.type < 1000 AND m.deleted_at IS NULL"#,
    )
    .bind(division)
    .fetch_one(&state.db)
    .await
    .map(|row| row.0)
    .unwrap_or(1);

    if next_round > total_rounds {
        return Ok(Json(serde_json::json!({
            "success": false,
            "message": "All swiss rounds complete"
        })));
    }

    if next_round > 1 {
        let prev_incomplete: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM matches m
               JOIN teams t ON m.t1_id = t.id
               WHERE m.type = ? AND t.division = ? AND (m.possession IS NULL OR m.possession < 3) AND m.deleted_at IS NULL"#,
        )
        .bind(next_round - 1)
        .bind(division)
        .fetch_one(&state.db)
        .await
        .unwrap_or((1,));

        if prev_incomplete.0 > 0 {
            return Ok(Json(serde_json::json!({
                "success": false,
                "message": "Previous round incomplete"
            })));
        }
    }

    let existing: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE m.type = ? AND t.division = ? AND m.deleted_at IS NULL"#,
    )
    .bind(next_round)
    .bind(division)
    .fetch_one(&state.db)
    .await
    .unwrap_or((0,));

    if existing.0 > 0 {
        return Ok(Json(serde_json::json!({
            "success": false,
            "message": "Round already generated"
        })));
    }

    let history = rounds::fetch_match_history(&state.db, division).await;
    let sorted = sorting::get_sorted_standings(&state.db, division).await;
    if sorted.len() % 2 != 0 {
        return Ok(Json(serde_json::json!({
            "success": false,
            "message": "Odd number of teams; cannot generate pairings"
        })));
    }

    let pairings = rounds::generate_round_pairings(&sorted, &history);
    if pairings.len() * 2 != sorted.len() {
        return Ok(Json(serde_json::json!({
            "success": false,
            "message": "Could not generate complete non-overlapping pairings"
        })));
    }

    insert_pairings_into_slots(&state.db, division, next_round, &pairings)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    cache::invalidate_all().await;

    Ok(Json(serde_json::json!({
        "success": true,
        "round": next_round,
        "matches_created": pairings.len()
    })))
}

pub async fn check_and_populate_gates(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Path(division): Path<i64>,
) -> Json<serde_json::Value> {
    if verify_admin(&state, &headers).await.is_err() {
        return Json(serde_json::json!({
            "action": "forbidden",
            "next_gate": "unauthorized"
        }));
    }

    let total_rounds = if division == 0 {
        crate::OPEN_ROUNDS
    } else {
        crate::WOMEN_ROUNDS
    };

    for round in 1..=total_rounds {
        let stats: (i64, i64) = sqlx::query_as(
            r#"SELECT COUNT(*), COALESCE(SUM(CASE WHEN possession >= 3 THEN 1 ELSE 0 END), 0)
               FROM matches m JOIN teams t ON m.t1_id = t.id
               WHERE m.type = ? AND t.division = ? AND m.deleted_at IS NULL"#,
        )
        .bind(round)
        .bind(division)
        .fetch_one(&state.db)
        .await
        .unwrap_or((0, 0));

        let (total, completed) = stats;
        if total == 0
            && (round == 1
                || {
                    let prev: (i64, i64) = sqlx::query_as(
                    r#"SELECT COUNT(*), COALESCE(SUM(CASE WHEN possession >= 3 THEN 1 ELSE 0 END), 0)
                       FROM matches m JOIN teams t ON m.t1_id = t.id
                       WHERE m.type = ? AND t.division = ? AND m.deleted_at IS NULL"#,
                )
                .bind(round - 1)
                .bind(division)
                .fetch_one(&state.db)
                .await
                .unwrap_or((0, 0));
                    prev.0 > 0 && prev.0 == prev.1
                })
        {
            let _ = generate_next_round_internal(&state.db, division, round).await;
            cache::invalidate_all().await;
            return Json(serde_json::json!({
                "action": format!("generated_round_{round}"),
                "next_gate": if round < total_rounds {
                    format!("round_{}_completion", round)
                } else {
                    "playoff_1_generation".to_string()
                }
            }));
        }

        if total > 0 && completed < total {
            return Json(serde_json::json!({
                "action": "none",
                "next_gate": format!("round_{}_completion", round)
            }));
        }
    }

    Json(serde_json::json!({
        "action": "none",
        "next_gate": "playoff_1_or_complete"
    }))
}

async fn generate_next_round_internal(
    db: &sqlx::SqlitePool,
    division: i64,
    round: i64,
) -> Result<(), sqlx::Error> {
    let sorted = sorting::get_sorted_standings(db, division).await;
    if sorted.len() % 2 != 0 {
        return Err(sqlx::Error::Protocol(
            "odd number of teams; cannot generate pairings".into(),
        ));
    }

    let history = rounds::fetch_match_history(db, division).await;
    let pairings = rounds::generate_round_pairings(&sorted, &history);
    if pairings.len() * 2 != sorted.len() {
        return Err(sqlx::Error::Protocol(
            "incomplete pairings generated".into(),
        ));
    }

    insert_pairings_into_slots(db, division, round, &pairings).await
}

async fn verify_admin(
    state: &crate::AppState,
    headers: &HeaderMap,
) -> Result<(), axum::http::StatusCode> {
    let email = extract_email(headers)?;

    let user: Option<(i64,)> =
        sqlx::query_as("SELECT role FROM users WHERE email = ? AND deleted_at IS NULL")
            .bind(&email)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    match user.map(|row| row.0) {
        Some(0 | 1) => Ok(()),
        Some(_) => Err(axum::http::StatusCode::FORBIDDEN),
        None => Err(axum::http::StatusCode::UNAUTHORIZED),
    }
}

pub async fn get_early_fixtures(
    State(state): State<crate::AppState>,
    Query(params): Query<DivisionQuery>,
) -> Json<serde_json::Value> {
    let division = params.division;
    let total_rounds = if division == 0 {
        crate::OPEN_ROUNDS
    } else {
        crate::WOMEN_ROUNDS
    };

    let current_round: i64 = sqlx::query_as::<_, (i64,)>(
        r#"SELECT COALESCE(MIN(m.type), 1)
           FROM matches m JOIN teams t ON m.t1_id = t.id
           WHERE t.division = ? AND m.type < 1000 AND m.possession < 3 AND m.deleted_at IS NULL"#,
    )
    .bind(division)
    .fetch_one(&state.db)
    .await
    .map(|row| row.0)
    .unwrap_or(1);

    if current_round > total_rounds {
        return Json(serde_json::json!({
            "early_pairings": [],
            "teams_waiting": [],
            "message": "All swiss rounds complete"
        }));
    }

    let standings = compute_standings(&state.db, division).await;
    let mut early_pairings = Vec::new();
    let mut teams_waiting = Vec::new();

    if standings.len() >= 2 {
        let top = &standings[0];
        let second = &standings[1];
        if top.wins == second.wins && top.wins > 0 {
            early_pairings.push(serde_json::json!({
                "team1_id": top.team_id,
                "team1_name": top.name,
                "team2_id": second.team_id,
                "team2_name": second.name,
                "round": current_round + 1,
                "confidence": "likely"
            }));
        }
    }

    for team in standings
        .iter()
        .skip(2)
        .take(standings.len().saturating_sub(4))
    {
        teams_waiting.push(team.team_id);
    }

    Json(serde_json::json!({
        "early_pairings": early_pairings,
        "teams_waiting": teams_waiting
    }))
}

async fn create_playoffs_if_needed(
    db: &sqlx::SqlitePool,
    division: i64,
) -> Result<bool, sqlx::Error> {
    let existing_playoffs: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM matches m JOIN teams t ON m.t1_id = t.id WHERE t.division = ? AND m.type = 1001 AND m.deleted_at IS NULL",
    )
    .bind(division)
    .fetch_one(db)
    .await?;

    if existing_playoffs.0 > 0 {
        return Ok(false);
    }

    let sorted = sorting::get_sorted_standings(db, division).await;
    if sorted.len() < 2 {
        return Ok(false);
    }

    let bracket_rounds = rounds::build_playoff_brackets(&sorted);
    let Some(playoff_round) = bracket_rounds.iter().find(|round| round.name == "playoffs") else {
        return Ok(false);
    };

    insert_pairings_into_slots(db, division, 1001, &playoff_round.matches).await?;
    Ok(true)
}

async fn fetch_completed_playoff_results(
    db: &sqlx::SqlitePool,
    division: i64,
) -> Result<Vec<rounds::PlayedMatchResult>, sqlx::Error> {
    let rows: Vec<(i64, i64, i64, i64)> = sqlx::query_as(
        r#"SELECT m.t1_id, m.t2_id, m.t1_score, m.t2_score
           FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE t.division = ? AND m.type = 1001 AND m.possession >= 3 AND m.deleted_at IS NULL
           ORDER BY m.time ASC, m.field_id ASC, m.id ASC"#,
    )
    .bind(division)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|(t1_id, t2_id, t1_score, t2_score)| {
            if t1_score == t2_score {
                return None;
            }

            Some(rounds::PlayedMatchResult {
                t1: t1_id,
                t2: t2_id,
                winner: if t1_score > t2_score { t1_id } else { t2_id },
            })
        })
        .collect())
}

async fn create_finals_if_needed(
    db: &sqlx::SqlitePool,
    division: i64,
) -> Result<bool, sqlx::Error> {
    let finals_existing: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM matches m JOIN teams t ON m.t1_id = t.id WHERE t.division = ? AND m.type = 1002 AND m.deleted_at IS NULL",
    )
    .bind(division)
    .fetch_one(db)
    .await?;

    if finals_existing.0 > 0 {
        return Ok(false);
    }

    let playoff_stats: (i64, i64) = sqlx::query_as(
        r#"SELECT COUNT(*), COALESCE(SUM(CASE WHEN m.possession >= 3 THEN 1 ELSE 0 END), 0)
           FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE t.division = ? AND m.type = 1001 AND m.deleted_at IS NULL"#,
    )
    .bind(division)
    .fetch_one(db)
    .await?;

    if playoff_stats.0 == 0 || playoff_stats.1 < playoff_stats.0 {
        return Ok(false);
    }

    let sorted = sorting::get_sorted_standings(db, division).await;
    let playoff_results = fetch_completed_playoff_results(db, division).await?;
    let final_pairings =
        rounds::build_final_pairings_from_playoff_results(&sorted, &playoff_results);
    if final_pairings.is_empty() {
        return Ok(false);
    }

    insert_pairings_into_slots(db, division, 1002, &final_pairings).await?;
    Ok(true)
}

async fn insert_pairings_into_slots(
    db: &sqlx::SqlitePool,
    division: i64,
    match_type: i64,
    pairings: &[rounds::Pairing],
) -> Result<(), sqlx::Error> {
    let slots = get_slot_assignments(db, division, match_type).await?;
    if pairings.len() > slots.len() {
        return Err(sqlx::Error::Protocol(format!(
            "not enough fixed schedule slots for division {division} type {match_type}: {} pairings for {} slots",
            pairings.len(),
            slots.len()
        )));
    }

    for (pairing, (field_id, start_time)) in pairings.iter().zip(slots.iter()) {
        sqlx::query(
            "INSERT INTO matches (t1_id, t2_id, field_id, time, type) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(pairing.t1)
        .bind(pairing.t2)
        .bind(*field_id)
        .bind(start_time)
        .bind(match_type)
        .execute(db)
        .await?;
    }

    cache::invalidate_division(division).await;
    Ok(())
}

async fn get_slot_assignments(
    db: &sqlx::SqlitePool,
    division: i64,
    match_type: i64,
) -> Result<Vec<(i64, String)>, sqlx::Error> {
    let rows = build_schedule_rows(&load_row_overrides().await);
    let fields = fetch_field_slots(db).await;
    if fields.len() < FIELD_COUNT {
        return Err(sqlx::Error::Protocol(
            "expected four fields for fixed schedule".into(),
        ));
    }

    let mut slots = Vec::new();
    for row in rows.iter().filter(|row| row.match_type == match_type) {
        for slot in row.slots.iter().filter(|slot| slot.division == division) {
            let field = fields
                .iter()
                .find(|field| field.label == format!("G{}", slot.field_index))
                .ok_or_else(|| sqlx::Error::Protocol("missing field mapping for slot".into()))?;
            slots.push((
                field.id,
                row.start_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            ));
        }
    }
    Ok(slots)
}

fn build_schedule_rows(overrides: &HashMap<String, RowTimeOverride>) -> Vec<RowTemplate> {
    let friday = NaiveDate::from_ymd_opt(2026, 1, 30).unwrap();
    let saturday = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
    let sunday = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();

    let mut rows = Vec::new();
    rows.extend(build_swiss_rows("fri", "Friday", friday, 1));
    rows.extend(build_swiss_rows("fri", "Friday", friday, 2));
    rows.extend(build_swiss_rows("fri", "Friday", friday, 3));
    rows.extend(build_swiss_rows("sat", "Saturday", saturday, 4));
    rows.extend(build_swiss_rows("sat", "Saturday", saturday, 5));
    rows.extend(build_swiss_rows("sat", "Saturday", saturday, 6));
    rows.extend(build_playoff_rows("sun", "Sunday", sunday));

    for row in &mut rows {
        if let Some(override_value) = overrides.get(&row.key) {
            let Some(start_time) = parse_clock(&override_value.start_time) else {
                continue;
            };
            let Some(end_time) = parse_clock(&override_value.end_time) else {
                continue;
            };
            if minutes_between(start_time, end_time) != row.duration_minutes {
                continue;
            }
            row.start_at = NaiveDateTime::new(row.date, start_time);
            row.end_at = NaiveDateTime::new(row.date, end_time);
        }
    }

    rows.sort_by_key(|row| row.start_at);
    rows
}

fn build_swiss_rows(
    day_key: &'static str,
    day_label: &'static str,
    date: NaiveDate,
    round: i64,
) -> Vec<RowTemplate> {
    let base = NaiveDateTime::new(date, NaiveTime::from_hms_opt(6, 30, 0).unwrap());
    let round_offset =
        Duration::minutes(round_offset(round) * (SWISS_MATCH_MINUTES + SWISS_BREAK_MINUTES));

    let mut rows = Vec::new();
    for row_index in 0..4 {
        let start_at = base
            + round_offset
            + Duration::minutes((row_index as i64) * (SWISS_MATCH_MINUTES + SWISS_BREAK_MINUTES));
        let end_at = start_at + Duration::minutes(SWISS_MATCH_MINUTES);
        rows.push(RowTemplate {
            key: format!("{day_key}-r{round}-{}", row_suffix(row_index)),
            day_key,
            day_label,
            label: format!("Round {round} · Row {}", row_name(row_index)),
            match_type: round,
            date,
            day_start: NaiveTime::from_hms_opt(6, 30, 0).unwrap(),
            day_end: NaiveTime::from_hms_opt(21, 30, 0).unwrap(),
            duration_minutes: SWISS_MATCH_MINUTES,
            start_at,
            end_at,
            slots: swiss_row_slots(round, row_index),
        });
    }
    rows
}

fn build_playoff_rows(
    day_key: &'static str,
    day_label: &'static str,
    date: NaiveDate,
) -> Vec<RowTemplate> {
    let base = NaiveDateTime::new(date, NaiveTime::from_hms_opt(6, 30, 0).unwrap());
    let mut rows = Vec::new();

    for row_index in 0..4 {
        let start_at = base
            + Duration::minutes(
                (row_index as i64) * (PLAYOFF_MATCH_MINUTES + PLAYOFF_BREAK_MINUTES),
            );
        let end_at = start_at + Duration::minutes(PLAYOFF_MATCH_MINUTES);
        rows.push(RowTemplate {
            key: format!("{day_key}-p1-{}", row_suffix(row_index)),
            day_key,
            day_label,
            label: format!("Playoff 1 · Row {}", row_name(row_index)),
            match_type: 1001,
            date,
            day_start: NaiveTime::from_hms_opt(6, 30, 0).unwrap(),
            day_end: NaiveTime::from_hms_opt(18, 30, 0).unwrap(),
            duration_minutes: PLAYOFF_MATCH_MINUTES,
            start_at,
            end_at,
            slots: playoff_one_row_slots(row_index),
        });
    }

    let playoff_two_base =
        base + Duration::minutes(4 * (PLAYOFF_MATCH_MINUTES + PLAYOFF_BREAK_MINUTES));
    for row_index in 0..4 {
        let start_at = playoff_two_base
            + Duration::minutes(
                (row_index as i64) * (PLAYOFF_MATCH_MINUTES + PLAYOFF_BREAK_MINUTES),
            );
        let end_at = start_at + Duration::minutes(PLAYOFF_MATCH_MINUTES);
        rows.push(RowTemplate {
            key: format!("{day_key}-p2-{}", row_suffix(row_index)),
            day_key,
            day_label,
            label: format!("Playoff 2 · Row {}", row_name(row_index)),
            match_type: 1002,
            date,
            day_start: NaiveTime::from_hms_opt(6, 30, 0).unwrap(),
            day_end: NaiveTime::from_hms_opt(18, 30, 0).unwrap(),
            duration_minutes: PLAYOFF_MATCH_MINUTES,
            start_at,
            end_at,
            slots: playoff_two_row_slots(row_index),
        });
    }

    rows
}

fn swiss_row_slots(round: i64, row_index: usize) -> Vec<SlotTemplate> {
    match row_index {
        0 => vec![
            open_slot(1, round, 1),
            open_slot(2, round, 2),
            open_slot(3, round, 3),
            open_slot(4, round, 4),
        ],
        1 => vec![
            open_slot(1, round, 5),
            open_slot(2, round, 6),
            open_slot(3, round, 7),
            open_slot(4, round, 8),
        ],
        2 => vec![
            open_slot(1, round, 9),
            open_slot(2, round, 10),
            open_slot(3, round, 11),
            women_slot(4, round, 1),
        ],
        _ => vec![
            women_slot(1, round, 2),
            women_slot(2, round, 3),
            women_slot(3, round, 4),
            women_slot(4, round, 5),
        ],
    }
}

fn playoff_one_row_slots(row_index: usize) -> Vec<SlotTemplate> {
    match row_index {
        0 => vec![
            playoff_open_slot(1, 1, 1),
            playoff_open_slot(2, 1, 2),
            playoff_open_slot(3, 1, 3),
            playoff_open_slot(4, 1, 4),
        ],
        1 => vec![
            playoff_open_slot(1, 1, 5),
            playoff_open_slot(2, 1, 6),
            playoff_open_slot(3, 1, 7),
            playoff_open_slot(4, 1, 8),
        ],
        2 => vec![
            playoff_open_slot(1, 1, 9),
            playoff_open_slot(2, 1, 10),
            playoff_open_slot(3, 1, 11),
            playoff_women_slot(4, 1, 1),
        ],
        _ => vec![
            playoff_women_slot(1, 1, 2),
            playoff_women_slot(2, 1, 3),
            playoff_women_slot(3, 1, 4),
            playoff_women_slot(4, 1, 5),
        ],
    }
}

fn playoff_two_row_slots(row_index: usize) -> Vec<SlotTemplate> {
    match row_index {
        0 => vec![
            playoff_open_slot(1, 2, 2),
            playoff_open_slot(2, 2, 3),
            playoff_open_slot(3, 2, 4),
            playoff_open_slot(4, 2, 5),
        ],
        1 => vec![
            playoff_open_slot(1, 2, 6),
            playoff_open_slot(2, 2, 7),
            playoff_open_slot(3, 2, 8),
            playoff_women_slot(4, 2, 2),
        ],
        2 => vec![
            playoff_open_slot(1, 2, 1),
            playoff_women_slot(2, 2, 3),
            playoff_women_slot(3, 2, 4),
        ],
        _ => vec![
            playoff_women_slot(1, 2, 1),
            playoff_open_slot(2, 2, 9),
            playoff_open_slot(3, 2, 10),
        ],
    }
}

fn open_slot(field_index: usize, round: i64, slot_number: usize) -> SlotTemplate {
    SlotTemplate {
        field_index,
        division: 0,
        slot_code: format!("O R{round}-{slot_number:02}"),
    }
}

fn women_slot(field_index: usize, round: i64, slot_number: usize) -> SlotTemplate {
    SlotTemplate {
        field_index,
        division: 1,
        slot_code: format!("W R{round}-{slot_number:02}"),
    }
}

fn playoff_open_slot(field_index: usize, playoff_round: i64, slot_number: usize) -> SlotTemplate {
    SlotTemplate {
        field_index,
        division: 0,
        slot_code: format!("O P{playoff_round}-{slot_number:02}"),
    }
}

fn playoff_women_slot(field_index: usize, playoff_round: i64, slot_number: usize) -> SlotTemplate {
    SlotTemplate {
        field_index,
        division: 1,
        slot_code: format!("W P{playoff_round}-{slot_number:02}"),
    }
}

fn round_offset(round: i64) -> i64 {
    match round {
        1 => 0,
        2 => 4,
        3 => 8,
        4 => 0,
        5 => 4,
        6 => 8,
        _ => 0,
    }
}

fn row_suffix(row_index: usize) -> &'static str {
    match row_index {
        0 => "a",
        1 => "b",
        2 => "c",
        _ => "d",
    }
}

fn row_name(row_index: usize) -> &'static str {
    match row_index {
        0 => "A",
        1 => "B",
        2 => "C",
        _ => "D",
    }
}

fn standings_round_for_match_type(division: i64, match_type: i64) -> i64 {
    if match_type <= 1 {
        0
    } else if match_type < 1000 {
        match_type - 1
    } else if division == 0 {
        crate::OPEN_ROUNDS
    } else {
        crate::WOMEN_ROUNDS
    }
}

async fn build_schedule_rank_snapshots(
    db: &sqlx::SqlitePool,
    rows: &[RowTemplate],
) -> HashMap<(i64, i64), HashMap<i64, i64>> {
    let mut needed_snapshots = HashSet::new();
    let mut needed_final_snapshots = HashSet::new();
    for row in rows {
        for slot in &row.slots {
            if row.match_type == 1002 {
                needed_final_snapshots.insert(slot.division);
            } else {
                needed_snapshots.insert((
                    slot.division,
                    standings_round_for_match_type(slot.division, row.match_type),
                ));
            }
        }
    }

    let mut snapshots = HashMap::new();
    for (division, standings_round) in needed_snapshots {
        let standings =
            sorting::get_sorted_standings_through_round(db, division, standings_round).await;
        let ranks = standings
            .into_iter()
            .enumerate()
            .map(|(index, team)| (team.team_id, index as i64 + 1))
            .collect();
        snapshots.insert((division, standings_round), ranks);
    }

    for division in needed_final_snapshots {
        let standings = sorting::get_sorted_standings(db, division).await;
        let playoff_results = fetch_completed_playoff_results(db, division)
            .await
            .unwrap_or_default();
        let ranks = rounds::build_seed_order_after_playoffs(&standings, &playoff_results)
            .into_iter()
            .enumerate()
            .map(|(index, team_id)| (team_id, index as i64 + 1))
            .collect();
        snapshots.insert((division, 1002), ranks);
    }

    snapshots
}

async fn fetch_field_slots(db: &sqlx::SqlitePool) -> Vec<FieldSlot> {
    let field_rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, name FROM fields ORDER BY id ASC LIMIT 4")
            .fetch_all(db)
            .await
            .unwrap_or_default();

    let mut fields = Vec::new();
    for index in 0..FIELD_COUNT {
        if let Some((id, name)) = field_rows.get(index) {
            fields.push(FieldSlot {
                id: *id,
                label: format!("G{}", index + 1),
                name: name.clone(),
            });
        }
    }
    fields
}

async fn fetch_grid_matches(db: &sqlx::SqlitePool) -> Vec<ScheduleGridMatchRecord> {
    sqlx::query_as::<_, ScheduleGridMatchRecord>(
        r#"SELECT m.id, m.t1_id, m.t2_id, t1.division as division,
                             m.t1_score, m.t2_score,
                             m.field_id, COALESCE(m.time, '') as time,
               m.possession, m.stream_url, m.type as match_type
        FROM matches m
        JOIN teams t1 ON t1.id = m.t1_id
        JOIN teams t2 ON t2.id = m.t2_id
        WHERE m.deleted_at IS NULL
        ORDER BY m.time ASC, m.field_id ASC, m.id ASC"#,
    )
    .fetch_all(db)
    .await
    .unwrap_or_default()
}

fn materialize_grid(
    rows: Vec<RowTemplate>,
    fields: Vec<FieldSlot>,
    matches: Vec<ScheduleGridMatchRecord>,
    rank_snapshots: &HashMap<(i64, i64), HashMap<i64, i64>>,
) -> Vec<ScheduleGridRow> {
    let mut exact_matches = HashMap::new();
    let mut fallback_matches: HashMap<(i64, i64), VecDeque<ScheduleGridMatchRecord>> =
        HashMap::new();

    for record in matches {
        if let Some(field_id) = record.field_id {
            exact_matches.insert((record.time.clone(), field_id), record.clone());
        }
        fallback_matches
            .entry((record.division, record.match_type))
            .or_default()
            .push_back(record);
    }

    let mut grid_rows = Vec::new();
    for row in rows {
        let row_time = row.start_at.format("%Y-%m-%d %H:%M:%S").to_string();
        let mut cells = Vec::new();
        for field_index in 1..=FIELD_COUNT {
            let field = fields
                .iter()
                .find(|candidate| candidate.label == format!("G{field_index}"));
            let slot_template = row
                .slots
                .iter()
                .find(|slot| slot.field_index == field_index);
            let rank_snapshot_key = slot_template.map(|slot| {
                if row.match_type == 1002 {
                    (slot.division, 1002)
                } else {
                    (
                        slot.division,
                        standings_round_for_match_type(slot.division, row.match_type),
                    )
                }
            });

            let matched = match (field, slot_template) {
                (Some(field), Some(slot)) => {
                    if let Some(record) = exact_matches.remove(&(row_time.clone(), field.id)) {
                        Some(record)
                    } else {
                        fallback_matches
                            .get_mut(&(slot.division, row.match_type))
                            .and_then(|records| pop_unassigned(records, &row_time, field.id))
                    }
                }
                _ => None,
            };

            let status = matched
                .as_ref()
                .map(|record| match_status(record.possession))
                .unwrap_or_else(|| "empty".to_string());
            let clickable = matched
                .as_ref()
                .map(|record| is_clickable_status(record.possession))
                .unwrap_or(false);
            let movable = matched
                .as_ref()
                .map(|record| record.possession.is_none())
                .unwrap_or(false);
            let rank_lookup = rank_snapshot_key.and_then(|key| rank_snapshots.get(&key));

            cells.push(ScheduleGridCell {
                field_index: field_index as i64,
                field_label: format!("G{field_index}"),
                field_name: field
                    .map(|item| item.name.clone())
                    .unwrap_or_else(|| format!("G{field_index}")),
                slot_code: slot_template.map(|slot| slot.slot_code.clone()),
                division: slot_template.map(|slot| slot.division),
                match_type: slot_template.map(|_| row.match_type),
                match_id: matched.as_ref().map(|record| record.id),
                data: matched.as_ref().map(|record| {
                    [
                        record.t1_id,
                        record.t2_id,
                        record.match_type,
                        record.t1_score,
                        record.t2_score,
                    ]
                }),
                seed_ranks: matched.as_ref().and_then(|record| {
                    let t1_seed = rank_lookup.and_then(|lookup| lookup.get(&record.t1_id).copied());
                    let t2_seed = rank_lookup.and_then(|lookup| lookup.get(&record.t2_id).copied());
                    match (t1_seed, t2_seed) {
                        (Some(t1_seed), Some(t2_seed)) => Some([t1_seed, t2_seed]),
                        _ => None,
                    }
                }),
                stream_url: matched
                    .as_ref()
                    .and_then(|record| record.stream_url.clone()),
                possession: matched.as_ref().and_then(|record| record.possession),
                status,
                clickable,
                movable,
            });
        }

        grid_rows.push(ScheduleGridRow {
            key: row.key,
            day_key: row.day_key.to_string(),
            day_label: row.day_label.to_string(),
            label: row.label,
            start_time: row.start_at.format("%H:%M").to_string(),
            end_time: row.end_at.format("%H:%M").to_string(),
            cells,
        });
    }

    grid_rows
}

fn pop_unassigned(
    records: &mut VecDeque<ScheduleGridMatchRecord>,
    row_time: &str,
    field_id: i64,
) -> Option<ScheduleGridMatchRecord> {
    if let Some(position) = records
        .iter()
        .position(|record| record.time == row_time && record.field_id == Some(field_id))
    {
        return records.remove(position);
    }

    records.pop_front()
}

async fn load_row_overrides() -> HashMap<String, RowTimeOverride> {
    cache::get::<HashMap<String, RowTimeOverride>>(ROW_OVERRIDE_CACHE_KEY)
        .await
        .unwrap_or_default()
}

async fn save_row_overrides(overrides: &HashMap<String, RowTimeOverride>) {
    cache::set(ROW_OVERRIDE_CACHE_KEY, overrides, ROW_OVERRIDE_TTL_SECONDS).await;
}

fn parse_clock(value: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(value, "%H:%M").ok()
}

fn minutes_between(start: NaiveTime, end: NaiveTime) -> i64 {
    end.signed_duration_since(start).num_minutes()
}

fn match_status(possession: Option<i64>) -> String {
    match possession {
        None => "upcoming".to_string(),
        Some(value) if value >= 3 => "done".to_string(),
        Some(_) => "live".to_string(),
    }
}

fn is_clickable_status(possession: Option<i64>) -> bool {
    matches!(possession, Some(value) if value >= 1)
}

async fn verify_super(
    state: &crate::AppState,
    headers: &HeaderMap,
) -> Result<(), axum::http::StatusCode> {
    let email = extract_email(headers)?;
    let user: Option<(i64,)> =
        sqlx::query_as("SELECT role FROM users WHERE email = ? AND deleted_at IS NULL")
            .bind(&email)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    if user.ok_or(axum::http::StatusCode::FORBIDDEN)?.0 != 0 {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }

    Ok(())
}

fn extract_email(headers: &HeaderMap) -> Result<String, axum::http::StatusCode> {
    crate::helpers::auth::extract_email(headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swiss_round_rows_fit_open_and_women_team_counts() {
        let rows = build_schedule_rows(&HashMap::new());

        for round in 1..=6 {
            let mut open_slots = 0;
            let mut women_slots = 0;

            for row in rows.iter().filter(|row| row.match_type == round) {
                for slot in &row.slots {
                    if slot.division == 0 {
                        open_slots += 1;
                    } else {
                        women_slots += 1;
                    }
                }
            }

            assert_eq!(
                open_slots, 11,
                "round {round} should have 11 open swiss slots"
            );
            assert_eq!(
                women_slots, 5,
                "round {round} should have 5 women swiss slots"
            );
        }
    }

    #[test]
    fn playoff_rows_fit_twenty_two_open_and_ten_women_teams() {
        let rows = build_schedule_rows(&HashMap::new());

        let playoff_one_open = rows
            .iter()
            .filter(|row| row.match_type == 1001)
            .flat_map(|row| row.slots.iter())
            .filter(|slot| slot.division == 0)
            .count();
        let playoff_one_women = rows
            .iter()
            .filter(|row| row.match_type == 1001)
            .flat_map(|row| row.slots.iter())
            .filter(|slot| slot.division == 1)
            .count();
        let playoff_two_open = rows
            .iter()
            .filter(|row| row.match_type == 1002)
            .flat_map(|row| row.slots.iter())
            .filter(|slot| slot.division == 0)
            .count();
        let playoff_two_women = rows
            .iter()
            .filter(|row| row.match_type == 1002)
            .flat_map(|row| row.slots.iter())
            .filter(|slot| slot.division == 1)
            .count();

        assert_eq!(playoff_one_open, 11);
        assert_eq!(playoff_one_women, 5);
        assert_eq!(playoff_two_open, 10);
        assert_eq!(playoff_two_women, 4);
    }

    #[test]
    fn swiss_slots_assign_open_before_women() {
        let rows = build_schedule_rows(&HashMap::new());

        for round in 1..=6 {
            let slot_divisions: Vec<i64> = rows
                .iter()
                .filter(|row| row.match_type == round)
                .flat_map(|row| row.slots.iter().map(|slot| slot.division))
                .collect();

            assert_eq!(
                slot_divisions.len(),
                16,
                "round {round} should expose 16 total swiss slots"
            );
            assert!(slot_divisions[..11].iter().all(|division| *division == 0));
            assert!(slot_divisions[11..].iter().all(|division| *division == 1));
        }
    }
}
