use axum::{
    extract::{State, Path, Query},
    Json,
};
use serde::{Deserialize, Serialize};

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

#[derive(Serialize, sqlx::FromRow)]
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
                SUM(CASE WHEN possession >= 3 THEN 1 ELSE 0 END) as completed,
                SUM(CASE WHEN possession IS NOT NULL AND possession < 3 THEN 1 ELSE 0 END) as in_progress,
                SUM(CASE WHEN possession IS NULL THEN 1 ELSE 0 END) as scheduled
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
    
    // Get standings with tiebreakers
    let standings = compute_standings(&state.db, division).await;
    
    Json(TournamentState {
        division,
        phase,
        current_round,
        total_rounds,
        round_status,
        standings,
    })
}

// Compute standings with proper tiebreakers
async fn compute_standings(db: &sqlx::SqlitePool, division: i64) -> Vec<TeamStanding> {
    // Get basic stats for all teams
    let teams: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        "SELECT id, name, small_logo FROM teams WHERE division = ? AND deleted_at IS NULL"
    ).bind(division).fetch_all(db).await.unwrap_or_default();
    
    let mut standings: Vec<TeamStanding> = Vec::new();
    
    for (team_id, name, small_logo) in teams {
        // Get W/L and points from completed matches only
        let stats: (i64, i64, i64, i64, f64) = sqlx::query_as(
            r#"SELECT 
                COALESCE(SUM(CASE WHEN (m.t1_id = ? AND m.t1_score > m.t2_score) OR (m.t2_id = ? AND m.t2_score > m.t1_score) THEN 1 ELSE 0 END), 0) as wins,
                COALESCE(SUM(CASE WHEN (m.t1_id = ? AND m.t1_score < m.t2_score) OR (m.t2_id = ? AND m.t2_score < m.t1_score) THEN 1 ELSE 0 END), 0) as losses,
                COALESCE(SUM(CASE WHEN m.t1_id = ? THEN m.t1_score WHEN m.t2_id = ? THEN m.t2_score ELSE 0 END), 0) as pf,
                COALESCE(SUM(CASE WHEN m.t1_id = ? THEN m.t2_score WHEN m.t2_id = ? THEN m.t1_score ELSE 0 END), 0) as pa,
                COALESCE(AVG(CASE WHEN m.t1_id = ? THEN m.t1_spirit WHEN m.t2_id = ? THEN m.t2_spirit END), 0.0) as spirit
            FROM matches m
            WHERE (m.t1_id = ? OR m.t2_id = ?) AND m.possession >= 3 AND m.deleted_at IS NULL"#
        ).bind(team_id).bind(team_id).bind(team_id).bind(team_id).bind(team_id).bind(team_id)
         .bind(team_id).bind(team_id).bind(team_id).bind(team_id).bind(team_id).bind(team_id)
         .fetch_one(db).await.unwrap_or((0, 0, 0, 0, 0.0));
        
        standings.push(TeamStanding {
            team_id,
            name,
            wins: stats.0,
            losses: stats.1,
            points_for: stats.2,
            points_against: stats.3,
            h2h_diff: 0, // Will be computed during tiebreaker
            spirit_avg: stats.4,
            small_logo,
        });
    }
    
    // Sort by tiebreaker criteria: W/L, then goal diff, then goals scored, then spirit
    standings.sort_by(|a, b| {
        // 1. Wins descending
        let win_cmp = b.wins.cmp(&a.wins);
        if win_cmp != std::cmp::Ordering::Equal { return win_cmp; }
        
        // 2. Losses ascending
        let loss_cmp = a.losses.cmp(&b.losses);
        if loss_cmp != std::cmp::Ordering::Equal { return loss_cmp; }
        
        // 3. Goal difference descending
        let a_diff = a.points_for - a.points_against;
        let b_diff = b.points_for - b.points_against;
        let diff_cmp = b_diff.cmp(&a_diff);
        if diff_cmp != std::cmp::Ordering::Equal { return diff_cmp; }
        
        // 4. Goals scored descending
        let pf_cmp = b.points_for.cmp(&a.points_for);
        if pf_cmp != std::cmp::Ordering::Equal { return pf_cmp; }
        
        // 5. Spirit average descending
        b.spirit_avg.partial_cmp(&a.spirit_avg).unwrap_or(std::cmp::Ordering::Equal)
    });
    
    standings
}

