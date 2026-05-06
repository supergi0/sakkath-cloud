use crate::helpers::{cache, rounds};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

static PERSISTENT_COIN_TOSS_SEED: OnceLock<u64> = OnceLock::new();

// Core team data fetched once from DB, reused across all criteria
#[derive(Clone, Serialize, Deserialize)]
pub struct TeamSortData {
    pub team_id: i64,
    pub name: String,
    pub abbreviation: Option<String>,
    pub small_logo: Option<String>,
    pub init_rank: i64,
    pub wins: i64,
    pub losses: i64,
    pub draws: i64,
    pub points: i64, // win=2, draw=1, loss=0
    pub points_for: i64,
    pub points_against: i64,
    pub spirit_avg: f64,
    // per-round result: 1=win, 0=draw, -1=loss
    pub round_results: Vec<i8>,
    pub opponents: Vec<i64>,
    // head-to-head: opponent_id -> 1=win, 0=draw, -1=loss
    pub h2h: HashMap<i64, i8>,
}

type TeamIdentityRow = (i64, String, Option<String>, Option<String>, i64);

#[derive(Clone, Default)]
struct DisplayStats {
    wins: i64,
    losses: i64,
    draws: i64,
    points_for: i64,
    points_against: i64,
    spirit_total: i64,
    spirit_count: i64,
}

pub async fn initialize_persistent_coin_toss_seed(db: &SqlitePool) -> Result<u64, sqlx::Error> {
    if let Some(seed) = PERSISTENT_COIN_TOSS_SEED.get() {
        return Ok(*seed);
    }

    let (seed,): (i64,) = sqlx::query_as(
        "SELECT seed FROM persistent_random_state WHERE name = 'standings_c7_coin_toss'",
    )
    .fetch_one(db)
    .await?;

    let seed = seed.max(0) as u64;
    let _ = PERSISTENT_COIN_TOSS_SEED.set(seed);
    Ok(*PERSISTENT_COIN_TOSS_SEED.get().unwrap_or(&seed))
}

fn persistent_coin_toss_seed() -> u64 {
    *PERSISTENT_COIN_TOSS_SEED.get_or_init(|| 0)
}

async fn ensure_stage_coin_toss_seed(
    db: &SqlitePool,
    division: i64,
    stage_key: i64,
) -> Result<u64, sqlx::Error> {
    let name = format!("standings_c7_coin_toss_division_{division}_stage_{stage_key}");

    sqlx::query(
        r#"INSERT OR IGNORE INTO persistent_random_state (name, seed)
           VALUES (?, ABS(RANDOM()))"#,
    )
    .bind(&name)
    .execute(db)
    .await?;

    let (seed,): (i64,) =
        sqlx::query_as("SELECT seed FROM persistent_random_state WHERE name = ?")
            .bind(&name)
            .fetch_one(db)
            .await?;

    Ok(seed.max(0) as u64)
}

async fn current_generated_stage_key(db: &SqlitePool, division: i64) -> i64 {
    sqlx::query_as::<_, (Option<i64>,)>(
        r#"SELECT MAX(m.type)
           FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE t.division = ? AND m.deleted_at IS NULL"#,
    )
    .bind(division)
    .fetch_one(db)
    .await
    .ok()
    .and_then(|row| row.0)
    .unwrap_or(1)
}

fn next_stage_key_for_round_context(division: i64, max_round: i64) -> i64 {
    if max_round < crate::OPEN_ROUNDS.min(crate::WOMEN_ROUNDS) {
        return max_round + 1;
    }

    if max_round < crate::OPEN_ROUNDS {
        return max_round + 1;
    }

    if division == 1 { 1002 } else { 1001 }
}

fn hash_team_tiebreak_state(team: &TeamSortData, points_map: &HashMap<i64, i64>) -> u64 {
    let mut hasher = DefaultHasher::new();
    team.team_id.hash(&mut hasher);
    team.points.hash(&mut hasher);
    team.points_for.hash(&mut hasher);
    team.points_against.hash(&mut hasher);
    team.wins.hash(&mut hasher);
    team.losses.hash(&mut hasher);
    team.draws.hash(&mut hasher);
    team.round_results.hash(&mut hasher);

    let mut opponent_points: Vec<(i64, i64)> = team
        .opponents
        .iter()
        .map(|opponent_id| {
            (
                *opponent_id,
                points_map.get(opponent_id).copied().unwrap_or_default(),
            )
        })
        .collect();
    opponent_points.sort_unstable();
    opponent_points.hash(&mut hasher);

    let mut h2h_entries: Vec<(i64, i8)> = team
        .h2h
        .iter()
        .map(|(opponent_id, result)| (*opponent_id, *result))
        .collect();
    h2h_entries.sort_unstable_by_key(|(opponent_id, _)| *opponent_id);
    h2h_entries.hash(&mut hasher);

    hasher.finish()
}

