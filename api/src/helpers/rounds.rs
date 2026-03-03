use std::collections::{HashMap, HashSet};
use crate::helpers::sorting::TeamSortData;

// A single match pairing
#[derive(Clone, Debug)]
pub struct Pairing {
    pub t1: i64,
    pub t2: i64,
}

// Scoring group: teams with the same win count
#[derive(Clone, Debug)]
pub struct ScoringGroup {
    pub wins: i64,
    pub team_ids: Vec<i64>,
}

// Build scoring groups from sorted teams, ensuring all groups have even count.
// If a group has odd count, push its last team down to the next group.
pub fn build_scoring_groups(sorted_teams: &[TeamSortData]) -> Vec<ScoringGroup> {
    if sorted_teams.is_empty() { return Vec::new(); }

    // Group by wins
    let mut groups: Vec<ScoringGroup> = Vec::new();
    let mut current_wins = sorted_teams[0].wins;
    let mut current_ids: Vec<i64> = Vec::new();

    for t in sorted_teams {
        if t.wins != current_wins {
            groups.push(ScoringGroup { wins: current_wins, team_ids: current_ids });
            current_wins = t.wins;
            current_ids = Vec::new();
        }
        current_ids.push(t.team_id);
    }
    groups.push(ScoringGroup { wins: current_wins, team_ids: current_ids });

    // Fix odd-sized groups by pushing last team to next group
    fix_odd_groups(&mut groups);

    // Remove any empty groups created by pushing
    groups.retain(|g| !g.team_ids.is_empty());
    groups
}

// Push last team of odd group to next group until all are even
fn fix_odd_groups(groups: &mut Vec<ScoringGroup>) {
    let len = groups.len();
    for i in 0..len {
        if groups[i].team_ids.len() % 2 != 0 {
            if i + 1 < len {
                let overflow = groups[i].team_ids.pop().unwrap();
                groups[i + 1].team_ids.insert(0, overflow);
            }
            // last group with odd: total teams is even so this shouldn't happen
        }
    }
}

// Backtracking matchmaking within a scoring group.
// Pairs top-half vs bottom-half (1v(n/2+1), 2v(n/2+2)...).
// On rematch conflict, swaps with next available in same half.
// Returns None only if absolutely no valid pairing exists (triggers cross-group rebalance).
pub fn pair_scoring_group(
    team_ids: &[i64],
    history: &HashMap<i64, HashSet<i64>>,
) -> Option<Vec<Pairing>> {
    let n = team_ids.len();
    if n < 2 { return Some(Vec::new()); }

    let half = n / 2;
    let top: Vec<i64> = team_ids[..half].to_vec();
    let bottom: Vec<i64> = team_ids[half..].to_vec();

    // Try backtracking assignment
    let mut pairings: Vec<Pairing> = Vec::new();
    let mut used_bottom: HashSet<usize> = HashSet::new();

    if backtrack_pair(&top, &bottom, 0, history, &mut pairings, &mut used_bottom) {
        Some(pairings)
    } else {
        None
    }
}

// Recursive backtracking: assign each top[i] a partner from bottom
fn backtrack_pair(
    top: &[i64],
    bottom: &[i64],
    idx: usize,
    history: &HashMap<i64, HashSet<i64>>,
    pairings: &mut Vec<Pairing>,
    used: &mut HashSet<usize>,
) -> bool {
    if idx == top.len() {
        return true;
    }

    let t = top[idx];
    let played = history.get(&t);

    // Try each bottom team in order (prefer natural position first)
    let mut candidates: Vec<usize> = (0..bottom.len()).collect();
    // Prefer natural pairing position (idx) first
    if idx < bottom.len() {
        candidates.remove(candidates.iter().position(|&x| x == idx).unwrap());
        candidates.insert(0, idx);
    }

    for &bi in &candidates {
        if used.contains(&bi) { continue; }
        let opp = bottom[bi];
        let is_rematch = played.map(|h| h.contains(&opp)).unwrap_or(false);
        if is_rematch { continue; }

        used.insert(bi);
        pairings.push(Pairing { t1: t, t2: opp });

        if backtrack_pair(top, bottom, idx + 1, history, pairings, used) {
            return true;
        }

        pairings.pop();
        used.remove(&bi);
    }

    false
}

