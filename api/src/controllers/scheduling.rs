use axum::{
    extract::{State, Path, Query},
    Json,
};
use serde::{Deserialize, Serialize};
use crate::helpers::{sorting, rounds, cache};

// Tournament state structures
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

async fn create_playoffs_if_needed(db: &sqlx::SqlitePool, division: i64) -> Result<bool, sqlx::Error> {
    let existing_playoffs: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM matches m JOIN teams t ON m.t1_id = t.id WHERE t.division = ? AND m.type = 1001 AND m.deleted_at IS NULL"
    ).bind(division).fetch_one(db).await?;

    if existing_playoffs.0 > 0 {
        return Ok(false);
    }

    let standings = compute_standings(db, division).await;
    if standings.len() < 4 {
        return Ok(false);
    }

    let top4: Vec<i64> = standings.iter().take(4).map(|team| team.team_id).collect();
    let base_time = "2026-02-01 09:00:00";

    sqlx::query("INSERT INTO matches (t1_id, t2_id, field_id, time, type) VALUES (?, ?, ?, ?, 1001)")
        .bind(top4[0]).bind(top4[3]).bind(1_i64).bind(base_time).execute(db).await?;

    sqlx::query("INSERT INTO matches (t1_id, t2_id, field_id, time, type) VALUES (?, ?, ?, ?, 1001)")
        .bind(top4[1]).bind(top4[2]).bind(2_i64).bind(base_time).execute(db).await?;

    cache::invalidate_division(division).await;

    Ok(true)
}

async fn create_finals_if_needed(db: &sqlx::SqlitePool, division: i64) -> Result<bool, sqlx::Error> {
    let finals_existing: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM matches m JOIN teams t ON m.t1_id = t.id WHERE t.division = ? AND m.type = 1002 AND m.deleted_at IS NULL"
    ).bind(division).fetch_one(db).await?;

    if finals_existing.0 > 0 {
        return Ok(false);
    }

    let playoffs: Vec<(i64, i64, i64, i64)> = sqlx::query_as(
        r#"SELECT m.t1_id, m.t2_id, m.t1_score, m.t2_score
           FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE t.division = ? AND m.type = 1001 AND m.deleted_at IS NULL"#
    ).bind(division).fetch_all(db).await?;

    if playoffs.len() < 2 {
        return Ok(false);
    }

    let completed = playoffs.iter().filter(|(_, _, s1, s2)| s1 != s2).count();
    if completed < 2 {
        return Ok(false);
    }

    let winners: Vec<i64> = playoffs.iter().map(|(t1, t2, s1, s2)| if s1 > s2 { *t1 } else { *t2 }).collect();
    if winners.len() < 2 {
        return Ok(false);
    }

    sqlx::query("INSERT INTO matches (t1_id, t2_id, field_id, time, type) VALUES (?, ?, ?, ?, 1002)")
        .bind(winners[0]).bind(winners[1]).bind(1_i64).bind("2026-02-02 10:00:00")
        .execute(db).await?;

    cache::invalidate_division(division).await;

    Ok(true)
}

pub async fn auto_advance_division_if_ready(db: &sqlx::SqlitePool, division: i64) -> Option<String> {
    let total_rounds = if division == 0 { crate::OPEN_ROUNDS } else { crate::WOMEN_ROUNDS };

    for round in 1..=total_rounds {
        let stats: (i64, i64) = sqlx::query_as(
            r#"SELECT COUNT(*), COALESCE(SUM(CASE WHEN possession >= 3 THEN 1 ELSE 0 END), 0)
               FROM matches m JOIN teams t ON m.t1_id = t.id
               WHERE m.type = ? AND t.division = ? AND m.deleted_at IS NULL"#
        ).bind(round).bind(division).fetch_one(db).await.unwrap_or((0, 0));

        let (total, completed) = stats;

        if total == 0 {
            if round == 1 || {
                let prev: (i64, i64) = sqlx::query_as(
                    r#"SELECT COUNT(*), COALESCE(SUM(CASE WHEN possession >= 3 THEN 1 ELSE 0 END), 0)
                       FROM matches m JOIN teams t ON m.t1_id = t.id
                       WHERE m.type = ? AND t.division = ? AND m.deleted_at IS NULL"#
                ).bind(round - 1).bind(division).fetch_one(db).await.unwrap_or((0, 0));
                prev.0 > 0 && prev.0 == prev.1
            } {
                if generate_next_round_internal(db, division, round, total_rounds).await.is_ok() {
                    return Some(format!("generated_round_{}", round));
                }
                return None;
            }
            return None;
        }

        if completed < total {
            return None;
        }
    }

    if create_playoffs_if_needed(db, division).await.unwrap_or(false) {
        return Some("generated_playoffs".to_string());
    }

    if create_finals_if_needed(db, division).await.unwrap_or(false) {
        return Some("generated_finals".to_string());
    }

    None
}