fn tie_group_signature(group: &[TeamSortData], snapshot: &[TeamSortData]) -> u64 {
    let points_map: HashMap<i64, i64> = snapshot.iter().map(|team| (team.team_id, team.points)).collect();
    let mut team_hashes: Vec<(i64, u64)> = group
        .iter()
        .map(|team| (team.team_id, hash_team_tiebreak_state(team, &points_map)))
        .collect();
    team_hashes.sort_unstable_by_key(|(team_id, _)| *team_id);

    let mut hasher = DefaultHasher::new();
    team_hashes.hash(&mut hasher);
    hasher.finish()
}

fn persistent_coin_toss_key_with_seed(seed: u64, team_id: i64, group_signature: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    group_signature.hash(&mut hasher);
    team_id.hash(&mut hasher);
    hasher.finish()
}

async fn fetch_sort_data_with_round_limit(
    db: &SqlitePool,
    division: i64,
    max_round: Option<i64>,
    require_post_match_complete: bool,
) -> Vec<TeamSortData> {
    let teams: Vec<TeamIdentityRow> = sqlx::query_as(
        "SELECT id, name, abbreviation, small_logo, COALESCE(init_rank, 9999) FROM teams WHERE division = ? AND deleted_at IS NULL ORDER BY init_rank ASC"
    ).bind(division).fetch_all(db).await.unwrap_or_default();

    // All completed swiss matches for this division
        let matches: Vec<(i64, i64, i64, i64, i64, Option<i64>, Option<i64>)> = sqlx::query_as(
                r#"SELECT m.id, m.t1_id, m.t2_id, m.t1_score, m.t2_score, m.t1_spirit, m.t2_spirit
           FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE t.division = ? AND m.possession >= 3 AND m.deleted_at IS NULL AND m.type < 1000
             AND (? IS NULL OR m.type <= ?)
           ORDER BY m.type ASC"#,
    )
    .bind(division)
    .bind(max_round)
    .bind(max_round)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let mut data_map: HashMap<i64, TeamSortData> = HashMap::new();
    for (id, name, abbreviation, logo, rank) in &teams {
        data_map.insert(
            *id,
            TeamSortData {
                team_id: *id,
                name: name.clone(),
                abbreviation: abbreviation.clone(),
                small_logo: logo.clone(),
                init_rank: *rank,
                wins: 0,
                losses: 0,
                draws: 0,
                points: 0,
                points_for: 0,
                points_against: 0,
                spirit_avg: 0.0,
                round_results: Vec::new(),
                opponents: Vec::new(),
                h2h: HashMap::new(),
            },
        );
    }

    let mut spirit_totals: HashMap<i64, (i64, i64)> = HashMap::new();

    // Process matches into team data
    for (match_id, t1, t2, s1, s2, t1_spirit, t2_spirit) in &matches {
        if require_post_match_complete
            && !crate::controllers::matches::match_post_match_is_complete(db, *match_id)
                .await
                .unwrap_or(false)
        {
            continue;
        }

        let (t1_res, t2_res): (i8, i8) = if s1 > s2 {
            (1, -1)
        } else if s1 < s2 {
            (-1, 1)
        } else {
            (0, 0)
        };

        if let Some(d) = data_map.get_mut(t1) {
            d.points_for += s1;
            d.points_against += s2;
            d.opponents.push(*t2);
            match t1_res {
                1 => {
                    d.wins += 1;
                    d.points += 2;
                }
                0 => {
                    d.draws += 1;
                    d.points += 1;
                }
                _ => {
                    d.losses += 1;
                }
            }
            d.round_results.push(t1_res);
            d.h2h.insert(*t2, t1_res);
        }
        if let Some(d) = data_map.get_mut(t2) {
            d.points_for += s2;
            d.points_against += s1;
            d.opponents.push(*t1);
            match t2_res {
                1 => {
                    d.wins += 1;
                    d.points += 2;
                }
                0 => {
                    d.draws += 1;
                    d.points += 1;
                }
                _ => {
                    d.losses += 1;
                }
            }
            d.round_results.push(t2_res);
            d.h2h.insert(*t1, t2_res);
        }

        if let Some(spirit) = t1_spirit {
            let entry = spirit_totals.entry(*t1).or_insert((0, 0));
            entry.0 += *spirit;
            entry.1 += 1;
        }

        if let Some(spirit) = t2_spirit {
            let entry = spirit_totals.entry(*t2).or_insert((0, 0));
            entry.0 += *spirit;
            entry.1 += 1;
        }
    }

    for (team_id, (total, count)) in spirit_totals {
        if let Some(d) = data_map.get_mut(&team_id)
            && count > 0
        {
            d.spirit_avg = total as f64 / count as f64;
        }
    }

    data_map.into_values().collect()
}

