use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::helpers::{cache, rounds, sorting};

const FIELD_COUNT: usize = 4;
const ROW_OVERRIDE_CACHE_KEY: &str = "schedule:row_overrides";
const ROW_OVERRIDE_TTL_SECONDS: u64 = 60 * 60 * 24 * 30;

fn record_schedule_info(key: &'static str, details: serde_json::Value) {
    crate::telemetry::record_info(key, details.to_string());
}

fn record_schedule_error(details: serde_json::Value) {
    crate::telemetry::record_error("schedule.error", details.to_string());
}

async fn stage_progress(db: &sqlx::SqlitePool, division: i64, match_type: i64) -> (i64, i64) {
    let total: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*)
           FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE m.type = ? AND t.division = ? AND m.deleted_at IS NULL"#,
    )
    .bind(match_type)
    .bind(division)
    .fetch_one(db)
    .await
    .unwrap_or((0,));

    let completed: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*)
           FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE m.type = ? AND t.division = ? AND m.deleted_at IS NULL AND m.possession >= 3"#,
    )
    .bind(match_type)
    .bind(division)
    .fetch_one(db)
    .await
    .unwrap_or((0,));

    (total.0, completed.0)
}

async fn stage_match_count_on_connection(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    division: i64,
    match_type: i64,
) -> Result<i64, sqlx::Error> {
    let existing: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*)
           FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE t.division = ? AND m.type = ? AND m.deleted_at IS NULL"#,
    )
    .bind(division)
    .bind(match_type)
    .fetch_one(&mut **conn)
    .await?;

    Ok(existing.0)
}

async fn begin_immediate_stage_transaction(
    db: &sqlx::SqlitePool,
) -> Result<sqlx::pool::PoolConnection<sqlx::Sqlite>, sqlx::Error> {
    let mut conn = db.acquire().await?;
    sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;
    Ok(conn)
}

async fn finish_stage_transaction(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    result: &Result<bool, sqlx::Error>,
) -> Result<(), sqlx::Error> {
    if result.is_ok() {
        sqlx::query("COMMIT").execute(&mut **conn).await?;
    } else {
        sqlx::query("ROLLBACK").execute(&mut **conn).await?;
    }

    Ok(())
}

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
    pub draws: i64,
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
    match_type: i64,
    slot_code: String,
}

#[derive(Clone)]
struct RowTemplate {
    key: String,
    day_key: &'static str,
    day_label: &'static str,
    label: String,
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
    t1_score: i64,
    t2_score: i64,
    field_id: Option<i64>,
    time: String,
    possession: Option<i64>,
    stream_url: Option<String>,
    match_type: i64,
}

#[derive(sqlx::FromRow)]
struct ExistingStageMatchRecord {
    id: i64,
    field_id: Option<i64>,
    time: Option<String>,
    possession: Option<i64>,
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
        let (total, completed) = stage_progress(db, division, round).await;

        if total == 0
            && (round == 1 || {
                let prev = stage_progress(db, division, round - 1).await;
                prev.0 > 0 && prev.0 == prev.1
            })
        {
            record_schedule_info(
                "schedule.start",
                serde_json::json!({
                    "source": "auto_advance",
                    "division": division,
                    "round": round,
                    "stage": "swiss"
                }),
            );
            if let Err(error) = generate_next_round_internal(db, division, round).await {
                record_schedule_error(serde_json::json!({
                    "source": "auto_advance",
                    "division": division,
                    "round": round,
                    "stage": "swiss",
                    "message": error.to_string()
                }));
                return None;
            }
            record_schedule_info(
                "schedule.end",
                serde_json::json!({
                    "source": "auto_advance",
                    "division": division,
                    "round": round,
                    "stage": "swiss",
                    "action": format!("generated_round_{round}")
                }),
            );
            return Some(format!("generated_round_{round}"));
        }

        if total == 0 {
            return None;
        }

        if completed < total {
            return None;
        }
    }

    let mut generated_actions = Vec::new();

    match create_playoffs_if_needed(db, division).await {
        Ok(true) => {
            record_schedule_info(
                "schedule.start",
                serde_json::json!({
                    "source": "auto_advance",
                    "division": division,
                    "stage": "playoff_1"
                }),
            );
            record_schedule_info(
                "schedule.end",
                serde_json::json!({
                    "source": "auto_advance",
                    "division": division,
                    "stage": "playoff_1",
                    "action": "generated_playoff_1"
                }),
            );
            generated_actions.push("generated_playoff_1");
        }
        Ok(false) => {}
        Err(error) => {
            record_schedule_error(serde_json::json!({
                "source": "auto_advance",
                "division": division,
                "stage": "playoff_1",
                "message": error.to_string()
            }));
            return None;
        }
    }

    match create_finals_if_needed(db, division).await {
        Ok(true) => {
            record_schedule_info(
                "schedule.start",
                serde_json::json!({
                    "source": "auto_advance",
                    "division": division,
                    "stage": "playoff_2"
                }),
            );
            record_schedule_info(
                "schedule.end",
                serde_json::json!({
                    "source": "auto_advance",
                    "division": division,
                    "stage": "playoff_2",
                    "action": "generated_playoff_2"
                }),
            );
            generated_actions.push("generated_playoff_2");
        }
        Ok(false) => {}
        Err(error) => {
            record_schedule_error(serde_json::json!({
                "source": "auto_advance",
                "division": division,
                "stage": "playoff_2",
                "message": error.to_string()
            }));
            return None;
        }
    }

    if generated_actions.is_empty() {
        None
    } else {
        Some(generated_actions.join(","))
    }
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
        } else if match_count.0 > 0
            && let Err(err) = sync_existing_round_one_schedule(db, division).await
        {
            tracing::error!(
                "Failed to sync R1 schedule for division {}: {}",
                division,
                err
            );
        }
    }
}

async fn generate_r1_from_seeding(db: &sqlx::SqlitePool, division: i64) -> Result<(), sqlx::Error> {
    let pairings = build_seeded_round_one_pairings(db, division).await?;
    if pairings.is_empty() {
        return Ok(());
    }

    record_schedule_info(
        "schedule.start",
        serde_json::json!({
            "source": "startup",
            "division": division,
            "round": 1,
            "stage": "swiss",
            "matches_created": pairings.len()
        }),
    );
    let result = insert_pairings_into_slots(db, division, 1, &pairings).await;
    match &result {
        Ok(()) => record_schedule_info(
            "schedule.end",
            serde_json::json!({
                "source": "startup",
                "division": division,
                "round": 1,
                "stage": "swiss",
                "matches_created": pairings.len()
            }),
        ),
        Err(error) => record_schedule_error(serde_json::json!({
            "source": "startup",
            "division": division,
            "round": 1,
            "stage": "swiss",
            "message": error.to_string()
        })),
    }
    result
}

