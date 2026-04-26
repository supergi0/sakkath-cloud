use crate::helpers::{cache, rounds};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::cmp::Ordering;
use std::collections::HashMap;

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

async fn fetch_sort_data_with_round_limit(
    db: &SqlitePool,
    division: i64,
    max_round: Option<i64>,
) -> Vec<TeamSortData> {
    let teams: Vec<TeamIdentityRow> = sqlx::query_as(
        "SELECT id, name, abbreviation, small_logo, COALESCE(init_rank, 9999) FROM teams WHERE division = ? AND deleted_at IS NULL ORDER BY init_rank ASC"
    ).bind(division).fetch_all(db).await.unwrap_or_default();

    // All completed swiss matches for this division
    let matches: Vec<(i64, i64, i64, i64, i64)> = sqlx::query_as(
        r#"SELECT m.t1_id, m.t2_id, m.t1_score, m.t2_score, m.type
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

    // Process matches into team data
    for (t1, t2, s1, s2, _round) in &matches {
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
    }

    // Spirit averages (separate query to keep clean)
    let spirits: Vec<(i64, f64)> = sqlx::query_as(
        r#"SELECT team_id, AVG(spirit) FROM (
            SELECT m.t1_id as team_id, m.t1_spirit as spirit FROM matches m
                        JOIN teams t ON m.t1_id = t.id WHERE t.division = ? AND m.t1_spirit IS NOT NULL AND m.possession >= 3 AND m.deleted_at IS NULL AND m.type < 1000
                            AND (? IS NULL OR m.type <= ?)
            UNION ALL
            SELECT m.t2_id as team_id, m.t2_spirit as spirit FROM matches m
                        JOIN teams t ON m.t2_id = t.id WHERE t.division = ? AND m.t2_spirit IS NOT NULL AND m.possession >= 3 AND m.deleted_at IS NULL AND m.type < 1000
                            AND (? IS NULL OR m.type <= ?)
        ) GROUP BY team_id"#
        ).bind(division).bind(max_round).bind(max_round).bind(division).bind(max_round).bind(max_round).fetch_all(db).await.unwrap_or_default();

    for (tid, avg) in spirits {
        if let Some(d) = data_map.get_mut(&tid) {
            d.spirit_avg = avg;
        }
    }

    data_map.into_values().collect()
}

// Fetch all sort data for a division in minimal queries
pub async fn fetch_sort_data(db: &SqlitePool, division: i64) -> Vec<TeamSortData> {
    fetch_sort_data_with_round_limit(db, division, None).await
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

// C7: Stable final fallback using seed, then team id
pub fn c7_stable_seed(a: &TeamSortData, b: &TeamSortData) -> Ordering {
    let a_rank = if a.init_rank > 0 {
        a.init_rank
    } else {
        i64::MAX
    };
    let b_rank = if b.init_rank > 0 {
        b.init_rank
    } else {
        i64::MAX
    };

    a_rank.cmp(&b_rank).then_with(|| a.team_id.cmp(&b.team_id))
}

// Master sort: apply all criteria in order c1..c7
pub fn sort_teams(teams: &mut [TeamSortData]) {
    let snapshot: Vec<TeamSortData> = teams.to_vec();

    teams.sort_by(|a, b| {
        let mut ord = c1_points(a, b);
        if ord != Ordering::Equal {
            return ord;
        }

        if tied_team_count(a.points, &snapshot) == 2 {
            ord = c2_head_to_head(a, b);
            if ord != Ordering::Equal {
                return ord;
            }
        }

        ord = c3_buchholz(a, b, &snapshot);
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

        ord = c6_momentum_score(a, b);
        if ord != Ordering::Equal {
            return ord;
        }

        c7_stable_seed(a, b)
    });
}

fn tied_team_count(points: i64, teams: &[TeamSortData]) -> usize {
    teams.iter().filter(|team| team.points == points).count()
}

// Full pipeline: fetch data and return sorted standings for a division (completed matches only)
pub async fn get_sorted_standings(db: &SqlitePool, division: i64) -> Vec<TeamSortData> {
    let mut teams = fetch_sort_data(db, division).await;
    sort_teams(&mut teams);
    teams
}

pub async fn get_sorted_standings_through_round(
    db: &SqlitePool,
    division: i64,
    max_round: i64,
) -> Vec<TeamSortData> {
    let round_limit = Some(max_round.max(0));
    let mut teams = fetch_sort_data_with_round_limit(db, division, round_limit).await;
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
             AND m.deleted_at IS NULL AND m.type < 1000"#,
    )
    .bind(division)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    // Build a lookup for quick access
    let mut team_map: HashMap<i64, usize> = HashMap::new();
    for (i, t) in teams.iter().enumerate() {
        team_map.insert(t.team_id, i);
    }

    // Layer live scores on top of completed data
    for (t1, t2, s1, s2) in &live {
        let (t1_res, t2_res): (i8, i8) = if s1 > s2 {
            (1, -1)
        } else if s1 < s2 {
            (-1, 1)
        } else {
            (0, 0)
        };

        if let Some(&idx) = team_map.get(t1) {
            teams[idx].points_for += s1;
            teams[idx].points_against += s2;
            match t1_res {
                1 => {
                    teams[idx].wins += 1;
                    teams[idx].points += 2;
                }
                0 => {
                    teams[idx].draws += 1;
                    teams[idx].points += 1;
                }
                _ => {
                    teams[idx].losses += 1;
                }
            }
        }
        if let Some(&idx) = team_map.get(t2) {
            teams[idx].points_for += s2;
            teams[idx].points_against += s1;
            match t2_res {
                1 => {
                    teams[idx].wins += 1;
                    teams[idx].points += 2;
                }
                0 => {
                    teams[idx].draws += 1;
                    teams[idx].points += 1;
                }
                _ => {
                    teams[idx].losses += 1;
                }
            }
        }
    }

    sort_teams(&mut teams);
    teams
}