async fn fetch_live_sort_data_with_round_limit(
    db: &SqlitePool,
    division: i64,
    max_round: Option<i64>,
) -> Vec<TeamSortData> {
    let teams: Vec<TeamIdentityRow> = sqlx::query_as(
        "SELECT id, name, abbreviation, small_logo, COALESCE(init_rank, 9999) FROM teams WHERE division = ? AND deleted_at IS NULL ORDER BY init_rank ASC"
    ).bind(division).fetch_all(db).await.unwrap_or_default();

    let matches: Vec<(i64, i64, i64, i64, i64, Option<i64>, Option<i64>, Option<i64>)> = sqlx::query_as(
        r#"SELECT m.id, m.t1_id, m.t2_id, m.t1_score, m.t2_score, m.possession, m.t1_spirit, m.t2_spirit
           FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE t.division = ? AND m.possession IS NOT NULL AND m.deleted_at IS NULL AND m.type < 1000
             AND (? IS NULL OR m.type <= ?)
           ORDER BY m.type ASC, m.id ASC"#,
    )
    .bind(division)
    .bind(max_round)
    .bind(max_round)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let mut data_map: HashMap<i64, TeamSortData> = HashMap::new();
    for (id, name, abbreviation, logo, rank) in &teams {
        data_map.insert(
            *id,
            TeamSortData {
                team_id: *id,
                name: name.clone(),
                abbreviation: abbreviation.clone(),
                small_logo: logo.clone(),
                init_rank: *rank,
                wins: 0,
                losses: 0,
                draws: 0,
                points: 0,
                points_for: 0,
                points_against: 0,
                spirit_avg: 0.0,
                round_results: Vec::new(),
                opponents: Vec::new(),
                h2h: HashMap::new(),
            },
        );
    }

    let mut spirit_totals: HashMap<i64, (i64, i64)> = HashMap::new();

    for (_match_id, t1, t2, s1, s2, possession, t1_spirit, t2_spirit) in &matches {
        let (t1_res, t2_res): (i8, i8) = if s1 > s2 {
            (1, -1)
        } else if s1 < s2 {
            (-1, 1)
        } else {
            (0, 0)
        };

        if let Some(d) = data_map.get_mut(t1) {
            d.points_for += s1;
            d.points_against += s2;
            d.opponents.push(*t2);
            match t1_res {
                1 => {
                    d.wins += 1;
                    d.points += 2;
                }
                0 => {
                    d.draws += 1;
                    d.points += 1;
                }
                _ => {
                    d.losses += 1;
                }
            }
            d.round_results.push(t1_res);
            d.h2h.insert(*t2, t1_res);
        }
        if let Some(d) = data_map.get_mut(t2) {
            d.points_for += s2;
            d.points_against += s1;
            d.opponents.push(*t1);
            match t2_res {
                1 => {
                    d.wins += 1;
                    d.points += 2;
                }
                0 => {
                    d.draws += 1;
                    d.points += 1;
                }
                _ => {
                    d.losses += 1;
                }
            }
            d.round_results.push(t2_res);
            d.h2h.insert(*t1, t2_res);
        }

        if matches!(possession, Some(value) if *value >= 3) {
            if let Some(spirit) = t1_spirit {
                let entry = spirit_totals.entry(*t1).or_insert((0, 0));
                entry.0 += *spirit;
                entry.1 += 1;
            }

            if let Some(spirit) = t2_spirit {
                let entry = spirit_totals.entry(*t2).or_insert((0, 0));
                entry.0 += *spirit;
                entry.1 += 1;
            }
        }
    }

    for (team_id, (total, count)) in spirit_totals {
        if let Some(d) = data_map.get_mut(&team_id)
            && count > 0
        {
            d.spirit_avg = total as f64 / count as f64;
        }
    }

    data_map.into_values().collect()
}