// Auto-generate R1 for divisions with no matches (called on startup)
pub async fn auto_generate_initial_rounds(db: &sqlx::SqlitePool) {
    for division in 0..=1 {
        let match_count: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM matches m 
               JOIN teams t ON m.t1_id = t.id 
               WHERE t.division = ? AND m.deleted_at IS NULL"#
        ).bind(division).fetch_one(db).await.unwrap_or((0,));
        
        if match_count.0 == 0 {
            if let Err(e) = generate_r1_from_seeding(db, division).await {
                tracing::error!("Failed to generate R1 for division {}: {}", division, e);
            } else {
                tracing::info!("Auto-generated R1 for division {}", division);
            }
        }
    }
}

// Generate R1 using initial seeding (top half vs bottom half)
async fn generate_r1_from_seeding(db: &sqlx::SqlitePool, division: i64) -> Result<(), sqlx::Error> {
    let teams: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, init_rank FROM teams WHERE division = ? AND deleted_at IS NULL ORDER BY init_rank ASC"
    ).bind(division).fetch_all(db).await?;
    
    if teams.is_empty() { return Ok(()); }
    
    let n = teams.len();
    let half = n / 2;
    let base_time = "2026-01-18 09:00:00";
    
    for i in 0..half {
        let t1 = teams[i].0;
        let t2 = teams[i + half].0;
        let field_id = (i % 5) + 1;
        
        sqlx::query(
            "INSERT INTO matches (t1_id, t2_id, field_id, time, type) VALUES (?, ?, ?, ?, 1)"
        ).bind(t1).bind(t2).bind(field_id as i64).bind(base_time).execute(db).await?;
    }
    
    Ok(())
}

// Read tournament state for a division
pub async fn read_tournament_state(
    State(state): State<crate::AppState>,
    Query(params): Query<DivisionQuery>,
) -> Json<TournamentState> {
    let division = params.division;
    let total_rounds = if division == 0 { crate::OPEN_ROUNDS } else { crate::WOMEN_ROUNDS };
    
    // Get round status for each round
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
            WHERE m.type = ? AND t1.division = ? AND m.deleted_at IS NULL"#
        ).bind(round).bind(division).fetch_one(&state.db).await.unwrap_or((0, 0, 0, 0));
        
        round_status.push(RoundStatus {
            round,
            total: stats.0,
            completed: stats.1,
            in_progress: stats.2,
            scheduled: stats.3,
        });
    }
    
    // Determine current round (lowest incomplete) and phase
    let current_round = round_status.iter()
        .find(|r| r.completed < r.total || r.total == 0)
        .map(|r| r.round)
        .unwrap_or(total_rounds + 1);
    
    let phase = if current_round <= total_rounds {
        format!("swiss_R{}", current_round)
    } else {
        // Check for playoffs
        let playoff_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM matches m JOIN teams t ON m.t1_id = t.id WHERE m.type = 1001 AND t.division = ? AND m.deleted_at IS NULL"
        ).bind(division).fetch_one(&state.db).await.unwrap_or((0,));
        
        if playoff_count.0 > 0 {
            "playoffs".to_string()
        } else {
            "complete".to_string()
        }
    };
    
    // Get standings — use intermediate for live display (includes in-progress scores)
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

// Compute standings using helpers with full c1-c7 tiebreakers
async fn compute_standings(db: &sqlx::SqlitePool, division: i64) -> Vec<TeamStanding> {
    let sorted = sorting::get_sorted_standings(db, division).await;
    sorted.iter().map(|t| TeamStanding {
        team_id: t.team_id,
        name: t.name.clone(),
        wins: t.wins,
        losses: t.losses,
        points_for: t.points_for,
        points_against: t.points_against,
        h2h_diff: 0,
        spirit_avg: t.spirit_avg,
        small_logo: t.small_logo.clone(),
    }).collect()
}

// Intermediate standings for live display (includes in-progress match scores)
async fn compute_intermediate_standings(db: &sqlx::SqlitePool, division: i64) -> Vec<TeamStanding> {
    let sorted = sorting::get_cached_intermediate_standings(db, division).await;
    sorted.iter().map(|t| TeamStanding {
        team_id: t.team_id,
        name: t.name.clone(),
        wins: t.wins,
        losses: t.losses,
        points_for: t.points_for,
        points_against: t.points_against,
        h2h_diff: 0,
        spirit_avg: t.spirit_avg,
        small_logo: t.small_logo.clone(),
    }).collect()
}