async fn load_seeded_round_one_team_ids(
    db: &sqlx::SqlitePool,
    division: i64,
) -> Result<Vec<i64>, sqlx::Error> {
    let teams: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM teams WHERE division = ? AND deleted_at IS NULL ORDER BY init_rank ASC, id ASC",
    )
    .bind(division)
    .fetch_all(db)
    .await?;

    Ok(teams.into_iter().map(|team| team.0).collect())
}

fn seeded_round_one_pairings_from_team_ids(
    team_ids: &[i64],
) -> Result<Vec<rounds::Pairing>, &'static str> {
    if !team_ids.len().is_multiple_of(2) {
        return Err("odd number of teams; cannot generate pairings");
    }

    let half = team_ids.len() / 2;
    let mut pairings = Vec::with_capacity(half);
    for index in 0..half {
        pairings.push(rounds::Pairing {
            t1: team_ids[index],
            t2: team_ids[index + half],
        });
    }

    Ok(pairings)
}

async fn build_seeded_round_one_pairings(
    db: &sqlx::SqlitePool,
    division: i64,
) -> Result<Vec<rounds::Pairing>, sqlx::Error> {
    let team_ids = load_seeded_round_one_team_ids(db, division).await?;
    seeded_round_one_pairings_from_team_ids(&team_ids)
        .map_err(|message| sqlx::Error::Protocol(message.into()))
}

async fn sync_existing_round_one_schedule(
    db: &sqlx::SqlitePool,
    division: i64,
) -> Result<bool, sqlx::Error> {
    let slots = get_slot_assignments(db, division, 1).await?;
    if slots.is_empty() {
        return Ok(false);
    }

    let existing_matches: Vec<ExistingStageMatchRecord> = sqlx::query_as(
        r#"SELECT m.id, m.field_id, m.time, m.possession
           FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE t.division = ? AND m.type = 1 AND m.deleted_at IS NULL
           ORDER BY COALESCE(m.time, ''), COALESCE(m.field_id, 0), m.id ASC"#,
    )
    .bind(division)
    .fetch_all(db)
    .await?;

    if existing_matches.len() != slots.len()
        || existing_matches
            .iter()
            .any(|record| record.possession.is_some())
    {
        return Ok(false);
    }

    let mut updated = false;
    for (existing_match, (field_id, start_time)) in existing_matches.iter().zip(slots.iter()) {
        if existing_match.field_id == Some(*field_id)
            && existing_match.time.as_deref() == Some(start_time.as_str())
        {
            continue;
        }

        sqlx::query(
            "UPDATE matches SET field_id = ?, time = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(*field_id)
        .bind(start_time)
        .bind(existing_match.id)
        .execute(db)
        .await?;
        updated = true;
    }

    if updated {
        cache::invalidate_division(division).await;
    }

    Ok(updated)
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
        let (total, completed) = stage_progress(&state.db, division, round).await;
        let scheduled: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*)
               FROM matches m
               JOIN teams t1 ON m.t1_id = t1.id
               WHERE m.type = ? AND t1.division = ? AND m.deleted_at IS NULL AND m.possession IS NULL"#,
        )
        .bind(round)
        .bind(division)
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));
        let in_progress = (total - completed - scheduled.0).max(0);

        round_status.push(RoundStatus {
            round,
            total,
            completed,
            in_progress,
            scheduled: scheduled.0,
        });
    }

    let current_round = round_status
        .iter()
        .find(|round| round.completed < round.total || round.total == 0)
        .map(|round| round.round)
        .unwrap_or(total_rounds + 1);

    let playoff_one = stage_progress(&state.db, division, 1001).await;
    let playoff_two = stage_progress(&state.db, division, 1002).await;

    let phase = if current_round <= total_rounds {
        format!("swiss_R{current_round}")
    } else if playoff_one.0 > 0 && playoff_one.1 < playoff_one.0 {
        "playoff_1".to_string()
    } else if playoff_two.0 > 0 {
        if playoff_two.1 < playoff_two.0 {
            "playoff_2".to_string()
        } else {
            "complete".to_string()
        }
    } else if playoff_one.0 > 0 {
        "playoff_2_pending".to_string()
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
    let sorted = sorting::get_display_standings(db, division).await;
    sorted
        .iter()
        .map(|team| TeamStanding {
            team_id: team.team_id,
            name: team.name.clone(),
            wins: team.wins,
            losses: team.losses,
            draws: team.draws,
            points_for: team.points_for,
            points_against: team.points_against,
            h2h_diff: 0,
            spirit_avg: team.spirit_avg,
            small_logo: team.small_logo.clone(),
        })
        .collect()
}