// Fetch all sort data for a division in minimal queries
pub async fn fetch_sort_data(db: &SqlitePool, division: i64) -> Vec<TeamSortData> {
    fetch_sort_data_with_round_limit(db, division, None, true).await
}

// C1: Compare by points (win=2, draw=1, loss=0) descending
pub fn c1_points(a: &TeamSortData, b: &TeamSortData) -> Ordering {
    b.points.cmp(&a.points)
}

// C2: Head-to-head. If a beat b, a ranks higher.
pub fn c2_head_to_head(a: &TeamSortData, b: &TeamSortData) -> Ordering {
    match a.h2h.get(&b.team_id) {
        Some(1) => Ordering::Less,     // a beat b
        Some(-1) => Ordering::Greater, // b beat a
        _ => Ordering::Equal,          // draw or never played
    }
}

// C3: Median Buchholz score - sum of opponent swiss points after dropping
// the highest and lowest opponent totals when at least three exist.
pub fn c3_buchholz(a: &TeamSortData, b: &TeamSortData, all: &[TeamSortData]) -> Ordering {
    let points_map: HashMap<i64, i64> = all.iter().map(|t| (t.team_id, t.points)).collect();

    let a_buch = median_buchholz_score(a, &points_map);
    let b_buch = median_buchholz_score(b, &points_map);

    b_buch.cmp(&a_buch)
}

fn median_buchholz_score(team: &TeamSortData, points_map: &HashMap<i64, i64>) -> i64 {
    let mut opponent_points: Vec<i64> = team
        .opponents
        .iter()
        .filter_map(|opponent_id| points_map.get(opponent_id).copied())
        .collect();

    opponent_points.sort_unstable();
    if opponent_points.len() > 2 {
        opponent_points[1..opponent_points.len() - 1].iter().sum()
    } else {
        opponent_points.iter().sum()
    }
}

// C4: Point difference (goals scored - goals allowed), descending
pub fn c4_point_difference(a: &TeamSortData, b: &TeamSortData) -> Ordering {
    let a_diff = a.points_for - a.points_against;
    let b_diff = b.points_for - b.points_against;
    b_diff.cmp(&a_diff)
}

// C5: Points scored (total goals), descending
pub fn c5_points_scored(a: &TeamSortData, b: &TeamSortData) -> Ordering {
    b.points_for.cmp(&a.points_for)
}

// C6: Momentum score - cumulative points after each round (higher = earlier wins)
pub fn c6_momentum_score(a: &TeamSortData, b: &TeamSortData) -> Ordering {
    let momentum = |results: &[i8]| -> i64 {
        let mut cumulative = 0i64;
        let mut total = 0i64;
        for r in results {
            match r {
                1 => cumulative += 2,
                0 => cumulative += 1,
                _ => {}
            }
            total += cumulative;
        }
        total
    };
    let a_mom = momentum(&a.round_results);
    let b_mom = momentum(&b.round_results);
    b_mom.cmp(&a_mom)
}

// C7: Random coin toss fallback for teams still tied after c1-c6.
pub fn c7_random_coin_toss() -> i64 {
    i64::from(rand::random::<bool>())
}

fn compare_teams_through_c6(a: &TeamSortData, b: &TeamSortData, snapshot: &[TeamSortData]) -> Ordering {
    let mut ord = c1_points(a, b);
    if ord != Ordering::Equal {
        return ord;
    }

    if tied_team_count(a.points, snapshot) == 2 {
        ord = c2_head_to_head(a, b);
        if ord != Ordering::Equal {
            return ord;
        }
    }

    ord = c3_buchholz(a, b, snapshot);
    if ord != Ordering::Equal {
        return ord;
    }

    ord = c4_point_difference(a, b);
    if ord != Ordering::Equal {
        return ord;
    }

    ord = c5_points_scored(a, b);
    if ord != Ordering::Equal {
        return ord;
    }

    c6_momentum_score(a, b)
}