// Get matches for schedule display
pub async fn get_schedule_matches(
    State(state): State<crate::AppState>,
    Query(params): Query<ScheduleQuery>,
) -> Json<Vec<ScheduleMatch>> {
    let division = params.division.unwrap_or(0);
    let status_key = params.status.as_deref().unwrap_or("all").to_ascii_lowercase();
    let should_cache = status_key == "upcoming" || status_key == "done";

    if should_cache {
        if let Some(cached) = cache::get_schedule::<Vec<ScheduleMatch>>(division, params.round, Some(status_key.as_str())).await {
            return Json(cached);
        }
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
        WHERE m.deleted_at IS NULL AND t1.division = ?"#
    );
    
    // Filter by round
    if let Some(round) = params.round {
        query.push_str(&format!(" AND m.type = {}", round));
    }
    
    // Filter by status
    if let Some(ref status) = params.status {
        match status.as_str() {
            "live" => query.push_str(" AND m.possession IS NOT NULL AND m.possession < 3"),
            "upcoming" => query.push_str(" AND m.possession IS NULL"),
            "done" => query.push_str(" AND m.possession >= 3"),
            _ => {} // "all" or invalid - no filter
        }
    }
    
    query.push_str(r#" ORDER BY m.time DESC,
        CASE 
            WHEN m.possession IS NOT NULL AND m.possession < 3 THEN 1
            WHEN m.possession IS NULL THEN 2
            WHEN m.possession >= 3 THEN 3
        END"#);
    
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

// Generate next round matches (swiss pairing)
pub async fn generate_next_round(
    State(state): State<crate::AppState>,
    Path(division): Path<i64>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let total_rounds = if division == 0 { crate::OPEN_ROUNDS } else { crate::WOMEN_ROUNDS };
    
    // Find the next round to generate
    let next_round: i64 = sqlx::query_as::<_, (i64,)>(
        r#"SELECT COALESCE(MAX(m.type), 0) + 1 
           FROM matches m 
           JOIN teams t ON m.t1_id = t.id 
           WHERE t.division = ? AND m.type < 1000 AND m.deleted_at IS NULL"#
    ).bind(division).fetch_one(&state.db).await.map(|r| r.0).unwrap_or(1);
    
    if next_round > total_rounds {
        return Ok(Json(serde_json::json!({"success": false, "message": "All swiss rounds complete"})));
    }
    
    // Verify previous round is complete
    if next_round > 1 {
        let prev_incomplete: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM matches m 
               JOIN teams t ON m.t1_id = t.id 
               WHERE m.type = ? AND t.division = ? AND (m.possession IS NULL OR m.possession < 3) AND m.deleted_at IS NULL"#
        ).bind(next_round - 1).bind(division).fetch_one(&state.db).await.unwrap_or((1,));
        
        if prev_incomplete.0 > 0 {
            return Ok(Json(serde_json::json!({"success": false, "message": "Previous round incomplete"})));
        }
    }
    
    // Check if matches already exist for this round
    let existing: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM matches m 
           JOIN teams t ON m.t1_id = t.id 
           WHERE m.type = ? AND t.division = ? AND m.deleted_at IS NULL"#
    ).bind(next_round).bind(division).fetch_one(&state.db).await.unwrap_or((0,));
    
    if existing.0 > 0 {
        return Ok(Json(serde_json::json!({"success": false, "message": "Round already generated"})));
    }
    
    // Get previous opponents for each team
    let history = rounds::fetch_match_history(&state.db, division).await;
    
    // Get sorted standings using helper (c1-c7 tiebreakers)
    let sorted = sorting::get_sorted_standings(&state.db, division).await;

    if sorted.len() % 2 != 0 {
        return Ok(Json(serde_json::json!({"success": false, "message": "Odd number of teams; cannot generate pairings"})));
    }
    
    // Generate pairings using helper backtracking algorithm
    let pairings = rounds::generate_round_pairings(&sorted, &history);

    if pairings.len() * 2 != sorted.len() {
        return Ok(Json(serde_json::json!({"success": false, "message": "Could not generate complete non-overlapping pairings"})));
    }
    
    // Insert matches
    let base_time = format!("2026-01-{:02} 09:00:00", 18 + next_round);
    for (idx, p) in pairings.iter().enumerate() {
        let field_id = (idx % 5) + 1;
        sqlx::query(
            "INSERT INTO matches (t1_id, t2_id, field_id, time, type) VALUES (?, ?, ?, ?, ?)"
        ).bind(p.t1).bind(p.t2).bind(field_id as i64).bind(&base_time).bind(next_round)
         .execute(&state.db).await
         .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    cache::invalidate_division(division).await;
    
    Ok(Json(serde_json::json!({
        "success": true, 
        "round": next_round, 
        "matches_created": pairings.len()
    })))
}

// Check and populate gates (called after match completion)
pub async fn check_and_populate_gates(
    State(state): State<crate::AppState>,
    Path(division): Path<i64>,
) -> Json<serde_json::Value> {
    let total_rounds = if division == 0 { crate::OPEN_ROUNDS } else { crate::WOMEN_ROUNDS };
    
    // Find current state
    for round in 1..=total_rounds {
        let stats: (i64, i64) = sqlx::query_as(
            r#"SELECT COUNT(*), COALESCE(SUM(CASE WHEN possession >= 3 THEN 1 ELSE 0 END), 0)
               FROM matches m JOIN teams t ON m.t1_id = t.id 
               WHERE m.type = ? AND t.division = ? AND m.deleted_at IS NULL"#
        ).bind(round).bind(division).fetch_one(&state.db).await.unwrap_or((0, 0));
        
        let (total, completed) = stats;
        
        // If no matches exist for this round, try to generate
        if total == 0 {
            if round == 1 || {
                // Check prev round complete
                let prev: (i64, i64) = sqlx::query_as(
                    r#"SELECT COUNT(*), COALESCE(SUM(CASE WHEN possession >= 3 THEN 1 ELSE 0 END), 0)
                       FROM matches m JOIN teams t ON m.t1_id = t.id 
                       WHERE m.type = ? AND t.division = ? AND m.deleted_at IS NULL"#
                ).bind(round - 1).bind(division).fetch_one(&state.db).await.unwrap_or((0, 0));
                prev.0 > 0 && prev.0 == prev.1
            } {
                // Generate this round
                let _ = generate_next_round_internal(&state.db, division, round, total_rounds).await;
                return Json(serde_json::json!({
                    "action": format!("generated_round_{}", round),
                    "next_gate": format!("round_{}", round + 1)
                }));
            }
        }
        
        // If round incomplete, stop here
        if total > 0 && completed < total {
            return Json(serde_json::json!({
                "action": "none",
                "next_gate": format!("round_{}_completion", round)
            }));
        }
    }
    
    Json(serde_json::json!({
        "action": "none",
        "next_gate": "playoffs_or_complete"
    }))
}

// Internal helper for generating rounds using helpers
async fn generate_next_round_internal(db: &sqlx::SqlitePool, division: i64, round: i64, _total_rounds: i64) -> Result<(), sqlx::Error> {
    let sorted = sorting::get_sorted_standings(db, division).await;
    if sorted.len() % 2 != 0 {
        return Err(sqlx::Error::Protocol("odd number of teams; cannot generate pairings".into()));
    }

    let history = rounds::fetch_match_history(db, division).await;

    let pairings = rounds::generate_round_pairings(&sorted, &history);
    if pairings.len() * 2 != sorted.len() {
        return Err(sqlx::Error::Protocol("incomplete pairings generated".into()));
    }

    let base_time = format!("2026-01-{:02} 09:00:00", 18 + round);
    for (idx, p) in pairings.iter().enumerate() {
        let field_id = (idx % 5) + 1;
        sqlx::query(
            "INSERT INTO matches (t1_id, t2_id, field_id, time, type) VALUES (?, ?, ?, ?, ?)"
        ).bind(p.t1).bind(p.t2).bind(field_id as i64).bind(&base_time).bind(round)
         .execute(db).await?;
    }

    cache::invalidate_division(division).await;
    Ok(())
}

// Get early fixtures (deterministic next matches)
pub async fn get_early_fixtures(
    State(state): State<crate::AppState>,
    Query(params): Query<DivisionQuery>,
) -> Json<serde_json::Value> {
    let division = params.division;
    let total_rounds = if division == 0 { crate::OPEN_ROUNDS } else { crate::WOMEN_ROUNDS };
    
    // Find current round
    let current_round: i64 = sqlx::query_as::<_, (i64,)>(
        r#"SELECT COALESCE(MIN(m.type), 1) 
           FROM matches m JOIN teams t ON m.t1_id = t.id 
           WHERE t.division = ? AND m.type < 1000 AND m.possession < 3 AND m.deleted_at IS NULL"#
    ).bind(division).fetch_one(&state.db).await.map(|r| r.0).unwrap_or(1);
    
    if current_round > total_rounds {
        return Json(serde_json::json!({
            "early_pairings": [],
            "teams_waiting": [],
            "message": "All swiss rounds complete"
        }));
    }
    
    // Get standings
    let standings = compute_standings(&state.db, division).await;
    
    // For early fixtures, we look at teams with extreme records
    // Teams at the top (e.g., undefeated) will likely face each other
    let mut early_pairings = Vec::new();
    let mut teams_waiting = Vec::new();
    
    // Simple heuristic: top 2 teams by wins are certain to be in top bracket
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
    
    // Teams waiting are those in the middle of the pack
    for team in standings.iter().skip(2).take(standings.len().saturating_sub(4)) {
        teams_waiting.push(team.team_id);
    }
    
    Json(serde_json::json!({
        "early_pairings": early_pairings,
        "teams_waiting": teams_waiting
    }))
}
