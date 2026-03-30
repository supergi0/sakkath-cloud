use serde::{Serialize, Deserialize};
use sqlx::SqlitePool;
use std::cmp::Ordering;
use std::collections::HashMap;
use rand::Rng;
use crate::helpers::cache;

// Core team data fetched once from DB, reused across all criteria
#[derive(Clone, Serialize, Deserialize)]
pub struct TeamSortData {
    pub team_id: i64,
    pub name: String,
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

// Fetch all sort data for a division in minimal queries
pub async fn fetch_sort_data(db: &SqlitePool, division: i64) -> Vec<TeamSortData> {
    let teams: Vec<(i64, String, Option<String>, i64)> = sqlx::query_as(
        "SELECT id, name, small_logo, COALESCE(init_rank, 9999) FROM teams WHERE division = ? AND deleted_at IS NULL ORDER BY init_rank ASC"
    ).bind(division).fetch_all(db).await.unwrap_or_default();

    // All completed swiss matches for this division
    let matches: Vec<(i64, i64, i64, i64, i64)> = sqlx::query_as(
        r#"SELECT m.t1_id, m.t2_id, m.t1_score, m.t2_score, m.type
           FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE t.division = ? AND m.possession >= 3 AND m.deleted_at IS NULL AND m.type < 1000
           ORDER BY m.type ASC"#
    ).bind(division).fetch_all(db).await.unwrap_or_default();

    let mut data_map: HashMap<i64, TeamSortData> = HashMap::new();
    for (id, name, logo, rank) in &teams {
        data_map.insert(*id, TeamSortData {
            team_id: *id,
            name: name.clone(),
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
        });
    }

    // Process matches into team data
    for (t1, t2, s1, s2, _round) in &matches {
        let (t1_res, t2_res): (i8, i8) = if s1 > s2 { (1, -1) } else if s1 < s2 { (-1, 1) } else { (0, 0) };

        if let Some(d) = data_map.get_mut(t1) {
            d.points_for += s1;
            d.points_against += s2;
            d.opponents.push(*t2);
            match t1_res {
                1 => { d.wins += 1; d.points += 2; }
                0 => { d.draws += 1; d.points += 1; }
                _ => { d.losses += 1; }
            }
            d.round_results.push(t1_res);
            d.h2h.insert(*t2, t1_res);
        }
        if let Some(d) = data_map.get_mut(t2) {
            d.points_for += s2;
            d.points_against += s1;
            d.opponents.push(*t1);
            match t2_res {
                1 => { d.wins += 1; d.points += 2; }
                0 => { d.draws += 1; d.points += 1; }
                _ => { d.losses += 1; }
            }
            d.round_results.push(t2_res);
            d.h2h.insert(*t1, t2_res);
        }
    }

    // Spirit averages (separate query to keep clean)
    let spirits: Vec<(i64, f64)> = sqlx::query_as(
        r#"SELECT team_id, AVG(spirit) FROM (
            SELECT m.t1_id as team_id, m.t1_spirit as spirit FROM matches m
            JOIN teams t ON m.t1_id = t.id WHERE t.division = ? AND m.t1_spirit IS NOT NULL AND m.possession >= 3 AND m.deleted_at IS NULL AND m.type < 1000
            UNION ALL
            SELECT m.t2_id as team_id, m.t2_spirit as spirit FROM matches m
            JOIN teams t ON m.t2_id = t.id WHERE t.division = ? AND m.t2_spirit IS NOT NULL AND m.possession >= 3 AND m.deleted_at IS NULL AND m.type < 1000
        ) GROUP BY team_id"#
    ).bind(division).bind(division).fetch_all(db).await.unwrap_or_default();

    for (tid, avg) in spirits {
        if let Some(d) = data_map.get_mut(&tid) {
            d.spirit_avg = avg;
        }
    }

    data_map.into_values().collect()
}

// C1: Compare by points (win=2, draw=1, loss=0) descending
pub fn c1_points(a: &TeamSortData, b: &TeamSortData) -> Ordering {
    b.points.cmp(&a.points)
}

// C2: Head-to-head. If a beat b, a ranks higher.
pub fn c2_head_to_head(a: &TeamSortData, b: &TeamSortData) -> Ordering {
    match a.h2h.get(&b.team_id) {
        Some(1) => Ordering::Less,    // a beat b
        Some(-1) => Ordering::Greater, // b beat a
        _ => Ordering::Equal,          // draw or never played
    }
}