fn apply_c7_random_coin_toss_with_seed(teams: &mut [TeamSortData], snapshot: &[TeamSortData], seed: u64) {
    let mut start = 0usize;
    while start < teams.len() {
        let mut end = start + 1;
        while end < teams.len()
            && compare_teams_through_c6(&teams[start], &teams[end], snapshot) == Ordering::Equal
        {
            end += 1;
        }

        if end - start > 1 {
            let group_signature = tie_group_signature(&teams[start..end], snapshot);
            teams[start..end].sort_by_cached_key(|team| {
                persistent_coin_toss_key_with_seed(seed, team.team_id, group_signature)
            });
        }

        start = end;
    }
}

fn team_has_standings_data(team: &TeamSortData) -> bool {
    !team.round_results.is_empty()
        || team.wins != 0
        || team.losses != 0
        || team.draws != 0
        || team.points != 0
        || team.points_for != 0
        || team.points_against != 0
}

// Master sort: apply all criteria in order c1..c7
pub fn sort_teams(teams: &mut [TeamSortData]) {
    sort_teams_with_seed(teams, persistent_coin_toss_seed());
}

pub fn sort_teams_with_seed(teams: &mut [TeamSortData], seed: u64) {
    if teams.iter().all(|team| !team_has_standings_data(team)) {
        teams.sort_by_key(|team| (team.init_rank, team.team_id));
        return;
    }

    let snapshot: Vec<TeamSortData> = teams.to_vec();

    teams.sort_by(|a, b| compare_teams_through_c6(a, b, &snapshot));
    apply_c7_random_coin_toss_with_seed(teams, &snapshot, seed);
}

fn tied_team_count(points: i64, teams: &[TeamSortData]) -> usize {
    teams.iter().filter(|team| team.points == points).count()
}

// Full pipeline: fetch data and return sorted standings for a division (completed matches only)
pub async fn get_sorted_standings(db: &SqlitePool, division: i64) -> Vec<TeamSortData> {
    let mut teams = fetch_sort_data(db, division).await;
    let stage_key = current_generated_stage_key(db, division).await;
    let seed = ensure_stage_coin_toss_seed(db, division, stage_key)
        .await
        .unwrap_or_else(|_| persistent_coin_toss_seed());
    sort_teams_with_seed(&mut teams, seed);
    teams
}

pub async fn get_sorted_standings_through_round(
    db: &SqlitePool,
    division: i64,
    max_round: i64,
) -> Vec<TeamSortData> {
    let round_limit = Some(max_round.max(0));
    let mut teams = fetch_sort_data_with_round_limit(db, division, round_limit, true).await;
    let stage_key = next_stage_key_for_round_context(division, max_round.max(0));
    let seed = ensure_stage_coin_toss_seed(db, division, stage_key)
        .await
        .unwrap_or_else(|_| persistent_coin_toss_seed());
    sort_teams_with_seed(&mut teams, seed);
    teams
}

pub async fn get_generation_standings_for_stage(
    db: &SqlitePool,
    division: i64,
    stage_key: i64,
) -> Vec<TeamSortData> {
    let mut teams = fetch_sort_data_with_round_limit(db, division, None, false).await;
    let seed = ensure_stage_coin_toss_seed(db, division, stage_key)
        .await
        .unwrap_or_else(|_| persistent_coin_toss_seed());
    sort_teams_with_seed(&mut teams, seed);
    teams
}

pub async fn get_generation_standings(db: &SqlitePool, division: i64) -> Vec<TeamSortData> {
    let stage_key = current_generated_stage_key(db, division).await;
    get_generation_standings_for_stage(db, division, stage_key).await
}

pub async fn get_generation_standings_through_round(
    db: &SqlitePool,
    division: i64,
    max_round: i64,
) -> Vec<TeamSortData> {
    let round_limit = Some(max_round.max(0));
    let mut teams = fetch_sort_data_with_round_limit(db, division, round_limit, false).await;
    let stage_key = next_stage_key_for_round_context(division, max_round.max(0));
    let seed = ensure_stage_coin_toss_seed(db, division, stage_key)
        .await
        .unwrap_or_else(|_| persistent_coin_toss_seed());
    sort_teams_with_seed(&mut teams, seed);
    teams
}