// Cached intermediate standings with Redis-first lookup and DB fallback.
pub async fn get_cached_intermediate_standings(
    db: &SqlitePool,
    division: i64,
) -> Vec<TeamSortData> {
    if let Some(cached) = cache::get_intermediate_standings::<Vec<TeamSortData>>(division).await {
        return cached;
    }

    let standings = get_intermediate_standings(db, division).await;
    cache::set_intermediate_standings(division, &standings).await;
    standings
}

async fn fetch_completed_stage_results(
    db: &SqlitePool,
    division: i64,
    match_type: i64,
) -> Vec<rounds::PlayedMatchResult> {
    let rows: Vec<(i64, i64, i64, i64)> = sqlx::query_as(
        r#"SELECT m.t1_id, m.t2_id, m.t1_score, m.t2_score
           FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE t.division = ? AND m.type = ? AND m.possession >= 3 AND m.deleted_at IS NULL
           ORDER BY m.time ASC, m.field_id ASC, m.id ASC"#,
    )
    .bind(division)
    .bind(match_type)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    rows.into_iter()
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
        .collect()
}

async fn fetch_display_stats(
    db: &SqlitePool,
    division: i64,
    include_live: bool,
) -> HashMap<i64, DisplayStats> {
    let matches: Vec<(i64, i64, i64, i64, Option<i64>, Option<i64>, Option<i64>)> =
        sqlx::query_as(
            r#"SELECT m.t1_id, m.t2_id, m.t1_score, m.t2_score, m.possession, m.t1_spirit, m.t2_spirit
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

    for (t1_id, t2_id, t1_score, t2_score, possession, t1_spirit, t2_spirit) in matches {
        let is_completed = matches!(possession, Some(value) if value >= 3);
        let is_live = include_live && matches!(possession, Some(value) if value < 3);
        if !is_completed && !is_live {
            continue;
        }

        apply_display_result(
            &mut stats_by_team,
            t1_id,
            t1_score,
            t2_score,
            is_completed,
            t1_spirit,
        );
        apply_display_result(
            &mut stats_by_team,
            t2_id,
            t2_score,
            t1_score,
            is_completed,
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
    is_completed: bool,
    spirit: Option<i64>,
) {
    let stats = stats_by_team.entry(team_id).or_default();
    stats.points_for += scored;
    stats.points_against += conceded;

    if scored > conceded {
        stats.wins += 1;
    } else if scored < conceded {
        stats.losses += 1;
    } else {
        stats.draws += 1;
    }

    if is_completed {
        if let Some(spirit) = spirit {
            stats.spirit_total += spirit;
            stats.spirit_count += 1;
        }
    }
}

fn apply_display_stats(
    mut teams: Vec<TeamSortData>,
    display_stats: &HashMap<i64, DisplayStats>,
) -> Vec<TeamSortData> {
    for team in &mut teams {
        let stats = display_stats.get(&team.team_id).cloned().unwrap_or_default();
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

fn reorder_teams_by_team_id(teams: Vec<TeamSortData>, ordered_team_ids: Vec<i64>) -> Vec<TeamSortData> {
    let mut teams_by_id: HashMap<i64, TeamSortData> = teams
        .into_iter()
        .map(|team| (team.team_id, team))
        .collect();

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
) -> Vec<TeamSortData> {
    if teams.is_empty() {
        return teams;
    }

    let playoff_results = fetch_completed_stage_results(db, division, 1001).await;
    let final_results = fetch_completed_stage_results(db, division, 1002).await;

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
    let ordered = apply_post_swiss_seed_order(db, division, teams).await;
    let display_stats = fetch_display_stats(db, division, false).await;
    apply_display_stats(ordered, &display_stats)
}

pub async fn get_display_intermediate_standings(
    db: &SqlitePool,
    division: i64,
) -> Vec<TeamSortData> {
    let teams = get_cached_intermediate_standings(db, division).await;
    let ordered = apply_post_swiss_seed_order(db, division, teams).await;
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

        let mut teams = vec![third, second, first];
        sort_teams(&mut teams);

        assert_eq!(teams[0].team_id, 1);
        assert_eq!(teams[1].team_id, 2);
        assert_eq!(teams[2].team_id, 3);
    }
}