// Full round generation with cross-group rebalancing fallback.
// Steps:
// 1. Build scoring groups from sorted standings
// 2. Pair within each group using backtracking
// 3. If a group fails, try moving teams between adjacent groups and retry
pub fn generate_round_pairings(
    sorted_teams: &[TeamSortData],
    history: &HashMap<i64, HashSet<i64>>,
) -> Vec<Pairing> {
    let mut groups = build_scoring_groups(sorted_teams);
    let mut all_pairings: Vec<Pairing> = Vec::new();

    // First pass: try each group individually
    let mut failed_groups: Vec<usize> = Vec::new();
    let mut group_pairings: Vec<Option<Vec<Pairing>>> = Vec::new();

    for (i, group) in groups.iter().enumerate() {
        match pair_scoring_group(&group.team_ids, history) {
            Some(p) => group_pairings.push(Some(p)),
            None => {
                failed_groups.push(i);
                group_pairings.push(None);
            }
        }
    }

    // Second pass: rebalance failed groups by swapping teams with adjacent groups
    for &fi in &failed_groups {
        if try_rebalance_and_pair(&mut groups, &mut group_pairings, fi, history) {
            continue;
        }
        // Ultimate fallback: force pair allowing rematches (shouldn't happen for 7 rounds / 24 teams)
        let forced = force_pair(&groups[fi].team_ids);
        group_pairings[fi] = Some(forced);
    }

    for gp in group_pairings {
        if let Some(pairs) = gp {
            all_pairings.extend(pairs);
        }
    }

    all_pairings
}

// Try swapping last team of failed group with first of next, or first of failed with last of prev
fn try_rebalance_and_pair(
    groups: &mut Vec<ScoringGroup>,
    group_pairings: &mut Vec<Option<Vec<Pairing>>>,
    failed_idx: usize,
    history: &HashMap<i64, HashSet<i64>>,
) -> bool {
    let num_groups = groups.len();

    // Try swapping with next group
    if failed_idx + 1 < num_groups {
        let last = *groups[failed_idx].team_ids.last().unwrap();
        let first_next = groups[failed_idx + 1].team_ids[0];

        // Swap
        let pos = groups[failed_idx].team_ids.len() - 1;
        groups[failed_idx].team_ids[pos] = first_next;
        groups[failed_idx + 1].team_ids[0] = last;

        if let Some(p) = pair_scoring_group(&groups[failed_idx].team_ids, history) {
            // Re-pair the affected neighbor too
            if let Some(p2) = pair_scoring_group(&groups[failed_idx + 1].team_ids, history) {
                group_pairings[failed_idx] = Some(p);
                group_pairings[failed_idx + 1] = Some(p2);
                return true;
            }
        }

        // Undo swap
        groups[failed_idx + 1].team_ids[0] = first_next;
        groups[failed_idx].team_ids[pos] = last;
    }

    // Try swapping with previous group
    if failed_idx > 0 {
        let first = groups[failed_idx].team_ids[0];
        let prev_last_idx = groups[failed_idx - 1].team_ids.len() - 1;
        let prev_last = groups[failed_idx - 1].team_ids[prev_last_idx];

        groups[failed_idx].team_ids[0] = prev_last;
        groups[failed_idx - 1].team_ids[prev_last_idx] = first;

        if let Some(p) = pair_scoring_group(&groups[failed_idx].team_ids, history) {
            if let Some(p2) = pair_scoring_group(&groups[failed_idx - 1].team_ids, history) {
                group_pairings[failed_idx] = Some(p);
                group_pairings[failed_idx - 1] = Some(p2);
                return true;
            }
        }

        // Undo
        groups[failed_idx - 1].team_ids[prev_last_idx] = prev_last;
        groups[failed_idx].team_ids[0] = first;
    }

    false
}