// Intermediate rankings: includes live/in-progress match scores for display purposes.
// Uses current scores from ongoing games on top of completed results.
// NOT used for round generation — only for live standings display.
pub async fn get_intermediate_standings(db: &SqlitePool, division: i64) -> Vec<TeamSortData> {
    let mut teams = fetch_live_sort_data_with_round_limit(db, division, None).await;
    let stage_key = current_generated_stage_key(db, division).await;
    let seed = ensure_stage_coin_toss_seed(db, division, stage_key)
        .await
        .unwrap_or_else(|_| persistent_coin_toss_seed());
    sort_teams_with_seed(&mut teams, seed);
    teams
}

// Cached intermediate standings with Redis-first lookup and DB fallback.
pub async fn get_cached_intermediate_standings(
    db: &SqlitePool,
    division: i64,
) -> Vec<TeamSortData> {
    if let Some(mut cached) = cache::get_intermediate_standings::<Vec<TeamSortData>>(division).await {
        let stage_key = current_generated_stage_key(db, division).await;
        let seed = ensure_stage_coin_toss_seed(db, division, stage_key)
            .await
            .unwrap_or_else(|_| persistent_coin_toss_seed());
        sort_teams_with_seed(&mut cached, seed);
        return cached;
    }

    let standings = get_intermediate_standings(db, division).await;
    cache::set_intermediate_standings(division, &standings).await;
    standings
}

async fn fetch_stage_results(
    db: &SqlitePool,
    division: i64,
    match_type: i64,
    include_live: bool,
    require_post_match_complete: bool,
) -> Vec<rounds::PlayedMatchResult> {
    let rows: Vec<(i64, i64, i64, i64, i64, Option<i64>)> = sqlx::query_as(
        r#"SELECT m.id, m.t1_id, m.t2_id, m.t1_score, m.t2_score, m.possession
           FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE t.division = ? AND m.type = ? AND m.possession IS NOT NULL AND m.deleted_at IS NULL
           ORDER BY m.time ASC, m.field_id ASC, m.id ASC"#,
    )
    .bind(division)
    .bind(match_type)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let mut results = Vec::new();

    for (match_id, t1_id, t2_id, t1_score, t2_score, possession) in rows {
        let is_live = matches!(possession, Some(value) if value < 3);
        let is_ended = matches!(possession, Some(value) if value >= 3);

        if !is_ended && !(include_live && is_live) {
            continue;
        }

        if require_post_match_complete
            && is_ended
            && !crate::controllers::matches::match_post_match_is_complete(db, match_id)
                .await
                .unwrap_or(false)
        {
            continue;
        }

        if t1_score == t2_score {
            continue;
        }

        results.push(rounds::PlayedMatchResult {
            t1: t1_id,
            t2: t2_id,
            winner: if t1_score > t2_score { t1_id } else { t2_id },
        });
    }

    results
}

async fn fetch_display_stats(
    db: &SqlitePool,
    division: i64,
    include_live: bool,
) -> HashMap<i64, DisplayStats> {
    type DisplayMatchRow = (i64, i64, i64, i64, i64, Option<i64>, Option<i64>, Option<i64>);

    let matches: Vec<DisplayMatchRow> = sqlx::query_as(
        r#"SELECT m.id, m.t1_id, m.t2_id, m.t1_score, m.t2_score, m.possession, m.t1_spirit, m.t2_spirit
               FROM matches m
               JOIN teams t ON m.t1_id = t.id
               WHERE t.division = ? AND m.deleted_at IS NULL
               ORDER BY m.type ASC, m.time ASC, m.field_id ASC, m.id ASC"#,
    )
    .bind(division)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let mut stats_by_team: HashMap<i64, DisplayStats> = HashMap::new();

    for (_match_id, t1_id, t2_id, t1_score, t2_score, possession, t1_spirit, t2_spirit) in matches {
        let is_started = matches!(possession, Some(_));
        let should_include = if include_live {
            is_started
        } else {
            matches!(possession, Some(value) if value >= 3)
        };
        if !should_include {
            continue;
        }

        apply_display_result(
            &mut stats_by_team,
            t1_id,
            t1_score,
            t2_score,
            t1_spirit,
        );
        apply_display_result(
            &mut stats_by_team,
            t2_id,
            t2_score,
            t1_score,
            t2_spirit,
        );
    }

    stats_by_team
}