async fn compute_intermediate_standings(db: &sqlx::SqlitePool, division: i64) -> Vec<TeamStanding> {
    let sorted = sorting::get_display_intermediate_standings(db, division).await;
    sorted
        .iter()
        .map(|team| TeamStanding {
            team_id: team.team_id,
            name: team.name.clone(),
            wins: team.wins,
            losses: team.losses,
            draws: team.draws,
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
    let overrides = load_row_overrides(&state.db).await;
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

    let mut overrides = load_row_overrides(&state.db).await;
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
    save_row_overrides(&state.db, &overrides)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let old_time = row.start_at.format("%Y-%m-%d %H:%M:%S").to_string();
    let new_time = new_start_at.format("%Y-%m-%d %H:%M:%S").to_string();

    let field_ids: Vec<i64> = row
        .slots
        .iter()
        .filter_map(|slot| {
            fields
                .iter()
                .find(|field| field.label == format!("G{}", slot.field_index))
                .map(|field| field.id)
        })
        .collect();
    if field_ids.is_empty() {
        return Err(axum::http::StatusCode::CONFLICT);
    }
    let placeholders = vec!["?"; field_ids.len()].join(",");
    let query = format!(
        "UPDATE matches SET time = ?, updated_at = CURRENT_TIMESTAMP WHERE deleted_at IS NULL AND time = ? AND field_id IN ({placeholders})"
    );
    let mut update_query = sqlx::query(&query).bind(&new_time).bind(&old_time);
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

    let rows = build_schedule_rows(&load_row_overrides(&state.db).await);
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

    target_row
        .slots
        .iter()
        .find(|slot| {
            slot.field_index == field_index
                && slot.division == division
                && slot.match_type == match_type
        })
        .ok_or(axum::http::StatusCode::BAD_REQUEST)?;

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
        record_schedule_info(
            "schedule.inprogress",
            serde_json::json!({
                "source": "manual_generate",
                "division": division,
                "reason": "all_swiss_rounds_complete"
            }),
        );
        return Ok(Json(serde_json::json!({
            "success": false,
            "message": "All swiss rounds complete"
        })));
    }

    if next_round > 1 {
        let prev_progress = stage_progress(&state.db, division, next_round - 1).await;

        if prev_progress.0 == 0 || prev_progress.1 < prev_progress.0 {
            record_schedule_info(
                "schedule.inprogress",
                serde_json::json!({
                    "source": "manual_generate",
                    "division": division,
                    "round": next_round,
                    "reason": "previous_round_incomplete"
                }),
            );
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
        record_schedule_info(
            "schedule.inprogress",
            serde_json::json!({
                "source": "manual_generate",
                "division": division,
                "round": next_round,
                "reason": "round_already_generated"
            }),
        );
        return Ok(Json(serde_json::json!({
            "success": false,
            "message": "Round already generated"
        })));
    }

    let pairings = if next_round == 1 {
        let team_ids = load_seeded_round_one_team_ids(&state.db, division)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
        match seeded_round_one_pairings_from_team_ids(&team_ids) {
            Ok(pairings) => pairings,
            Err(message) => {
                record_schedule_error(serde_json::json!({
                    "source": "manual_generate",
                    "division": division,
                    "round": next_round,
                    "message": message
                }));
                return Ok(Json(serde_json::json!({
                    "success": false,
                    "message": "Odd number of teams; cannot generate pairings"
                })));
            }
        }
    } else {
        let history = rounds::fetch_match_history(&state.db, division).await;
        let sorted =
            sorting::get_generation_standings_for_stage(&state.db, division, next_round).await;
        if sorted.len() % 2 != 0 {
            record_schedule_error(serde_json::json!({
                "source": "manual_generate",
                "division": division,
                "round": next_round,
                "message": "odd number of teams; cannot generate pairings"
            }));
            return Ok(Json(serde_json::json!({
                "success": false,
                "message": "Odd number of teams; cannot generate pairings"
            })));
        }

        let pairings = rounds::generate_round_pairings(&sorted, &history, next_round);
        if pairings.len() * 2 != sorted.len() {
            record_schedule_error(serde_json::json!({
                "source": "manual_generate",
                "division": division,
                "round": next_round,
                "message": "could not generate complete non-overlapping pairings"
            }));
            return Ok(Json(serde_json::json!({
                "success": false,
                "message": "Could not generate complete non-overlapping pairings"
            })));
        }

        pairings
    };

    record_schedule_info(
        "schedule.start",
        serde_json::json!({
            "source": "manual_generate",
            "division": division,
            "round": next_round,
            "stage": "swiss",
            "matches_created": pairings.len()
        }),
    );

    insert_pairings_into_slots(&state.db, division, next_round, &pairings)
        .await
        .map_err(|error| {
            record_schedule_error(serde_json::json!({
                "source": "manual_generate",
                "division": division,
                "round": next_round,
                "stage": "swiss",
                "message": error.to_string()
            }));
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    cache::invalidate_all().await;

    record_schedule_info(
        "schedule.end",
        serde_json::json!({
            "source": "manual_generate",
            "division": division,
            "round": next_round,
            "stage": "swiss",
            "matches_created": pairings.len()
        }),
    );

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
        let (total, completed) = stage_progress(&state.db, division, round).await;
        if total == 0
            && (round == 1 || {
                let prev = stage_progress(&state.db, division, round - 1).await;
                prev.0 > 0 && prev.0 == prev.1
            })
        {
            record_schedule_info(
                "schedule.start",
                serde_json::json!({
                    "source": "check_gates",
                    "division": division,
                    "round": round,
                    "stage": "swiss"
                }),
            );
            if let Err(error) = generate_next_round_internal(&state.db, division, round).await {
                record_schedule_error(serde_json::json!({
                    "source": "check_gates",
                    "division": division,
                    "round": round,
                    "stage": "swiss",
                    "message": error.to_string()
                }));
                return Json(serde_json::json!({
                    "action": "error",
                    "next_gate": format!("round_{}_generation_failed", round),
                }));
            }
            cache::invalidate_all().await;
            record_schedule_info(
                "schedule.end",
                serde_json::json!({
                    "source": "check_gates",
                    "division": division,
                    "round": round,
                    "stage": "swiss",
                    "action": format!("generated_round_{round}")
                }),
            );
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

    let mut generated_actions = Vec::new();

    match create_playoffs_if_needed(&state.db, division).await {
        Ok(true) => {
            cache::invalidate_all().await;
            generated_actions.push("generated_playoff_1");
        }
        Ok(false) => {}
        Err(error) => {
            record_schedule_error(serde_json::json!({
                "source": "check_gates",
                "division": division,
                "stage": "playoff_1",
                "message": error.to_string()
            }));
            return Json(serde_json::json!({
                "action": "error",
                "next_gate": "playoff_1_generation_failed",
            }));
        }
    }

    match create_finals_if_needed(&state.db, division).await {
        Ok(true) => {
            cache::invalidate_all().await;
            generated_actions.push("generated_playoff_2");
        }
        Ok(false) => {}
        Err(error) => {
            record_schedule_error(serde_json::json!({
                "source": "check_gates",
                "division": division,
                "stage": "playoff_2",
                "message": error.to_string()
            }));
            return Json(serde_json::json!({
                "action": "error",
                "next_gate": "playoff_2_generation_failed",
            }));
        }
    }

    let playoff_one = stage_progress(&state.db, division, 1001).await;
    let playoff_two = stage_progress(&state.db, division, 1002).await;
    let next_gate = if playoff_one.0 > 0 && playoff_one.1 < playoff_one.0 {
        "playoff_1_completion".to_string()
    } else if playoff_two.0 > 0 && playoff_two.1 < playoff_two.0 {
        "playoff_2_completion".to_string()
    } else if playoff_one.0 > 0 && playoff_two.0 == 0 {
        "playoff_2_generation".to_string()
    } else if playoff_two.0 > 0 {
        "complete".to_string()
    } else {
        "playoff_1_generation".to_string()
    };

    if !generated_actions.is_empty() {
        return Json(serde_json::json!({
            "action": generated_actions.join(","),
            "next_gate": next_gate,
        }));
    }

    Json(serde_json::json!({
        "action": "none",
        "next_gate": next_gate
    }))
}

async fn generate_next_round_internal(
    db: &sqlx::SqlitePool,
    division: i64,
    round: i64,
) -> Result<(), sqlx::Error> {
    let pairings = if round == 1 {
        build_seeded_round_one_pairings(db, division).await?
    } else {
        let sorted = sorting::get_generation_standings_for_stage(db, division, round).await;
        if sorted.len() % 2 != 0 {
            return Err(sqlx::Error::Protocol(
                "odd number of teams; cannot generate pairings".into(),
            ));
        }

        let history = rounds::fetch_match_history(db, division).await;
        let pairings = rounds::generate_round_pairings(&sorted, &history, round);
        if pairings.len() * 2 != sorted.len() {
            return Err(sqlx::Error::Protocol(
                "incomplete pairings generated".into(),
            ));
        }

        pairings
    };

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
    let mut conn = begin_immediate_stage_transaction(db).await?;
    let result = async {
        if stage_match_count_on_connection(&mut conn, division, 1001).await? > 0 {
            return Ok(false);
        }

        let sorted = sorting::get_generation_standings_for_stage(db, division, 1001).await;
        let playoff_team_count = playoff_bracket_team_count(division, sorted.len());
        if playoff_team_count == 0 {
            return Ok(false);
        }

        let bracket_rounds = rounds::build_playoff_brackets(&sorted[..playoff_team_count]);
        let Some(playoff_round) = bracket_rounds.iter().find(|round| round.name == "playoffs")
        else {
            return Ok(false);
        };

        let slot_insertions =
            build_slot_insertions(db, division, 1001, &playoff_round.matches, 0).await?;
        insert_slot_insertions(&mut conn, 1001, &slot_insertions).await?;
        Ok(true)
    }
    .await;

    finish_stage_transaction(&mut conn, &result).await?;
    if matches!(result, Ok(true)) {
        cache::invalidate_division(division).await;
    }
    result
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

    let mut results = Vec::new();

    for (t1_id, t2_id, t1_score, t2_score) in rows {
        if t1_score == t2_score {
            continue;
        }

        results.push(rounds::PlayedMatchResult {
            t1: t1_id,
            t2: t2_id,
            winner: if t1_score > t2_score { t1_id } else { t2_id },
        });
    }

    Ok(results)
}

async fn create_finals_if_needed(
    db: &sqlx::SqlitePool,
    division: i64,
) -> Result<bool, sqlx::Error> {
    let mut conn = begin_immediate_stage_transaction(db).await?;
    let result = async {
        let existing_finals =
            stage_match_count_on_connection(&mut conn, division, 1002).await? as usize;

        let sorted = sorting::get_generation_standings_for_stage(db, division, 1002).await;
        if sorted.len() < 2 {
            return Ok(false);
        }

        let playoff_pair_count = playoff_bracket_team_count(division, sorted.len()) / 2;
        let playoff_stats = stage_progress(db, division, 1001).await;
        let playoffs_complete = playoff_pair_count == 0
            || (playoff_stats.0 == playoff_pair_count as i64 && playoff_stats.1 == playoff_stats.0);

        let playoff_results = fetch_completed_playoff_results(db, division).await?;
        let final_pairings =
            rounds::build_final_pairings_from_playoff_results(&sorted, &playoff_results);

        if final_pairings.is_empty() || existing_finals == final_pairings.len() {
            return Ok(false);
        }

        let ready_direct_pairings = &final_pairings[playoff_pair_count.min(final_pairings.len())..];

        if existing_finals == 0 {
            if playoffs_complete || playoff_pair_count == 0 {
                let slot_insertions =
                    build_slot_insertions(db, division, 1002, &final_pairings, 0).await?;
                insert_slot_insertions(&mut conn, 1002, &slot_insertions).await?;
                return Ok(true);
            }

            if ready_direct_pairings.is_empty() {
                return Ok(false);
            }

            let slot_insertions = build_slot_insertions(
                db,
                division,
                1002,
                ready_direct_pairings,
                playoff_pair_count,
            )
            .await?;
            insert_slot_insertions(&mut conn, 1002, &slot_insertions).await?;
            return Ok(true);
        }

        if playoffs_complete
            && playoff_pair_count > 0
            && existing_finals == ready_direct_pairings.len()
        {
            let slot_insertions =
                build_slot_insertions(db, division, 1002, &final_pairings[..playoff_pair_count], 0)
                    .await?;
            insert_slot_insertions(&mut conn, 1002, &slot_insertions).await?;
            return Ok(true);
        }

        Ok(false)
    }
    .await;

    finish_stage_transaction(&mut conn, &result).await?;
    if matches!(result, Ok(true)) {
        cache::invalidate_division(division).await;
    }
    result
}

async fn insert_slot_insertions(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    match_type: i64,
    slot_insertions: &[(rounds::Pairing, i64, String)],
) -> Result<(), sqlx::Error> {
    for (pairing, field_id, start_time) in slot_insertions {
        sqlx::query(
            "INSERT INTO matches (t1_id, t2_id, field_id, time, type) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(pairing.t1)
        .bind(pairing.t2)
        .bind(*field_id)
        .bind(start_time)
        .bind(match_type)
        .execute(&mut **conn)
        .await?;
    }

    Ok(())
}

async fn build_slot_insertions(
    db: &sqlx::SqlitePool,
    division: i64,
    match_type: i64,
    pairings: &[rounds::Pairing],
    slot_offset: usize,
) -> Result<Vec<(rounds::Pairing, i64, String)>, sqlx::Error> {
    let slots = get_slot_assignments(db, division, match_type).await?;
    if slot_offset > slots.len() || pairings.len() + slot_offset > slots.len() {
        return Err(sqlx::Error::Protocol(format!(
            "not enough fixed schedule slots for division {division} type {match_type}: {} pairings for {} slots",
            pairings.len() + slot_offset,
            slots.len()
        )));
    }

    let scheduled_pairings = {
        let mut rng = rand::rng();
        arrange_pairings_for_slots(pairings, match_type, &mut rng)
    };

    Ok(scheduled_pairings
        .into_iter()
        .zip(slots.into_iter().skip(slot_offset))
        .map(|(pairing, (field_id, start_time))| (pairing, field_id, start_time))
        .collect())
}

async fn insert_pairings_into_slots(
    db: &sqlx::SqlitePool,
    division: i64,
    match_type: i64,
    pairings: &[rounds::Pairing],
) -> Result<(), sqlx::Error> {
    insert_pairings_into_slots_with_offset(db, division, match_type, pairings, 0).await
}

async fn insert_pairings_into_slots_with_offset(
    db: &sqlx::SqlitePool,
    division: i64,
    match_type: i64,
    pairings: &[rounds::Pairing],
    slot_offset: usize,
) -> Result<(), sqlx::Error> {
    let slot_insertions =
        build_slot_insertions(db, division, match_type, pairings, slot_offset).await?;

    for (pairing, field_id, start_time) in &slot_insertions {
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

fn playoff_bracket_team_count(division: i64, team_count: usize) -> usize {
    match division {
        0 if team_count >= 8 => 8,
        1 if team_count >= 4 => 4,
        _ => 0,
    }
}

fn arrange_pairings_for_slots<R: rand::Rng + ?Sized>(
    pairings: &[rounds::Pairing],
    match_type: i64,
    rng: &mut R,
) -> Vec<rounds::Pairing> {
    let mut scheduled_pairings = pairings.to_vec();
    if (2..1000).contains(&match_type) {
        scheduled_pairings.shuffle(rng);
    }
    scheduled_pairings
}

async fn get_slot_assignments(
    db: &sqlx::SqlitePool,
    division: i64,
    match_type: i64,
) -> Result<Vec<(i64, String)>, sqlx::Error> {
    let rows = build_schedule_rows(&load_row_overrides(db).await);
    let fields = fetch_field_slots(db).await;
    if fields.len() < FIELD_COUNT {
        return Err(sqlx::Error::Protocol(
            "expected four fields for fixed schedule".into(),
        ));
    }

    let mut slots = Vec::new();
    for row in &rows {
        for slot in row
            .slots
            .iter()
            .filter(|slot| slot.division == division && slot.match_type == match_type)
        {
            let field = fields
                .iter()
                .find(|field| field.label == format!("G{}", slot.field_index))
                .ok_or_else(|| sqlx::Error::Protocol("missing field mapping for slot".into()))?;
            slots.push((
                slot.slot_code.clone(),
                field.id,
                row.start_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            ));
        }
    }
    slots.sort_by(|left, right| left.0.cmp(&right.0));

    Ok(slots
        .into_iter()
        .map(|(_, field_id, start_time)| (field_id, start_time))
        .collect())
}

fn build_schedule_rows(overrides: &HashMap<String, RowTimeOverride>) -> Vec<RowTemplate> {
    let friday = NaiveDate::from_ymd_opt(2026, 5, 22).unwrap();
    let saturday = NaiveDate::from_ymd_opt(2026, 5, 23).unwrap();
    let sunday = NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();

    let friday_start = NaiveTime::from_hms_opt(6, 0, 0).unwrap();
    let friday_end = NaiveTime::from_hms_opt(21, 55, 0).unwrap();
    let saturday_start = NaiveTime::from_hms_opt(6, 0, 0).unwrap();
    let saturday_end = NaiveTime::from_hms_opt(21, 55, 0).unwrap();
    let sunday_start = NaiveTime::from_hms_opt(6, 15, 0).unwrap();
    let sunday_end = NaiveTime::from_hms_opt(16, 55, 0).unwrap();

    let mut rows = vec![
        build_row(
            "fri-r1-a",
            "fri",
            "Friday",
            "Round 1 · Row A",
            friday,
            friday_start,
            friday_end,
            "06:00",
            "07:05",
            vec![
                open_slot(1, 1, 1),
                open_slot(2, 1, 2),
                open_slot(3, 1, 3),
                open_slot(4, 1, 4),
            ],
        ),
        build_row(
            "fri-r1-b",
            "fri",
            "Friday",
            "Round 1 · Row B",
            friday,
            friday_start,
            friday_end,
            "07:20",
            "08:25",
            vec![
                open_slot(1, 1, 5),
                open_slot(2, 1, 6),
                open_slot(3, 1, 7),
                women_slot(4, 1, 1),
            ],
        ),
        build_row(
            "fri-r1-c",
            "fri",
            "Friday",
            "Round 1 · Row C",
            friday,
            friday_start,
            friday_end,
            "08:40",
            "09:45",
            vec![
                women_slot(1, 1, 2),
                women_slot(2, 1, 3),
                women_slot(3, 1, 4),
                women_slot(4, 1, 5),
            ],
        ),
        build_row(
            "fri-r1-d",
            "fri",
            "Friday",
            "Round 1 · Row D",
            friday,
            friday_start,
            friday_end,
            "10:00",
            "11:05",
            vec![
                open_slot(1, 1, 8),
                open_slot(2, 1, 9),
                open_slot(3, 1, 10),
                open_slot(4, 1, 11),
            ],
        ),
        build_row(
            "fri-r2-a",
            "fri",
            "Friday",
            "Round 2 · Row A",
            friday,
            friday_start,
            friday_end,
            "11:20",
            "12:25",
            vec![
                women_slot(1, 2, 1),
                women_slot(2, 2, 2),
                women_slot(3, 2, 3),
                women_slot(4, 2, 4),
            ],
        ),
        build_row(
            "fri-r2-b",
            "fri",
            "Friday",
            "Round 2 · Row B",
            friday,
            friday_start,
            friday_end,
            "12:40",
            "13:45",
            vec![
                open_slot(1, 2, 1),
                open_slot(2, 2, 2),
                women_slot(3, 2, 5),
                open_slot(4, 2, 3),
            ],
        ),
        build_row(
            "fri-r2-c",
            "fri",
            "Friday",
            "Round 2 · Row C",
            friday,
            friday_start,
            friday_end,
            "14:00",
            "15:00",
            vec![
                open_slot(1, 2, 4),
                open_slot(2, 2, 5),
                open_slot(3, 2, 6),
                open_slot(4, 2, 7),
            ],
        ),
        build_row(
            "fri-r2-d",
            "fri",
            "Friday",
            "Round 2 · Row D",
            friday,
            friday_start,
            friday_end,
            "15:20",
            "16:25",
            vec![
                open_slot(1, 2, 8),
                open_slot(2, 2, 9),
                open_slot(3, 2, 10),
                open_slot(4, 2, 11),
            ],
        ),
        build_row(
            "fri-r3-a",
            "fri",
            "Friday",
            "Round 3 · Row A",
            friday,
            friday_start,
            friday_end,
            "16:50",
            "17:55",
            vec![
                women_slot(1, 3, 1),
                women_slot(2, 3, 2),
                women_slot(3, 3, 3),
                women_slot(4, 3, 4),
            ],
        ),
        build_row(
            "fri-r3-b",
            "fri",
            "Friday",
            "Round 3 · Row B",
            friday,
            friday_start,
            friday_end,
            "18:10",
            "19:15",
            vec![
                open_slot(1, 3, 1),
                women_slot(2, 3, 5),
                open_slot(3, 3, 2),
                open_slot(4, 3, 3),
            ],
        ),
        build_row(
            "fri-r3-c",
            "fri",
            "Friday",
            "Round 3 · Row C",
            friday,
            friday_start,
            friday_end,
            "19:30",
            "20:35",
            vec![
                open_slot(1, 3, 4),
                open_slot(2, 3, 5),
                open_slot(3, 3, 6),
                open_slot(4, 3, 7),
            ],
        ),
        build_row(
            "fri-r3-d",
            "fri",
            "Friday",
            "Round 3 · Row D",
            friday,
            friday_start,
            friday_end,
            "20:50",
            "21:55",
            vec![
                open_slot(1, 3, 8),
                open_slot(2, 3, 9),
                open_slot(3, 3, 10),
                open_slot(4, 3, 11),
            ],
        ),
        build_row(
            "sat-r4-a",
            "sat",
            "Saturday",
            "Round 4 · Row A",
            saturday,
            saturday_start,
            saturday_end,
            "06:00",
            "07:05",
            vec![
                open_slot(1, 4, 1),
                open_slot(2, 4, 2),
                open_slot(3, 4, 3),
                open_slot(4, 4, 4),
            ],
        ),
        build_row(
            "sat-r4-b",
            "sat",
            "Saturday",
            "Round 4 · Row B",
            saturday,
            saturday_start,
            saturday_end,
            "07:20",
            "08:25",
            vec![
                women_slot(1, 4, 1),
                open_slot(2, 4, 5),
                open_slot(3, 4, 6),
                open_slot(4, 4, 7),
            ],
        ),
        build_row(
            "sat-r4-c",
            "sat",
            "Saturday",
            "Round 4 · Row C",
            saturday,
            saturday_start,
            saturday_end,
            "08:40",
            "09:45",
            vec![
                women_slot(1, 4, 2),
                women_slot(2, 4, 3),
                women_slot(3, 4, 4),
                women_slot(4, 4, 5),
            ],
        ),
        build_row(
            "sat-r4-d",
            "sat",
            "Saturday",
            "Round 4 · Row D",
            saturday,
            saturday_start,
            saturday_end,
            "10:00",
            "11:05",
            vec![
                open_slot(1, 4, 8),
                open_slot(2, 4, 9),
                open_slot(3, 4, 10),
                open_slot(4, 4, 11),
            ],
        ),
        build_row(
            "sat-r5-a",
            "sat",
            "Saturday",
            "Round 5 · Row A",
            saturday,
            saturday_start,
            saturday_end,
            "11:20",
            "12:25",
            vec![
                women_slot(1, 5, 1),
                women_slot(2, 5, 2),
                women_slot(3, 5, 3),
                women_slot(4, 5, 4),
            ],
        ),
        build_row(
            "sat-r5-b",
            "sat",
            "Saturday",
            "Round 5 · Row B",
            saturday,
            saturday_start,
            saturday_end,
            "12:40",
            "13:45",
            vec![
                open_slot(1, 5, 1),
                women_slot(2, 5, 5),
                open_slot(3, 5, 2),
                open_slot(4, 5, 3),
            ],
        ),
        build_row(
            "sat-r5-c",
            "sat",
            "Saturday",
            "Round 5 · Row C",
            saturday,
            saturday_start,
            saturday_end,
            "14:00",
            "15:00",
            vec![
                open_slot(1, 5, 4),
                open_slot(2, 5, 5),
                open_slot(3, 5, 6),
                open_slot(4, 5, 7),
            ],
        ),
        build_row(
            "sat-r5-d",
            "sat",
            "Saturday",
            "Round 5 · Row D",
            saturday,
            saturday_start,
            saturday_end,
            "15:20",
            "16:25",
            vec![
                open_slot(1, 5, 8),
                open_slot(2, 5, 9),
                open_slot(3, 5, 10),
                open_slot(4, 5, 11),
            ],
        ),
        build_row(
            "sat-r6-a",
            "sat",
            "Saturday",
            "Round 6 · Row A",
            saturday,
            saturday_start,
            saturday_end,
            "16:50",
            "17:55",
            vec![
                women_slot(1, 6, 1),
                women_slot(2, 6, 2),
                women_slot(3, 6, 3),
                women_slot(4, 6, 4),
            ],
        ),
        build_row(
            "sat-r6-b",
            "sat",
            "Saturday",
            "Round 6 · Row B",
            saturday,
            saturday_start,
            saturday_end,
            "18:10",
            "19:15",
            vec![
                open_slot(1, 6, 1),
                open_slot(2, 6, 2),
                women_slot(3, 6, 5),
                open_slot(4, 6, 3),
            ],
        ),
        build_row(
            "sat-r6-c",
            "sat",
            "Saturday",
            "Round 6 · Row C",
            saturday,
            saturday_start,
            saturday_end,
            "19:30",
            "20:35",
            vec![
                open_slot(1, 6, 4),
                open_slot(2, 6, 5),
                open_slot(3, 6, 6),
                open_slot(4, 6, 7),
            ],
        ),
        build_row(
            "sat-r6-d",
            "sat",
            "Saturday",
            "Round 6 · Row D",
            saturday,
            saturday_start,
            saturday_end,
            "20:50",
            "21:55",
            vec![
                open_slot(1, 6, 8),
                open_slot(2, 6, 9),
                open_slot(3, 6, 10),
                open_slot(4, 6, 11),
            ],
        ),
        build_row(
            "sun-p1-a",
            "sun",
            "Sunday",
            "Playoff 1 · Row A",
            sunday,
            sunday_start,
            sunday_end,
            "06:15",
            "07:30",
            vec![
                playoff_open_slot(1, 1, 1),
                playoff_open_slot(2, 1, 2),
                playoff_open_slot(3, 1, 3),
                playoff_open_slot(4, 1, 4),
            ],
        ),
        build_row(
            "sun-p1-b",
            "sun",
            "Sunday",
            "Playoff 1 · Row B",
            sunday,
            sunday_start,
            sunday_end,
            "07:50",
            "09:05",
            vec![
                playoff_women_slot(1, 1, 1),
                playoff_women_slot(2, 1, 2),
                playoff_women_slot(3, 2, 4),
                playoff_open_slot(4, 2, 6),
            ],
        ),
        build_row(
            "sun-p2-a",
            "sun",
            "Sunday",
            "Playoff 2 · Row A",
            sunday,
            sunday_start,
            sunday_end,
            "09:25",
            "10:40",
            vec![
                playoff_open_slot(1, 2, 4),
                playoff_open_slot(2, 2, 5),
                playoff_open_slot(3, 2, 7),
                playoff_open_slot(4, 2, 8),
            ],
        ),
        build_row(
            "sun-p2-b",
            "sun",
            "Sunday",
            "Playoff 2 · Row B",
            sunday,
            sunday_start,
            sunday_end,
            "11:00",
            "12:15",
            vec![
                playoff_open_slot(1, 2, 3),
                playoff_women_slot(2, 2, 3),
                playoff_open_slot(3, 2, 9),
                playoff_open_slot(4, 2, 10),
            ],
        ),
        build_row(
            "sun-p2-c",
            "sun",
            "Sunday",
            "Playoff 2 · Row C",
            sunday,
            sunday_start,
            sunday_end,
            "12:35",
            "13:50",
            vec![
                playoff_women_slot(1, 2, 2),
                playoff_open_slot(2, 2, 2),
                playoff_open_slot(3, 2, 11),
                playoff_women_slot(4, 2, 5),
            ],
        ),
        build_row(
            "sun-p2-d",
            "sun",
            "Sunday",
            "Playoff 2 · Row D",
            sunday,
            sunday_start,
            sunday_end,
            "14:10",
            "15:25",
            vec![playoff_women_slot(1, 2, 1)],
        ),
        build_row(
            "sun-p2-e",
            "sun",
            "Sunday",
            "Playoff 2 · Row E",
            sunday,
            sunday_start,
            sunday_end,
            "15:40",
            "16:55",
            vec![playoff_open_slot(1, 2, 1)],
        ),
    ];

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

#[allow(clippy::too_many_arguments)]
fn build_row(
    key: &str,
    day_key: &'static str,
    day_label: &'static str,
    label: &str,
    date: NaiveDate,
    day_start: NaiveTime,
    day_end: NaiveTime,
    start_time: &str,
    end_time: &str,
    slots: Vec<SlotTemplate>,
) -> RowTemplate {
    let start_at = NaiveDateTime::new(date, parse_clock(start_time).expect("valid row start"));
    let end_at = NaiveDateTime::new(date, parse_clock(end_time).expect("valid row end"));

    RowTemplate {
        key: key.to_string(),
        day_key,
        day_label,
        label: label.to_string(),
        date,
        day_start,
        day_end,
        duration_minutes: minutes_between(start_at.time(), end_at.time()),
        start_at,
        end_at,
        slots,
    }
}

fn open_slot(field_index: usize, round: i64, slot_number: usize) -> SlotTemplate {
    SlotTemplate {
        field_index,
        division: 0,
        match_type: round,
        slot_code: format!("O R{round}-{slot_number:02}"),
    }
}

fn women_slot(field_index: usize, round: i64, slot_number: usize) -> SlotTemplate {
    SlotTemplate {
        field_index,
        division: 1,
        match_type: round,
        slot_code: format!("W R{round}-{slot_number:02}"),
    }
}

fn playoff_open_slot(field_index: usize, playoff_round: i64, slot_number: usize) -> SlotTemplate {
    SlotTemplate {
        field_index,
        division: 0,
        match_type: if playoff_round == 1 { 1001 } else { 1002 },
        slot_code: format!("O P{playoff_round}-{slot_number:02}"),
    }
}

fn playoff_women_slot(field_index: usize, playoff_round: i64, slot_number: usize) -> SlotTemplate {
    SlotTemplate {
        field_index,
        division: 1,
        match_type: if playoff_round == 1 { 1001 } else { 1002 },
        slot_code: format!("W P{playoff_round}-{slot_number:02}"),
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
            if slot.match_type == 1002 {
                needed_final_snapshots.insert(slot.division);
            } else {
                needed_snapshots.insert((
                    slot.division,
                    standings_round_for_match_type(slot.division, slot.match_type),
                ));
            }
        }
    }

    let mut snapshots = HashMap::new();
    for (division, standings_round) in needed_snapshots {
        let standings =
            sorting::get_generation_standings_through_round(db, division, standings_round).await;
        let ranks = standings
            .into_iter()
            .enumerate()
            .map(|(index, team)| (team.team_id, index as i64 + 1))
            .collect();
        snapshots.insert((division, standings_round), ranks);
    }

    for division in needed_final_snapshots {
        let standings = sorting::get_generation_standings_for_stage(db, division, 1002).await;
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
        r#"SELECT m.id, m.t1_id, m.t2_id,
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

    for record in matches {
        if let Some(field_id) = record.field_id {
            exact_matches.insert((record.time.clone(), field_id), record.clone());
        }
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
                if slot.match_type == 1002 {
                    (slot.division, 1002)
                } else {
                    (
                        slot.division,
                        standings_round_for_match_type(slot.division, slot.match_type),
                    )
                }
            });

            let matched = match (field, slot_template) {
                (Some(field), Some(_slot)) => exact_matches.remove(&(row_time.clone(), field.id)),
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
                match_type: slot_template.map(|slot| slot.match_type),
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

#[derive(sqlx::FromRow)]
struct RowTimeOverrideRecord {
    row_key: String,
    start_time: String,
    end_time: String,
}

async fn load_row_overrides(db: &sqlx::SqlitePool) -> HashMap<String, RowTimeOverride> {
    if let Some(cached) =
        cache::get::<HashMap<String, RowTimeOverride>>(ROW_OVERRIDE_CACHE_KEY).await
    {
        return cached;
    }

    let rows = sqlx::query_as::<_, RowTimeOverrideRecord>(
        r#"SELECT row_key, start_time, end_time
           FROM schedule_row_overrides
           ORDER BY row_key ASC"#,
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let overrides: HashMap<String, RowTimeOverride> = rows
        .into_iter()
        .map(|row| {
            (
                row.row_key,
                RowTimeOverride {
                    start_time: row.start_time,
                    end_time: row.end_time,
                },
            )
        })
        .collect();

    cache::set(ROW_OVERRIDE_CACHE_KEY, &overrides, ROW_OVERRIDE_TTL_SECONDS).await;
    overrides
}

async fn save_row_overrides(
    db: &sqlx::SqlitePool,
    overrides: &HashMap<String, RowTimeOverride>,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;

    sqlx::query("DELETE FROM schedule_row_overrides")
        .execute(&mut *tx)
        .await?;

    for (row_key, override_value) in overrides {
        sqlx::query(
            r#"INSERT INTO schedule_row_overrides (row_key, start_time, end_time, updated_at)
               VALUES (?, ?, ?, CURRENT_TIMESTAMP)"#,
        )
        .bind(row_key)
        .bind(&override_value.start_time)
        .bind(&override_value.end_time)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    cache::set(ROW_OVERRIDE_CACHE_KEY, overrides, ROW_OVERRIDE_TTL_SECONDS).await;
    Ok(())
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

    #[derive(Default)]
    struct ZeroRng;

    impl rand::RngCore for ZeroRng {
        fn next_u32(&mut self) -> u32 {
            0
        }

        fn next_u64(&mut self) -> u64 {
            0
        }

        fn fill_bytes(&mut self, dest: &mut [u8]) {
            dest.fill(0);
        }
    }

    fn sample_pairings() -> Vec<rounds::Pairing> {
        vec![
            rounds::Pairing { t1: 1, t2: 2 },
            rounds::Pairing { t1: 3, t2: 4 },
            rounds::Pairing { t1: 5, t2: 6 },
            rounds::Pairing { t1: 7, t2: 8 },
            rounds::Pairing { t1: 9, t2: 10 },
        ]
    }

    #[test]
    fn swiss_round_rows_fit_open_and_women_team_counts() {
        let rows = build_schedule_rows(&HashMap::new());

        for round in 1..=6 {
            let mut open_slots = 0;
            let mut women_slots = 0;

            for slot in rows
                .iter()
                .flat_map(|row| row.slots.iter())
                .filter(|slot| slot.match_type == round)
            {
                if slot.division == 0 {
                    open_slots += 1;
                } else {
                    women_slots += 1;
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
            .flat_map(|row| row.slots.iter())
            .filter(|slot| slot.match_type == 1001 && slot.division == 0)
            .count();
        let playoff_one_women = rows
            .iter()
            .flat_map(|row| row.slots.iter())
            .filter(|slot| slot.match_type == 1001 && slot.division == 1)
            .count();
        let playoff_two_open = rows
            .iter()
            .flat_map(|row| row.slots.iter())
            .filter(|slot| slot.match_type == 1002 && slot.division == 0)
            .count();
        let playoff_two_women = rows
            .iter()
            .flat_map(|row| row.slots.iter())
            .filter(|slot| slot.match_type == 1002 && slot.division == 1)
            .count();

        assert_eq!(playoff_one_open, 4);
        assert_eq!(playoff_one_women, 2);
        assert_eq!(playoff_two_open, 11);
        assert_eq!(playoff_two_women, 5);
    }

    #[test]
    fn swiss_slots_assign_open_before_women() {
        let rows = build_schedule_rows(&HashMap::new());

        for round in 1..=6 {
            let slot_divisions: Vec<i64> = rows
                .iter()
                .flat_map(|row| row.slots.iter())
                .filter(|slot| slot.match_type == round)
                .map(|slot| slot.division)
                .collect();

            assert_eq!(
                slot_divisions.len(),
                16,
                "round {round} should expose 16 total swiss slots"
            );
            assert_eq!(
                slot_divisions
                    .iter()
                    .filter(|division| **division == 0)
                    .count(),
                11
            );
            assert_eq!(
                slot_divisions
                    .iter()
                    .filter(|division| **division == 1)
                    .count(),
                5
            );
        }
    }

    #[test]
    fn round_one_pairings_keep_their_original_slot_order() {
        let pairings = sample_pairings();
        let mut rng = ZeroRng;

        let scheduled = arrange_pairings_for_slots(&pairings, 1, &mut rng);

        assert_eq!(scheduled, pairings);
    }

    #[test]
    fn later_swiss_pairings_are_shuffled_before_slot_assignment() {
        let pairings = sample_pairings();
        let mut rng = ZeroRng;

        let scheduled = arrange_pairings_for_slots(&pairings, 3, &mut rng);

        assert_ne!(scheduled, pairings);

        let original_set: HashSet<(i64, i64)> = pairings
            .iter()
            .map(|pairing| (pairing.t1, pairing.t2))
            .collect();
        let scheduled_set: HashSet<(i64, i64)> = scheduled
            .iter()
            .map(|pairing| (pairing.t1, pairing.t2))
            .collect();

        assert_eq!(scheduled_set, original_set);
    }

    #[test]
    fn seeded_round_one_pairings_use_top_half_vs_bottom_half() {
        let pairings = seeded_round_one_pairings_from_team_ids(&[1, 2, 3, 4, 5, 6]).unwrap();

        assert_eq!(
            pairings,
            vec![
                rounds::Pairing { t1: 1, t2: 4 },
                rounds::Pairing { t1: 2, t2: 5 },
                rounds::Pairing { t1: 3, t2: 6 },
            ]
        );
    }

    #[test]
    fn elimination_pairings_keep_their_original_slot_order() {
        let pairings = sample_pairings();
        let mut rng = ZeroRng;

        let scheduled = arrange_pairings_for_slots(&pairings, 1002, &mut rng);

        assert_eq!(scheduled, pairings);
    }
}