// Get matches for schedule display
pub async fn get_schedule_matches(
    State(state): State<crate::AppState>,
    Query(params): Query<ScheduleQuery>,
) -> Json<Vec<ScheduleMatch>> {
    let division = params.division.unwrap_or(0);
    
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
               WHERE m.type = ? AND t.division = ? AND m.possession < 3 AND m.deleted_at IS NULL"#
        ).bind(next_round - 1).bind(division).fetch_one(&state.db).await.unwrap_or((1,));
        
        if prev_incomplete.0 > 0 {
            return Ok(Json(serde_json::json!({"success": false, "message": "Previous round incomplete"})));
        }
    }
    
    // Get standings for pairing
    let standings = compute_standings(&state.db, division).await;
    
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
    let mut history: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    let prev_matches: Vec<(i64, i64)> = sqlx::query_as(
        r#"SELECT m.t1_id, m.t2_id FROM matches m 
           JOIN teams t ON m.t1_id = t.id 
           WHERE t.division = ? AND m.type < ? AND m.deleted_at IS NULL"#
    ).bind(division).bind(next_round).fetch_all(&state.db).await.unwrap_or_default();
    
    for (t1, t2) in prev_matches {
        history.entry(t1).or_default().push(t2);
        history.entry(t2).or_default().push(t1);
    }
    
    // Swiss pairing: group by points (wins), then pair within groups
    let mut teams_by_points: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    for team in &standings {
        teams_by_points.entry(team.wins).or_default().push(team.team_id);
    }
    
    // Sort point groups descending
    let mut point_groups: Vec<i64> = teams_by_points.keys().cloned().collect();
    point_groups.sort_by(|a, b| b.cmp(a));
    
    // Flatten and pair: top half vs bottom half within each group, overflow moves to next
    let mut all_teams: Vec<i64> = Vec::new();
    for points in point_groups {
        if let Some(teams) = teams_by_points.get(&points) {
            all_teams.extend(teams);
        }
    }
    
    // Swiss fold pairing: 1v(n/2+1), 2v(n/2+2), etc.
    let mut pairings: Vec<(i64, i64)> = Vec::new();
    let n = all_teams.len();
    let half = n / 2;
    
    for i in 0..half {
        let t1 = all_teams[i];
        let t2 = all_teams[i + half];
        
        // Check if they've played before
        let played = history.get(&t1).map(|h| h.contains(&t2)).unwrap_or(false);
        if played && i + 1 < half {
            // Try swapping with next pair
            let alt_t2 = all_teams[i + 1 + half];
            let alt_played = history.get(&t1).map(|h| h.contains(&alt_t2)).unwrap_or(false);
            if !alt_played {
                pairings.push((t1, alt_t2));
                pairings.push((all_teams[i + 1], t2));
                continue;
            }
        }
        pairings.push((t1, t2));
    }
    
    // Insert matches
    let base_time = format!("2026-01-{:02} 09:00:00", 18 + next_round);
    for (idx, (t1, t2)) in pairings.iter().enumerate() {
        let field_id = (idx % 5) + 1;
        sqlx::query(
            "INSERT INTO matches (t1_id, t2_id, field_id, time, type) VALUES (?, ?, ?, ?, ?)"
        ).bind(t1).bind(t2).bind(field_id as i64).bind(&base_time).bind(next_round)
         .execute(&state.db).await
         .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    
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
            r#"SELECT COUNT(*), SUM(CASE WHEN possession >= 3 THEN 1 ELSE 0 END)
               FROM matches m JOIN teams t ON m.t1_id = t.id 
               WHERE m.type = ? AND t.division = ? AND m.deleted_at IS NULL"#
        ).bind(round).bind(division).fetch_one(&state.db).await.unwrap_or((0, 0));
        
        let (total, completed) = stats;
        
        // If no matches exist for this round, try to generate
        if total == 0 {
            if round == 1 || {
                // Check prev round complete
                let prev: (i64, i64) = sqlx::query_as(
                    r#"SELECT COUNT(*), SUM(CASE WHEN possession >= 3 THEN 1 ELSE 0 END)
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

// Internal helper for generating rounds
async fn generate_next_round_internal(db: &sqlx::SqlitePool, division: i64, round: i64, _total_rounds: i64) -> Result<(), sqlx::Error> {
    let standings = compute_standings(db, division).await;
    
    // Get previous opponents
    let mut history: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    let prev_matches: Vec<(i64, i64)> = sqlx::query_as(
        r#"SELECT m.t1_id, m.t2_id FROM matches m 
           JOIN teams t ON m.t1_id = t.id 
           WHERE t.division = ? AND m.type < ? AND m.deleted_at IS NULL"#
    ).bind(division).bind(round).fetch_all(db).await.unwrap_or_default();
    
    for (t1, t2) in prev_matches {
        history.entry(t1).or_default().push(t2);
        history.entry(t2).or_default().push(t1);
    }
    
    // Swiss pairing
    let mut teams_by_points: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    for team in &standings {
        teams_by_points.entry(team.wins).or_default().push(team.team_id);
    }
    
    let mut point_groups: Vec<i64> = teams_by_points.keys().cloned().collect();
    point_groups.sort_by(|a, b| b.cmp(a));
    
    let mut all_teams: Vec<i64> = Vec::new();
    for points in point_groups {
        if let Some(teams) = teams_by_points.get(&points) {
            all_teams.extend(teams);
        }
    }
    
    let mut pairings: Vec<(i64, i64)> = Vec::new();
    let n = all_teams.len();
    let half = n / 2;
    
    for i in 0..half {
        let t1 = all_teams[i];
        let t2 = all_teams[i + half];
        pairings.push((t1, t2));
    }
    
    let base_time = format!("2026-01-{:02} 09:00:00", 18 + round);
    for (idx, (t1, t2)) in pairings.iter().enumerate() {
        let field_id = (idx % 5) + 1;
        sqlx::query(
            "INSERT INTO matches (t1_id, t2_id, field_id, time, type) VALUES (?, ?, ?, ?, ?)"
        ).bind(t1).bind(t2).bind(field_id as i64).bind(&base_time).bind(round)
         .execute(db).await?;
    }
    
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
