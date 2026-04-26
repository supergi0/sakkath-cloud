use super::tracker::TournamentTracker;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub(crate) struct SortMetrics {
    pub(crate) team_id: i64,
    pub(crate) init_rank: i64,
    pub(crate) wins: i64,
    pub(crate) losses: i64,
    pub(crate) draws: i64,
    pub(crate) points: i64,
    pub(crate) points_for: i64,
    pub(crate) points_against: i64,
    pub(crate) spirit_avg: f64,
    pub(crate) round_results: Vec<i8>,
    pub(crate) opponents: Vec<i64>,
    pub(crate) h2h: HashMap<i64, i8>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExpectedSwissRound {
    pub(crate) pairings: Vec<(i64, i64)>,
    pub(crate) naive_had_rematch: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ExpectedPlayoffMatch {
    pub(crate) team_a: i64,
    pub(crate) seed_a: i64,
    pub(crate) team_b: i64,
    pub(crate) seed_b: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct PairingScore {
    same_score_pairs: i64,
    cutline_adjacent_pairs: i64,
    cutline_companion_pairs: i64,
    natural_pairs: i64,
    reverse_gap_sum: i64,
}

struct PairingPreferences<'a> {
    points_by_team: &'a HashMap<i64, i64>,
    rank_by_team: &'a HashMap<i64, usize>,
    cutlines: &'a [usize],
}

pub(crate) fn sort_metrics(metrics: &mut [SortMetrics]) {
    let snapshot = metrics.to_vec();
    metrics.sort_by(|left, right| compare_metrics(left, right, &snapshot));
}

pub(crate) fn build_scoring_groups(sorted: &[SortMetrics]) -> Vec<Vec<i64>> {
    if sorted.is_empty() {
        return Vec::new();
    }

    let mut groups = Vec::new();
    let mut current_points = sorted[0].points;
    let mut current_group = Vec::new();

    for team in sorted {
        if team.points != current_points {
            groups.push(current_group);
            current_group = Vec::new();
            current_points = team.points;
        }
        current_group.push(team.team_id);
    }
    groups.push(current_group);

    fix_odd_groups(&mut groups);
    groups
        .into_iter()
        .filter(|group| !group.is_empty())
        .collect()
}

pub(crate) fn naive_pairings(groups: &[Vec<i64>]) -> Vec<(i64, i64)> {
    let mut pairings = Vec::new();
    for group in groups {
        let half = group.len() / 2;
        for index in 0..half {
            pairings.push((group[index], group[half + index]));
        }
    }
    pairings
}

pub(crate) fn generate_round_pairings(
    sorted: &[SortMetrics],
    history: &HashMap<i64, HashSet<i64>>,
    round: i64,
) -> Vec<(i64, i64)> {
    let mut groups = build_scoring_groups(sorted);
    let mut group_pairings: Vec<Option<Vec<(i64, i64)>>> = Vec::new();
    let mut failed_indices = Vec::new();
    let points_by_team: HashMap<i64, i64> = sorted
        .iter()
        .map(|metrics| (metrics.team_id, metrics.points))
        .collect();
    let rank_by_team: HashMap<i64, usize> = sorted
        .iter()
        .enumerate()
        .map(|(index, metrics)| (metrics.team_id, index + 1))
        .collect();
    let cutlines = build_cutlines(sorted.len());
    let preferences = PairingPreferences {
        points_by_team: &points_by_team,
        rank_by_team: &rank_by_team,
        cutlines: &cutlines,
    };

    for (index, group) in groups.iter().enumerate() {
        match pair_scoring_group_for_round(group, history, round, &preferences) {
            Some(pairings) => group_pairings.push(Some(pairings)),
            None => {
                group_pairings.push(None);
                failed_indices.push(index);
            }
        }
    }

    for failed_index in failed_indices {
        if try_rebalance_and_pair(
            &mut groups,
            &mut group_pairings,
            failed_index,
            history,
            round,
            &preferences,
        ) {
            continue;
        }
        group_pairings[failed_index] = Some(force_pairings(&groups[failed_index]));
    }

    group_pairings.into_iter().flatten().flatten().collect()
}

pub(crate) fn build_playoff_round_one(standings: &[SortMetrics]) -> Vec<ExpectedPlayoffMatch> {
    let mut expected = Vec::new();
    let mut offset = 0usize;

    while offset < standings.len() {
        let remaining = standings.len() - offset;
        let bracket_size = if remaining >= 4 { 4 } else { remaining };
        let bracket = &standings[offset..offset + bracket_size];

        if bracket_size == 4 {
            expected.push(ExpectedPlayoffMatch {
                team_a: bracket[0].team_id,
                seed_a: offset as i64 + 1,
                team_b: bracket[3].team_id,
                seed_b: offset as i64 + 4,
            });
            expected.push(ExpectedPlayoffMatch {
                team_a: bracket[1].team_id,
                seed_a: offset as i64 + 2,
                team_b: bracket[2].team_id,
                seed_b: offset as i64 + 3,
            });
        } else if bracket_size == 2 {
            expected.push(ExpectedPlayoffMatch {
                team_a: bracket[0].team_id,
                seed_a: offset as i64 + 1,
                team_b: bracket[1].team_id,
                seed_b: offset as i64 + 2,
            });
        }

        offset += bracket_size;
    }

    expected
}

pub(crate) fn build_playoff_round_two(
    round_one: &[ExpectedPlayoffMatch],
    tracker: &TournamentTracker,
) -> Vec<ExpectedPlayoffMatch> {
    let mut expected = Vec::new();
    let mut index = 0usize;

    while index < round_one.len() {
        if index + 1 >= round_one.len() {
            break;
        }

        let left = &round_one[index];
        let right = &round_one[index + 1];

        if (left.seed_b - left.seed_a).abs() != 3 || (right.seed_b - right.seed_a).abs() != 1 {
            index += 1;
            continue;
        }

        let mut seed_holders = HashMap::from([
            (left.seed_a, left.team_a),
            (right.seed_a, right.team_a),
            (right.seed_b, right.team_b),
            (left.seed_b, left.team_b),
        ]);

        apply_seed_swap(left, 1001, tracker, &mut seed_holders);
        apply_seed_swap(right, 1001, tracker, &mut seed_holders);

        expected.push(ExpectedPlayoffMatch {
            team_a: *seed_holders
                .get(&left.seed_a)
                .expect("missing seed one holder"),
            seed_a: left.seed_a,
            team_b: *seed_holders
                .get(&right.seed_a)
                .expect("missing seed two holder"),
            seed_b: right.seed_a,
        });
        expected.push(ExpectedPlayoffMatch {
            team_a: *seed_holders
                .get(&right.seed_b)
                .expect("missing seed three holder"),
            seed_a: right.seed_b,
            team_b: *seed_holders
                .get(&left.seed_b)
                .expect("missing seed four holder"),
            seed_b: left.seed_b,
        });

        index += 2;
    }

    expected
}

pub(crate) fn build_playoff_seed_order(
    round_one: &[ExpectedPlayoffMatch],
    tracker: &TournamentTracker,
) -> Vec<i64> {
    let mut ordered = Vec::new();
    let mut index = 0usize;

    while index < round_one.len() {
        if index + 1 < round_one.len()
            && is_four_team_bracket_start(&round_one[index], &round_one[index + 1])
        {
            let left = &round_one[index];
            let right = &round_one[index + 1];
            let mut seed_holders = HashMap::from([
                (left.seed_a, left.team_a),
                (right.seed_a, right.team_a),
                (right.seed_b, right.team_b),
                (left.seed_b, left.team_b),
            ]);

            apply_seed_swap(left, 1001, tracker, &mut seed_holders);
            apply_seed_swap(right, 1001, tracker, &mut seed_holders);

            ordered.push(*seed_holders.get(&left.seed_a).expect("missing seed holder"));
            ordered.push(
                *seed_holders
                    .get(&right.seed_a)
                    .expect("missing seed holder"),
            );
            ordered.push(
                *seed_holders
                    .get(&right.seed_b)
                    .expect("missing seed holder"),
            );
            ordered.push(*seed_holders.get(&left.seed_b).expect("missing seed holder"));
            index += 2;
            continue;
        }

        let match_expectation = &round_one[index];
        let mut seed_holders = HashMap::from([
            (match_expectation.seed_a, match_expectation.team_a),
            (match_expectation.seed_b, match_expectation.team_b),
        ]);
        apply_seed_swap(match_expectation, 1001, tracker, &mut seed_holders);
        ordered.push(
            *seed_holders
                .get(&match_expectation.seed_a)
                .expect("missing direct seed holder"),
        );
        ordered.push(
            *seed_holders
                .get(&match_expectation.seed_b)
                .expect("missing direct seed holder"),
        );
        index += 1;
    }

    ordered
}

pub(crate) fn build_final_seed_order(
    round_one: &[ExpectedPlayoffMatch],
    tracker: &TournamentTracker,
) -> Vec<i64> {
    let after_playoffs = build_playoff_seed_order(round_one, tracker);
    let round_two = build_playoff_round_two(round_one, tracker);
    let mut ordered = Vec::new();
    let mut round_one_index = 0usize;
    let mut round_two_index = 0usize;
    let mut seed_offset = 0usize;

    while round_one_index < round_one.len() {
        if round_one_index + 1 < round_one.len()
            && is_four_team_bracket_start(
                &round_one[round_one_index],
                &round_one[round_one_index + 1],
            )
        {
            let left = round_two
                .get(round_two_index)
                .expect("missing first finals expectation for bracket");
            let right = round_two
                .get(round_two_index + 1)
                .expect("missing second finals expectation for bracket");
            let mut seed_holders = HashMap::from([
                (left.seed_a, left.team_a),
                (left.seed_b, left.team_b),
                (right.seed_a, right.team_a),
                (right.seed_b, right.team_b),
            ]);

            apply_seed_swap(left, 1002, tracker, &mut seed_holders);
            apply_seed_swap(right, 1002, tracker, &mut seed_holders);

            ordered.push(*seed_holders.get(&left.seed_a).expect("missing seed holder"));
            ordered.push(*seed_holders.get(&left.seed_b).expect("missing seed holder"));
            ordered.push(
                *seed_holders
                    .get(&right.seed_a)
                    .expect("missing seed holder"),
            );
            ordered.push(
                *seed_holders
                    .get(&right.seed_b)
                    .expect("missing seed holder"),
            );

            round_one_index += 2;
            round_two_index += 2;
            seed_offset += 4;
            continue;
        }

        ordered.extend(after_playoffs[seed_offset..seed_offset + 2].iter().copied());
        round_one_index += 1;
        seed_offset += 2;
    }

    ordered
}

fn is_four_team_bracket_start(left: &ExpectedPlayoffMatch, right: &ExpectedPlayoffMatch) -> bool {
    (left.seed_b - left.seed_a).abs() == 3 && (right.seed_b - right.seed_a).abs() == 1
}

fn compare_metrics(left: &SortMetrics, right: &SortMetrics, all: &[SortMetrics]) -> Ordering {
    c1_points(left, right)
        .then_with(|| {
            if tied_team_count(left.points, all) == 2 {
                c2_head_to_head(left, right)
            } else {
                Ordering::Equal
            }
        })
        .then_with(|| c3_buchholz(left, right, all))
        .then_with(|| c4_point_difference(left, right))
        .then_with(|| c5_points_scored(left, right))
        .then_with(|| c6_momentum(left, right))
        .then_with(|| c7_seed(left, right))
}

fn c1_points(left: &SortMetrics, right: &SortMetrics) -> Ordering {
    right.points.cmp(&left.points)
}

fn c2_head_to_head(left: &SortMetrics, right: &SortMetrics) -> Ordering {
    match left.h2h.get(&right.team_id) {
        Some(1) => Ordering::Less,
        Some(-1) => Ordering::Greater,
        _ => Ordering::Equal,
    }
}

fn tied_team_count(points: i64, teams: &[SortMetrics]) -> usize {
    teams.iter().filter(|team| team.points == points).count()
}

fn c3_buchholz(left: &SortMetrics, right: &SortMetrics, all: &[SortMetrics]) -> Ordering {
    let points_map: HashMap<i64, i64> = all
        .iter()
        .map(|metrics| (metrics.team_id, metrics.points))
        .collect();
    let left_buchholz = median_buchholz_score(left, &points_map);
    let right_buchholz = median_buchholz_score(right, &points_map);
    right_buchholz.cmp(&left_buchholz)
}

fn median_buchholz_score(metrics: &SortMetrics, points_map: &HashMap<i64, i64>) -> i64 {
    let mut opponent_points: Vec<i64> = metrics
        .opponents
        .iter()
        .filter_map(|team_id| points_map.get(team_id).copied())
        .collect();

    opponent_points.sort_unstable();
    if opponent_points.len() > 2 {
        opponent_points[1..opponent_points.len() - 1].iter().sum()
    } else {
        opponent_points.iter().sum()
    }
}

fn c4_point_difference(left: &SortMetrics, right: &SortMetrics) -> Ordering {
    let left_diff = left.points_for - left.points_against;
    let right_diff = right.points_for - right.points_against;
    right_diff.cmp(&left_diff)
}

fn c5_points_scored(left: &SortMetrics, right: &SortMetrics) -> Ordering {
    right.points_for.cmp(&left.points_for)
}

fn c6_momentum(left: &SortMetrics, right: &SortMetrics) -> Ordering {
    fn momentum(results: &[i8]) -> i64 {
        let mut cumulative = 0;
        let mut total = 0;
        for result in results {
            match result {
                1 => cumulative += 2,
                0 => cumulative += 1,
                _ => {}
            }
            total += cumulative;
        }
        total
    }

    momentum(&right.round_results).cmp(&momentum(&left.round_results))
}

fn c7_seed(left: &SortMetrics, right: &SortMetrics) -> Ordering {
    let left_rank = if left.init_rank > 0 {
        left.init_rank
    } else {
        i64::MAX
    };
    let right_rank = if right.init_rank > 0 {
        right.init_rank
    } else {
        i64::MAX
    };
    left_rank
        .cmp(&right_rank)
        .then_with(|| left.team_id.cmp(&right.team_id))
}

fn fix_odd_groups(groups: &mut [Vec<i64>]) {
    for index in 0..groups.len() {
        if !groups[index].len().is_multiple_of(2) && index + 1 < groups.len() {
            let overflow = groups[index]
                .pop()
                .expect("odd group should have a trailing team");
            groups[index + 1].insert(0, overflow);
        }
    }
}

fn pair_scoring_group(
    team_ids: &[i64],
    history: &HashMap<i64, HashSet<i64>>,
) -> Option<Vec<(i64, i64)>> {
    if team_ids.len() < 2 {
        return Some(Vec::new());
    }

    let half = team_ids.len() / 2;
    let top_half = &team_ids[..half];
    let bottom_half = &team_ids[half..];
    let mut used_bottom = HashSet::new();
    let mut pairings = Vec::new();

    if backtrack_pairs(
        top_half,
        bottom_half,
        0,
        history,
        &mut used_bottom,
        &mut pairings,
    ) {
        Some(pairings)
    } else {
        None
    }
}

fn pair_scoring_group_crossover(
    team_ids: &[i64],
    history: &HashMap<i64, HashSet<i64>>,
    preferences: &PairingPreferences<'_>,
) -> Option<Vec<(i64, i64)>> {
    if team_ids.len() < 2 {
        return Some(Vec::new());
    }

    let half = team_ids.len() / 2;
    let top_half = &team_ids[..half];
    let bottom_half = &team_ids[half..];
    let mut used_bottom = HashSet::new();
    let mut pairings = Vec::new();
    let mut best_pairings: Option<Vec<(i64, i64)>> = None;
    let mut best_score: Option<PairingScore> = None;

    enumerate_crossover_pairings(
        top_half,
        bottom_half,
        0,
        history,
        &mut used_bottom,
        &mut pairings,
        0,
        0,
        preferences,
        &mut best_pairings,
        &mut best_score,
    );

    best_pairings
}

fn enumerate_crossover_pairings(
    top_half: &[i64],
    bottom_half: &[i64],
    index: usize,
    history: &HashMap<i64, HashSet<i64>>,
    used_bottom: &mut HashSet<usize>,
    pairings: &mut Vec<(i64, i64)>,
    natural_pairs: i64,
    gap_sum: i64,
    preferences: &PairingPreferences<'_>,
    best_pairings: &mut Option<Vec<(i64, i64)>>,
    best_score: &mut Option<PairingScore>,
) {
    if index == top_half.len() {
        let score = score_crossover_pairings(pairings, natural_pairs, gap_sum, preferences);
        if best_score.is_none_or(|current| score > current) {
            *best_score = Some(score);
            *best_pairings = Some(pairings.clone());
        }
        return;
    }

    let team = top_half[index];
    let mut candidate_indices: Vec<usize> = (0..bottom_half.len()).collect();
    if index < bottom_half.len() {
        candidate_indices.remove(index);
        candidate_indices.insert(0, index);
    }

    for candidate_index in candidate_indices {
        if used_bottom.contains(&candidate_index) {
            continue;
        }

        let opponent = bottom_half[candidate_index];
        let is_rematch = history
            .get(&team)
            .map(|opponents| opponents.contains(&opponent))
            .unwrap_or(false);
        if is_rematch {
            continue;
        }

        used_bottom.insert(candidate_index);
        pairings.push((team, opponent));

        enumerate_crossover_pairings(
            top_half,
            bottom_half,
            index + 1,
            history,
            used_bottom,
            pairings,
            natural_pairs + i64::from(candidate_index == index),
            gap_sum + (candidate_index as i64 - index as i64).abs(),
            preferences,
            best_pairings,
            best_score,
        );

        pairings.pop();
        used_bottom.remove(&candidate_index);
    }
}

fn score_crossover_pairings(
    pairings: &[(i64, i64)],
    natural_pairs: i64,
    gap_sum: i64,
    preferences: &PairingPreferences<'_>,
) -> PairingScore {
    let mut score = PairingScore {
        natural_pairs,
        reverse_gap_sum: -gap_sum,
        ..PairingScore::default()
    };

    for &(team_a, team_b) in pairings {
        if preferences.points_by_team.get(&team_a) == preferences.points_by_team.get(&team_b) {
            score.same_score_pairs += 1;
        }

        let left_rank = *preferences
            .rank_by_team
            .get(&team_a)
            .expect("missing left rank for pairing preference");
        let right_rank = *preferences
            .rank_by_team
            .get(&team_b)
            .expect("missing right rank for pairing preference");
        let (lower_rank, higher_rank) = if left_rank < right_rank {
            (left_rank, right_rank)
        } else {
            (right_rank, left_rank)
        };

        for &cutline in preferences.cutlines {
            if lower_rank == cutline && higher_rank == cutline + 1 {
                score.cutline_adjacent_pairs += 1;
            }
            if cutline > 1 && lower_rank == cutline - 1 && higher_rank == cutline + 2 {
                score.cutline_companion_pairs += 1;
            }
        }
    }

    score
}

fn should_use_crossover_pairing(round: i64) -> bool {
    (5..=6).contains(&round)
}

fn build_cutlines(team_count: usize) -> Vec<usize> {
    (4..team_count).step_by(4).collect()
}

fn pair_scoring_group_for_round(
    team_ids: &[i64],
    history: &HashMap<i64, HashSet<i64>>,
    round: i64,
    preferences: &PairingPreferences<'_>,
) -> Option<Vec<(i64, i64)>> {
    if should_use_crossover_pairing(round) {
        pair_scoring_group_crossover(team_ids, history, preferences)
    } else {
        pair_scoring_group(team_ids, history)
    }
}

fn backtrack_pairs(
    top_half: &[i64],
    bottom_half: &[i64],
    index: usize,
    history: &HashMap<i64, HashSet<i64>>,
    used_bottom: &mut HashSet<usize>,
    pairings: &mut Vec<(i64, i64)>,
) -> bool {
    if index == top_half.len() {
        return true;
    }

    let team = top_half[index];
    let mut candidate_indices: Vec<usize> = (0..bottom_half.len()).collect();
    if index < bottom_half.len() {
        candidate_indices.remove(index);
        candidate_indices.insert(0, index);
    }

    for candidate_index in candidate_indices {
        if used_bottom.contains(&candidate_index) {
            continue;
        }

        let opponent = bottom_half[candidate_index];
        let is_rematch = history
            .get(&team)
            .map(|opponents| opponents.contains(&opponent))
            .unwrap_or(false);
        if is_rematch {
            continue;
        }

        used_bottom.insert(candidate_index);
        pairings.push((team, opponent));

        if backtrack_pairs(
            top_half,
            bottom_half,
            index + 1,
            history,
            used_bottom,
            pairings,
        ) {
            return true;
        }

        pairings.pop();
        used_bottom.remove(&candidate_index);
    }

    false
}

fn try_rebalance_and_pair(
    groups: &mut [Vec<i64>],
    group_pairings: &mut [Option<Vec<(i64, i64)>>],
    failed_index: usize,
    history: &HashMap<i64, HashSet<i64>>,
    round: i64,
    preferences: &PairingPreferences<'_>,
) -> bool {
    if failed_index + 1 < groups.len()
        && !groups[failed_index].is_empty()
        && !groups[failed_index + 1].is_empty()
    {
        let failed_last_index = groups[failed_index].len() - 1;
        let last_team = groups[failed_index][failed_last_index];
        let next_first_team = groups[failed_index + 1][0];

        groups[failed_index][failed_last_index] = next_first_team;
        groups[failed_index + 1][0] = last_team;

        if let Some(failed_pairs) = pair_scoring_group_for_round(
            &groups[failed_index],
            history,
            round,
            preferences,
        ) && let Some(next_pairs) = pair_scoring_group_for_round(
            &groups[failed_index + 1],
            history,
            round,
            preferences,
        )
        {
            group_pairings[failed_index] = Some(failed_pairs);
            group_pairings[failed_index + 1] = Some(next_pairs);
            return true;
        }

        groups[failed_index + 1][0] = next_first_team;
        groups[failed_index][failed_last_index] = last_team;
    }

    if failed_index > 0 && !groups[failed_index].is_empty() && !groups[failed_index - 1].is_empty()
    {
        let previous_last_index = groups[failed_index - 1].len() - 1;
        let first_team = groups[failed_index][0];
        let previous_last_team = groups[failed_index - 1][previous_last_index];

        groups[failed_index][0] = previous_last_team;
        groups[failed_index - 1][previous_last_index] = first_team;

        if let Some(failed_pairs) = pair_scoring_group_for_round(
            &groups[failed_index],
            history,
            round,
            preferences,
        ) && let Some(previous_pairs) = pair_scoring_group_for_round(
            &groups[failed_index - 1],
            history,
            round,
            preferences,
        )
        {
            group_pairings[failed_index] = Some(failed_pairs);
            group_pairings[failed_index - 1] = Some(previous_pairs);
            return true;
        }

        groups[failed_index - 1][previous_last_index] = previous_last_team;
        groups[failed_index][0] = first_team;
    }

    false
}

fn force_pairings(team_ids: &[i64]) -> Vec<(i64, i64)> {
    let mut pairings = Vec::new();
    let mut index = 0;
    while index + 1 < team_ids.len() {
        pairings.push((team_ids[index], team_ids[index + 1]));
        index += 2;
    }
    pairings
}

fn apply_seed_swap(
    expected_match: &ExpectedPlayoffMatch,
    match_type: i64,
    tracker: &TournamentTracker,
    seed_holders: &mut HashMap<i64, i64>,
) {
    let actual = tracker
        .matches
        .values()
        .find(|state| {
            state.match_type == match_type
                && state.possession.unwrap_or(0) >= 3
                && state.t1_id == expected_match.team_a
                && state.t2_id == expected_match.team_b
        })
        .unwrap_or_else(|| {
            panic!(
                "missing completed playoff match for seeds {} vs {}",
                expected_match.seed_a, expected_match.seed_b
            )
        });

    let winner = if actual.t1_score > actual.t2_score {
        actual.t1_id
    } else {
        actual.t2_id
    };

    let lower_seed_team = if expected_match.seed_a > expected_match.seed_b {
        expected_match.team_a
    } else {
        expected_match.team_b
    };

    if winner == lower_seed_team {
        seed_holders.insert(expected_match.seed_a, expected_match.team_b);
        seed_holders.insert(expected_match.seed_b, expected_match.team_a);
    }
}