// Force pair when all else fails (allows rematches)
fn force_pair(team_ids: &[i64]) -> Vec<Pairing> {
    let mut pairs = Vec::new();
    let mut i = 0;
    while i + 1 < team_ids.len() {
        pairs.push(Pairing { t1: team_ids[i], t2: team_ids[i + 1] });
        i += 2;
    }
    pairs
}

// Generate playoff bracket from final swiss standings.
// For groups of 4: semi-finals (1v4, 2v3), then finals from winners.
// For groups of 2: direct match (e.g. 9v10 for last placement).
// For larger groups: top vs bottom fold (5v8, 6v7), then winner/loser brackets.
pub fn generate_playoff_pairings(
    sorted_teams: &[TeamSortData],
    bracket_size: usize,
    offset: usize,
) -> Vec<PlayoffRound> {
    if offset + bracket_size > sorted_teams.len() { return Vec::new(); }
    let slice = &sorted_teams[offset..offset + bracket_size];
    let ids: Vec<i64> = slice.iter().map(|t| t.team_id).collect();

    match bracket_size {
        2 => {
            // Direct match
            vec![PlayoffRound {
                name: "placement".to_string(),
                matches: vec![Pairing { t1: ids[0], t2: ids[1] }],
            }]
        }
        4 => {
            // Semi-finals: 1v4, 2v3
            vec![PlayoffRound {
                name: "semi_finals".to_string(),
                matches: vec![
                    Pairing { t1: ids[0], t2: ids[3] },
                    Pairing { t1: ids[1], t2: ids[2] },
                ],
            }]
            // Finals generated after semis complete (dynamic)
        }
        _ => {
            // Fold pairing: 1v(n), 2v(n-1), etc. then winner/loser brackets
            let half = bracket_size / 2;
            let mut matches = Vec::new();
            for i in 0..half {
                matches.push(Pairing { t1: ids[i], t2: ids[bracket_size - 1 - i] });
            }
            vec![PlayoffRound {
                name: "round_of".to_string(),
                matches,
            }]
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlayoffRound {
    pub name: String,
    pub matches: Vec<Pairing>,
}

// Build full playoff structure for a division.
// Top 1-4: semis+finals. 5-8: placement bracket (5v8,6v7 -> winners play, losers play). 9-10: direct.
pub fn build_playoff_brackets(sorted_teams: &[TeamSortData]) -> Vec<PlayoffRound> {
    let n = sorted_teams.len();
    let mut rounds = Vec::new();

    // Top 4: semis
    if n >= 4 {
        rounds.extend(generate_playoff_pairings(sorted_teams, 4, 0));
    }

    // 5-8: placement bracket
    if n >= 8 {
        rounds.extend(generate_playoff_pairings(sorted_teams, 4, 4));
    }

    // Remaining pairs: 9-10, 11-12, etc.
    let mut i = 8;
    while i + 1 < n {
        let size = std::cmp::min(2, n - i);
        if size == 2 {
            rounds.extend(generate_playoff_pairings(sorted_teams, 2, i));
        }
        i += 2;
    }

    rounds
}

// Get match history map from DB matches
pub async fn fetch_match_history(db: &sqlx::SqlitePool, division: i64) -> HashMap<i64, HashSet<i64>> {
    let matches: Vec<(i64, i64)> = sqlx::query_as(
        r#"SELECT m.t1_id, m.t2_id FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE t.division = ? AND m.type < 1000 AND m.deleted_at IS NULL"#
    ).bind(division).fetch_all(db).await.unwrap_or_default();

    let mut history: HashMap<i64, HashSet<i64>> = HashMap::new();
    for (t1, t2) in matches {
        history.entry(t1).or_default().insert(t2);
        history.entry(t2).or_default().insert(t1);
    }
    history
}