// C3: Buchholz score - sum of wins of all opponents (higher = faced harder competition)
pub fn c3_buchholz(a: &TeamSortData, b: &TeamSortData, all: &[TeamSortData]) -> Ordering {
    let wins_map: HashMap<i64, i64> = all.iter().map(|t| (t.team_id, t.wins)).collect();

    let a_buch: i64 = a.opponents.iter().filter_map(|o| wins_map.get(o)).sum();
    let b_buch: i64 = b.opponents.iter().filter_map(|o| wins_map.get(o)).sum();

    b_buch.cmp(&a_buch)
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

// C7: True random coin flip tiebreaker
pub fn c7_rng(_a: &TeamSortData, _b: &TeamSortData) -> Ordering {
    if rand::rng().random_bool(0.5) { Ordering::Less } else { Ordering::Greater }
}

// Master sort: apply all criteria in order c1..c7
pub fn sort_teams(teams: &mut Vec<TeamSortData>) {
    let snapshot: Vec<TeamSortData> = teams.clone();

    teams.sort_by(|a, b| {
        let mut ord = c1_points(a, b);
        if ord != Ordering::Equal { return ord; }

        ord = c2_head_to_head(a, b);
        if ord != Ordering::Equal { return ord; }

        ord = c3_buchholz(a, b, &snapshot);
        if ord != Ordering::Equal { return ord; }

        ord = c4_point_difference(a, b);
        if ord != Ordering::Equal { return ord; }

        ord = c5_points_scored(a, b);
        if ord != Ordering::Equal { return ord; }

        ord = c6_momentum_score(a, b);
        if ord != Ordering::Equal { return ord; }

        c7_rng(a, b)
    });
}

// Full pipeline: fetch data and return sorted standings for a division (completed matches only)
pub async fn get_sorted_standings(db: &SqlitePool, division: i64) -> Vec<TeamSortData> {
    let mut teams = fetch_sort_data(db, division).await;
    sort_teams(&mut teams);
    teams
}

// Intermediate rankings: includes live/in-progress match scores for display purposes.
// Uses current scores from ongoing games on top of completed results.
// NOT used for round generation — only for live standings display.
pub async fn get_intermediate_standings(db: &SqlitePool, division: i64) -> Vec<TeamSortData> {
    let mut teams = fetch_sort_data(db, division).await;

    // Fetch in-progress matches (possession 1 or 2 = live)
    let live: Vec<(i64, i64, i64, i64)> = sqlx::query_as(
        r#"SELECT m.t1_id, m.t2_id, m.t1_score, m.t2_score
           FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE t.division = ? AND m.possession IS NOT NULL AND m.possession < 3
             AND m.deleted_at IS NULL AND m.type < 1000"#
    ).bind(division).fetch_all(db).await.unwrap_or_default();

    // Build a lookup for quick access
    let mut team_map: HashMap<i64, usize> = HashMap::new();
    for (i, t) in teams.iter().enumerate() {
        team_map.insert(t.team_id, i);
    }

    // Layer live scores on top of completed data
    for (t1, t2, s1, s2) in &live {
        let (t1_res, t2_res): (i8, i8) = if s1 > s2 { (1, -1) } else if s1 < s2 { (-1, 1) } else { (0, 0) };

        if let Some(&idx) = team_map.get(t1) {
            teams[idx].points_for += s1;
            teams[idx].points_against += s2;
            match t1_res {
                1 => { teams[idx].wins += 1; teams[idx].points += 2; }
                0 => { teams[idx].draws += 1; teams[idx].points += 1; }
                _ => { teams[idx].losses += 1; }
            }
        }
        if let Some(&idx) = team_map.get(t2) {
            teams[idx].points_for += s2;
            teams[idx].points_against += s1;
            match t2_res {
                1 => { teams[idx].wins += 1; teams[idx].points += 2; }
                0 => { teams[idx].draws += 1; teams[idx].points += 1; }
                _ => { teams[idx].losses += 1; }
            }
        }
    }

    sort_teams(&mut teams);
    teams
}

// Cached intermediate standings with Redis-first lookup and DB fallback.
pub async fn get_cached_intermediate_standings(db: &SqlitePool, division: i64) -> Vec<TeamSortData> {
    if let Some(cached) = cache::get_intermediate_standings::<Vec<TeamSortData>>(division).await {
        return cached;
    }

    let standings = get_intermediate_standings(db, division).await;
    cache::set_intermediate_standings(division, &standings).await;
    standings
}

// Refresh intermediate standings cache after score/spirit writes.
pub async fn refresh_intermediate_standings_cache(db: &SqlitePool, division: i64) {
    let standings = get_intermediate_standings(db, division).await;
    cache::set_intermediate_standings(division, &standings).await;
}