fn apply_display_result(
    stats_by_team: &mut HashMap<i64, DisplayStats>,
    team_id: i64,
    scored: i64,
    conceded: i64,
    spirit: Option<i64>,
) {
    let stats = stats_by_team.entry(team_id).or_default();
    stats.points_for += scored;
    stats.points_against += conceded;

    if scored != conceded {
        if scored > conceded {
            stats.wins += 1;
        } else {
            stats.losses += 1;
        }
    } else {
        stats.draws += 1;
    }

    if let Some(spirit) = spirit {
        stats.spirit_total += spirit;
        stats.spirit_count += 1;
    }
}

fn apply_display_stats(
    mut teams: Vec<TeamSortData>,
    display_stats: &HashMap<i64, DisplayStats>,
) -> Vec<TeamSortData> {
    for team in &mut teams {
        let stats = display_stats
            .get(&team.team_id)
            .cloned()
            .unwrap_or_default();
        team.wins = stats.wins;
        team.losses = stats.losses;
        team.draws = stats.draws;
        team.points = stats.wins * 2 + stats.draws;
        team.points_for = stats.points_for;
        team.points_against = stats.points_against;
        team.spirit_avg = if stats.spirit_count > 0 {
            stats.spirit_total as f64 / stats.spirit_count as f64
        } else {
            0.0
        };
    }

    teams
}

fn reorder_teams_by_team_id(
    teams: Vec<TeamSortData>,
    ordered_team_ids: Vec<i64>,
) -> Vec<TeamSortData> {
    let mut teams_by_id: HashMap<i64, TeamSortData> =
        teams.into_iter().map(|team| (team.team_id, team)).collect();

    let mut ordered = Vec::with_capacity(teams_by_id.len());
    for team_id in ordered_team_ids {
        if let Some(team) = teams_by_id.remove(&team_id) {
            ordered.push(team);
        }
    }

    ordered.extend(teams_by_id.into_values());
    ordered
}

async fn apply_post_swiss_seed_order(
    db: &SqlitePool,
    division: i64,
    teams: Vec<TeamSortData>,
    include_live: bool,
) -> Vec<TeamSortData> {
    if teams.is_empty() {
        return teams;
    }

    let playoff_results = fetch_stage_results(db, division, 1001, include_live, false).await;
    let final_results = fetch_stage_results(db, division, 1002, include_live, false).await;

    if playoff_results.is_empty() && final_results.is_empty() {
        return teams;
    }

    let ordered_team_ids = rounds::build_seed_order_after_elimination_results(
        &teams,
        &playoff_results,
        &final_results,
    );
    reorder_teams_by_team_id(teams, ordered_team_ids)
}

pub async fn get_display_standings(db: &SqlitePool, division: i64) -> Vec<TeamSortData> {
    let teams = get_sorted_standings(db, division).await;
    let ordered = apply_post_swiss_seed_order(db, division, teams, false).await;
    let display_stats = fetch_display_stats(db, division, false).await;
    apply_display_stats(ordered, &display_stats)
}

pub async fn get_display_intermediate_standings(
    db: &SqlitePool,
    division: i64,
) -> Vec<TeamSortData> {
    let teams = get_cached_intermediate_standings(db, division).await;
    let ordered = apply_post_swiss_seed_order(db, division, teams, true).await;
    let display_stats = fetch_display_stats(db, division, true).await;
    apply_display_stats(ordered, &display_stats)
}

// Refresh intermediate standings cache after score/spirit writes.
pub async fn refresh_intermediate_standings_cache(db: &SqlitePool, division: i64) {
    let standings = get_intermediate_standings(db, division).await;
    cache::set_intermediate_standings(division, &standings).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn team(team_id: i64, seed: i64) -> TeamSortData {
        TeamSortData {
            team_id,
            name: format!("Team {team_id}"),
            abbreviation: None,
            small_logo: None,
            init_rank: seed,
            wins: 2,
            losses: 1,
            draws: 0,
            points: 4,
            points_for: 30,
            points_against: 20,
            spirit_avg: 0.0,
            round_results: vec![1, 1, -1],
            opponents: vec![1, 2, 3],
            h2h: HashMap::new(),
        }
    }

    #[test]
    fn sort_teams_uses_head_to_head_before_later_tiebreakers() {
        let mut first = team(1, 2);
        let mut second = team(2, 1);
        first.h2h.insert(2, 1);
        second.h2h.insert(1, -1);

        let mut teams = vec![second, first];
        sort_teams(&mut teams);

        assert_eq!(teams[0].team_id, 1);
        assert_eq!(teams[1].team_id, 2);
    }

    #[test]
    fn sort_teams_skips_pairwise_head_to_head_for_three_way_ties() {
        let mut first = team(1, 1);
        let mut second = team(2, 2);
        let mut third = team(3, 3);

        first.h2h.insert(2, 1);
        first.h2h.insert(3, -1);
        second.h2h.insert(1, -1);
        second.h2h.insert(3, 1);
        third.h2h.insert(1, 1);
        third.h2h.insert(2, -1);

        let teams = vec![third, second, first];

        assert_eq!(
            compare_teams_through_c6(&teams[2], &teams[1], &teams),
            Ordering::Equal
        );
        assert_eq!(
            compare_teams_through_c6(&teams[1], &teams[0], &teams),
            Ordering::Equal
        );
    }

    #[test]
    fn c7_random_coin_toss_returns_binary_values() {
        for _ in 0..128 {
            let toss = c7_random_coin_toss();
            assert!(toss == 0 || toss == 1);
        }
    }

    #[test]
    fn sort_teams_keeps_init_rank_order_before_any_results() {
        let mut teams = vec![
            TeamSortData {
                team_id: 30,
                init_rank: 3,
                wins: 0,
                losses: 0,
                draws: 0,
                points: 0,
                points_for: 0,
                points_against: 0,
                round_results: Vec::new(),
                opponents: Vec::new(),
                h2h: HashMap::new(),
                ..team(30, 3)
            },
            TeamSortData {
                team_id: 10,
                init_rank: 1,
                wins: 0,
                losses: 0,
                draws: 0,
                points: 0,
                points_for: 0,
                points_against: 0,
                round_results: Vec::new(),
                opponents: Vec::new(),
                h2h: HashMap::new(),
                ..team(10, 1)
            },
            TeamSortData {
                team_id: 20,
                init_rank: 2,
                wins: 0,
                losses: 0,
                draws: 0,
                points: 0,
                points_for: 0,
                points_against: 0,
                round_results: Vec::new(),
                opponents: Vec::new(),
                h2h: HashMap::new(),
                ..team(20, 2)
            },
        ];

        sort_teams(&mut teams);

        assert_eq!(teams.iter().map(|team| team.team_id).collect::<Vec<_>>(), vec![10, 20, 30]);
    }

    #[test]
    fn c3_buchholz_uses_median_opponent_points() {
        let contender = TeamSortData {
            team_id: 1,
            name: "Team 1".to_string(),
            abbreviation: None,
            small_logo: None,
            init_rank: 1,
            wins: 2,
            losses: 1,
            draws: 1,
            points: 5,
            points_for: 0,
            points_against: 0,
            spirit_avg: 0.0,
            round_results: vec![1, 1, 0, -1],
            opponents: vec![10, 11, 12, 13],
            h2h: HashMap::new(),
        };
        let challenger = TeamSortData {
            team_id: 2,
            name: "Team 2".to_string(),
            abbreviation: None,
            small_logo: None,
            init_rank: 2,
            wins: 2,
            losses: 1,
            draws: 1,
            points: 5,
            points_for: 0,
            points_against: 0,
            spirit_avg: 0.0,
            round_results: vec![1, 0, 1, -1],
            opponents: vec![14, 15, 16, 17],
            h2h: HashMap::new(),
        };

        let all = vec![
            contender.clone(),
            challenger.clone(),
            team(10, 10),
            TeamSortData {
                team_id: 11,
                points: 8,
                wins: 4,
                ..team(11, 11)
            },
            TeamSortData {
                team_id: 12,
                points: 4,
                wins: 2,
                ..team(12, 12)
            },
            TeamSortData {
                team_id: 13,
                points: 2,
                wins: 1,
                ..team(13, 13)
            },
            TeamSortData {
                team_id: 14,
                points: 12,
                wins: 6,
                ..team(14, 14)
            },
            TeamSortData {
                team_id: 15,
                points: 6,
                wins: 3,
                ..team(15, 15)
            },
            TeamSortData {
                team_id: 16,
                points: 6,
                wins: 3,
                ..team(16, 16)
            },
            TeamSortData {
                team_id: 17,
                points: 0,
                wins: 0,
                ..team(17, 17)
            },
        ];

        assert_eq!(c3_buchholz(&contender, &challenger, &all), Ordering::Greater);
    }
}
