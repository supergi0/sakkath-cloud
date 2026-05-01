use crate::config::swiss_pairing;
use crate::helpers::sorting::TeamSortData;
use rand::{SeedableRng, rngs::StdRng};
use rand::RngCore;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

// A single match pairing
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pairing {
    pub t1: i64,
    pub t2: i64,
}

#[derive(Clone, Debug, Default)]
pub struct RoundPairingDiagnostics {
    pub point_gap_limit: Option<i64>,
    pub explored_candidates: usize,
    pub kept_candidates: usize,
    pub simulations_per_candidate: usize,
    pub total_simulations_run: usize,
    pub generation_time_ms: f64,
    pub exploration_time_ms: f64,
    pub simulation_time_ms: f64,
    pub chosen_same_point_cases: usize,
    pub chosen_same_point_met: usize,
    pub best_same_point_cases: usize,
    pub best_same_point_met: usize,
    pub average_kept_same_point_probability: Option<f64>,
    pub chosen_worst_case_same_point_miss_probability: Option<f64>,
    pub best_kept_worst_case_same_point_miss_probability: Option<f64>,
    pub average_kept_worst_case_same_point_miss_probability: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct RoundPairingResult {
    pub pairings: Vec<Pairing>,
    pub diagnostics: RoundPairingDiagnostics,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayedMatchResult {
    pub t1: i64,
    pub t2: i64,
    pub winner: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SamePointBoundaryStats {
    total_slots: usize,
    cases: usize,
    met: usize,
}

impl SamePointBoundaryStats {
    fn probability(self) -> Option<f64> {
        if self.cases == 0 {
            None
        } else {
            Some(self.met as f64 / self.cases as f64)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BoundaryOutcomeCounts {
    met: usize,
    missed: usize,
    split: usize,
}

impl BoundaryOutcomeCounts {
    fn total(self) -> usize {
        self.met + self.missed + self.split
    }

    fn miss_probability(self) -> Option<f64> {
        let total = self.total();
        if total == 0 {
            None
        } else {
            Some(self.missed as f64 / total as f64)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct LookaheadEvaluation {
    score: LookaheadScore,
    same_point_boundary_stats: SamePointBoundaryStats,
    boundary_outcomes: [BoundaryOutcomeCounts; 4],
}

impl LookaheadEvaluation {
    fn worst_case_same_point_miss_probability(self) -> Option<f64> {
        self.boundary_outcomes
            .into_iter()
            .filter_map(BoundaryOutcomeCounts::miss_probability)
            .max_by(|left, right| {
                left.partial_cmp(right)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct LookaheadScore {
    reverse_worst_same_point_boundary_missed: i64,
    reverse_same_point_boundary_missed: i64,
    same_point_boundary_met: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct RoundPairingScore {
    same_score_pairs: i64,
    reverse_point_gap_sum: i64,
    reverse_rank_gap_sum: i64,
    natural_pairs: i64,
    boundary_band_score: i64,
    lookahead: LookaheadScore,
}

#[derive(Clone, Debug, Default)]
struct RoundPairingCandidate {
    pairings: Vec<Pairing>,
    score: RoundPairingScore,
    lookahead_evaluation: LookaheadEvaluation,
}

#[derive(Clone, Debug, Default)]
struct RoundCandidateCollection {
    candidates: Vec<RoundPairingCandidate>,
    explored_candidates: usize,
}

#[derive(Clone, Copy, Debug)]
struct BoundaryBand {
    cutline: usize,
    start_rank: usize,
    end_rank: usize,
}

#[derive(Clone, Debug)]
struct BoundaryState {
    rank_by_team: HashMap<i64, usize>,
    boundary_bands: Vec<BoundaryBand>,
}

#[derive(Clone, Copy, Debug)]
struct RoundPairingSettings {
    band_radius: usize,
    band_weight: i64,
    lookahead_scenarios: usize,
    candidate_limit: usize,
    exploration_limit: usize,
}

fn round_pairing_settings(round: i64, allow_lookahead: bool) -> RoundPairingSettings {
    match round {
        4 => RoundPairingSettings {
            band_radius: swiss_pairing::ROUND4_BAND_RADIUS,
            band_weight: swiss_pairing::ROUND4_BAND_WEIGHT,
            lookahead_scenarios: swiss_pairing::DISABLED_LOOKAHEAD_SCENARIOS,
            candidate_limit: swiss_pairing::SINGLE_CANDIDATE_LIMIT,
            exploration_limit: swiss_pairing::SINGLE_EXPLORATION_LIMIT,
        },
        5 | 6 if allow_lookahead => RoundPairingSettings {
            band_radius: swiss_pairing::LATE_ROUND_BAND_RADIUS,
            band_weight: swiss_pairing::LATE_ROUND_BAND_WEIGHT,
            lookahead_scenarios: if round == 5 {
                swiss_pairing::ROUND5_LOOKAHEAD_SCENARIOS
            } else {
                swiss_pairing::ROUND6_LOOKAHEAD_SCENARIOS
            },
            candidate_limit: swiss_pairing::LOOKAHEAD_CANDIDATE_LIMIT,
            exploration_limit: swiss_pairing::LOOKAHEAD_EXPLORATION_LIMIT,
        },
        5 | 6 => RoundPairingSettings {
            band_radius: swiss_pairing::LATE_ROUND_BAND_RADIUS,
            band_weight: swiss_pairing::LATE_ROUND_BAND_WEIGHT,
            lookahead_scenarios: swiss_pairing::DISABLED_LOOKAHEAD_SCENARIOS,
            candidate_limit: swiss_pairing::SINGLE_CANDIDATE_LIMIT,
            exploration_limit: swiss_pairing::SINGLE_EXPLORATION_LIMIT,
        },
        _ => RoundPairingSettings {
            band_radius: swiss_pairing::DEFAULT_BAND_RADIUS,
            band_weight: swiss_pairing::DEFAULT_BAND_WEIGHT,
            lookahead_scenarios: swiss_pairing::DISABLED_LOOKAHEAD_SCENARIOS,
            candidate_limit: swiss_pairing::SINGLE_CANDIDATE_LIMIT,
            exploration_limit: swiss_pairing::SINGLE_EXPLORATION_LIMIT,
        },
    }
}

fn build_cutlines(team_count: usize) -> Vec<usize> {
    (4..team_count).step_by(4).collect()
}

fn build_boundary_state(
    sorted_teams: &[TeamSortData],
    settings: RoundPairingSettings,
) -> BoundaryState {
    let rank_by_team = sorted_teams
        .iter()
        .enumerate()
        .map(|(index, team)| (team.team_id, index + 1))
        .collect();
    let boundary_bands = build_cutlines(sorted_teams.len())
        .into_iter()
        .map(|cutline| BoundaryBand {
            cutline,
            start_rank: cutline.saturating_sub(settings.band_radius.saturating_sub(1)).max(1),
            end_rank: (cutline + settings.band_radius).min(sorted_teams.len()),
        })
        .collect();

    BoundaryState {
        rank_by_team,
        boundary_bands,
    }
}

fn generate_round_pairings_with_settings(
    sorted_teams: &[TeamSortData],
    history: &HashMap<i64, HashSet<i64>>,
    round: i64,
    settings: RoundPairingSettings,
) -> RoundPairingResult {
    if sorted_teams.is_empty() {
        return RoundPairingResult::default();
    }

    let generation_started_at = Instant::now();

    let boundary_state = build_boundary_state(sorted_teams, settings);
    let Some(point_gap_limit) = find_minimum_point_gap_limit(sorted_teams, history) else {
        return RoundPairingResult::default();
    };

    let candidate_collection = collect_round_pairing_candidates(
        sorted_teams,
        history,
        point_gap_limit,
        &boundary_state,
        settings,
    );
    let mut candidates = candidate_collection.candidates;

    let mut diagnostics = RoundPairingDiagnostics {
        point_gap_limit: Some(point_gap_limit),
        explored_candidates: candidate_collection.explored_candidates,
        kept_candidates: candidates.len(),
        simulations_per_candidate: settings.lookahead_scenarios,
        total_simulations_run: settings.lookahead_scenarios * candidates.len(),
        exploration_time_ms: generation_started_at.elapsed().as_secs_f64() * 1000.0,
        ..RoundPairingDiagnostics::default()
    };

    if settings.lookahead_scenarios > 0 {
        let simulation_started_at = Instant::now();
        for candidate in &mut candidates {
            candidate.lookahead_evaluation = evaluate_round_lookahead(
                sorted_teams,
                history,
                round,
                &candidate.pairings,
                settings.lookahead_scenarios,
            );
            candidate.score.lookahead = candidate.lookahead_evaluation.score;
        }
        candidates.sort_by(|left, right| compare_round_pairing_candidates(right, left));

        if let Some(chosen) = candidates.first() {
            diagnostics.chosen_same_point_cases =
                chosen.lookahead_evaluation.same_point_boundary_stats.cases;
            diagnostics.chosen_same_point_met =
                chosen.lookahead_evaluation.same_point_boundary_stats.met;
            diagnostics.chosen_worst_case_same_point_miss_probability = chosen
                .lookahead_evaluation
                .worst_case_same_point_miss_probability();
        }
        if let Some(best_candidate) = candidates.iter().max_by(|left, right| {
            compare_optional_probability(
                left.lookahead_evaluation.same_point_boundary_stats.probability(),
                right.lookahead_evaluation.same_point_boundary_stats.probability(),
            )
        }) {
            diagnostics.best_same_point_cases =
                best_candidate.lookahead_evaluation.same_point_boundary_stats.cases;
            diagnostics.best_same_point_met =
                best_candidate.lookahead_evaluation.same_point_boundary_stats.met;
        }

        let worst_case_samples: Vec<f64> = candidates
            .iter()
            .filter_map(|candidate| {
                candidate
                    .lookahead_evaluation
                    .worst_case_same_point_miss_probability()
            })
            .collect();
        diagnostics.best_kept_worst_case_same_point_miss_probability = worst_case_samples
            .iter()
            .copied()
            .min_by(|left, right| {
                left.partial_cmp(right)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        diagnostics.average_kept_worst_case_same_point_miss_probability =
            if worst_case_samples.is_empty() {
                None
            } else {
                Some(
                    worst_case_samples.iter().sum::<f64>() / worst_case_samples.len() as f64,
                )
            };

        let probability_samples: Vec<f64> = candidates
            .iter()
            .filter_map(|candidate| {
                candidate
                    .lookahead_evaluation
                    .same_point_boundary_stats
                    .probability()
            })
            .collect();
        diagnostics.average_kept_same_point_probability = if probability_samples.is_empty() {
            None
        } else {
            Some(probability_samples.iter().sum::<f64>() / probability_samples.len() as f64)
        };
        diagnostics.simulation_time_ms = simulation_started_at.elapsed().as_secs_f64() * 1000.0;
    }

    diagnostics.kept_candidates = candidates.len();
    diagnostics.total_simulations_run = settings.lookahead_scenarios * candidates.len();
    diagnostics.generation_time_ms = generation_started_at.elapsed().as_secs_f64() * 1000.0;

    RoundPairingResult {
        pairings: candidates
            .into_iter()
            .next()
            .map(|candidate| candidate.pairings)
            .unwrap_or_default(),
        diagnostics,
    }
}

// Full round generation with round-aware pairing strategy.
pub fn generate_round_pairings_with_diagnostics(
    sorted_teams: &[TeamSortData],
    history: &HashMap<i64, HashSet<i64>>,
    round: i64,
) -> RoundPairingResult {
    let settings = round_pairing_settings(round, true);
    generate_round_pairings_with_settings(sorted_teams, history, round, settings)
}

pub fn generate_round_pairings(
    sorted_teams: &[TeamSortData],
    history: &HashMap<i64, HashSet<i64>>,
    round: i64,
) -> Vec<Pairing> {
    generate_round_pairings_with_diagnostics(sorted_teams, history, round).pairings
}

fn find_minimum_point_gap_limit(
    sorted_teams: &[TeamSortData],
    history: &HashMap<i64, HashSet<i64>>,
) -> Option<i64> {
    let max_point_gap = sorted_teams
        .iter()
        .map(|team| team.points)
        .max()
        .unwrap_or(0);
    let min_point_gap = sorted_teams
        .iter()
        .map(|team| team.points)
        .min()
        .unwrap_or(0);
    let all_remaining = remaining_mask(sorted_teams.len());

    for point_gap_limit in 0..=(max_point_gap - min_point_gap) {
        let allowed_opponents = build_allowed_opponents(sorted_teams, history, point_gap_limit);
        let mut feasibility_cache = HashMap::new();
        if can_complete_pairing(all_remaining, &allowed_opponents, &mut feasibility_cache) {
            return Some(point_gap_limit);
        }
    }

    None
}

fn collect_round_pairing_candidates(
    sorted_teams: &[TeamSortData],
    history: &HashMap<i64, HashSet<i64>>,
    point_gap_limit: i64,
    boundary_state: &BoundaryState,
    settings: RoundPairingSettings,
) -> RoundCandidateCollection {
    let allowed_opponents = build_allowed_opponents(sorted_teams, history, point_gap_limit);
    let mut feasibility_cache = HashMap::new();
    let mut collection = RoundCandidateCollection::default();
    let mut current_pairings = Vec::new();

    collect_round_pairing_candidates_recursive(
        sorted_teams,
        &allowed_opponents,
        boundary_state,
        settings,
        remaining_mask(sorted_teams.len()),
        &mut feasibility_cache,
        &mut current_pairings,
        RoundPairingScore::default(),
        &mut collection,
    );

    collection
        .candidates
        .sort_by(|left, right| right.score.cmp(&left.score));
    collection
        .candidates
        .truncate(settings.candidate_limit.max(1));
    collection
}

fn collect_round_pairing_candidates_recursive(
    sorted_teams: &[TeamSortData],
    allowed_opponents: &[Vec<usize>],
    boundary_state: &BoundaryState,
    settings: RoundPairingSettings,
    remaining: u32,
    feasibility_cache: &mut HashMap<u32, bool>,
    current_pairings: &mut Vec<Pairing>,
    current_score: RoundPairingScore,
    collection: &mut RoundCandidateCollection,
) -> bool {
    if remaining == 0 {
        collection.explored_candidates += 1;
        collection.candidates.push(RoundPairingCandidate {
            pairings: current_pairings.clone(),
            score: current_score,
            lookahead_evaluation: LookaheadEvaluation::default(),
        });
        return collection.explored_candidates >= settings.exploration_limit.max(1);
    }

    let first_index = first_remaining_index(remaining);
    let ordered_opponents = ordered_remaining_opponents(
        first_index,
        remaining,
        sorted_teams,
        allowed_opponents,
        boundary_state,
        settings,
    );

    for opponent_index in ordered_opponents {
        let next_remaining = remove_pair_from_mask(remaining, first_index, opponent_index);
        if !can_complete_pairing(next_remaining, allowed_opponents, feasibility_cache) {
            continue;
        }

        let pairing = Pairing {
            t1: sorted_teams[first_index].team_id,
            t2: sorted_teams[opponent_index].team_id,
        };
        current_pairings.push(pairing.clone());
        if collect_round_pairing_candidates_recursive(
            sorted_teams,
            allowed_opponents,
            boundary_state,
            settings,
            next_remaining,
            feasibility_cache,
            current_pairings,
            combine_round_pairing_scores(
                current_score,
                score_pairing(&pairing, sorted_teams, boundary_state, settings),
            ),
            collection,
        ) {
            current_pairings.pop();
            return true;
        }
        current_pairings.pop();
    }

    false
}

fn build_allowed_opponents(
    sorted_teams: &[TeamSortData],
    history: &HashMap<i64, HashSet<i64>>,
    point_gap_limit: i64,
) -> Vec<Vec<usize>> {
    sorted_teams
        .iter()
        .enumerate()
        .map(|(left_index, left_team)| {
            sorted_teams
                .iter()
                .enumerate()
                .filter_map(|(right_index, right_team)| {
                    if left_index == right_index {
                        return None;
                    }

                    let already_played = history
                        .get(&left_team.team_id)
                        .map(|opponents| opponents.contains(&right_team.team_id))
                        .unwrap_or(false);
                    if already_played {
                        return None;
                    }

                    let point_gap = (left_team.points - right_team.points).abs();
                    if point_gap > point_gap_limit {
                        return None;
                    }

                    Some(right_index)
                })
                .collect()
        })
        .collect()
}

fn can_complete_pairing(
    remaining: u32,
    allowed_opponents: &[Vec<usize>],
    feasibility_cache: &mut HashMap<u32, bool>,
) -> bool {
    if remaining == 0 {
        return true;
    }
    if let Some(can_complete) = feasibility_cache.get(&remaining) {
        return *can_complete;
    }

    let first_index = first_remaining_index(remaining);
    let mut can_complete = false;

    for &opponent_index in &allowed_opponents[first_index] {
        if !mask_contains(remaining, opponent_index) {
            continue;
        }

        if can_complete_pairing(
            remove_pair_from_mask(remaining, first_index, opponent_index),
            allowed_opponents,
            feasibility_cache,
        ) {
            can_complete = true;
            break;
        }
    }

    feasibility_cache.insert(remaining, can_complete);
    can_complete
}

fn ordered_remaining_opponents(
    first_index: usize,
    remaining: u32,
    sorted_teams: &[TeamSortData],
    allowed_opponents: &[Vec<usize>],
    boundary_state: &BoundaryState,
    settings: RoundPairingSettings,
) -> Vec<usize> {
    let mut opponents: Vec<usize> = allowed_opponents[first_index]
        .iter()
        .copied()
        .filter(|opponent_index| mask_contains(remaining, *opponent_index))
        .collect();

    opponents.sort_by(|left, right| {
        let left_pairing = Pairing {
            t1: sorted_teams[first_index].team_id,
            t2: sorted_teams[*left].team_id,
        };
        let right_pairing = Pairing {
            t1: sorted_teams[first_index].team_id,
            t2: sorted_teams[*right].team_id,
        };
        score_pairing(&right_pairing, sorted_teams, boundary_state, settings).cmp(
            &score_pairing(&left_pairing, sorted_teams, boundary_state, settings),
        )
    });

    opponents
}

fn score_pairing(
    pairing: &Pairing,
    sorted_teams: &[TeamSortData],
    boundary_state: &BoundaryState,
    settings: RoundPairingSettings,
) -> RoundPairingScore {
    let left_rank = *boundary_state
        .rank_by_team
        .get(&pairing.t1)
        .expect("missing left rank for Swiss pairing score");
    let right_rank = *boundary_state
        .rank_by_team
        .get(&pairing.t2)
        .expect("missing right rank for Swiss pairing score");
    let left_team = &sorted_teams[left_rank - 1];
    let right_team = &sorted_teams[right_rank - 1];
    let point_gap = (left_team.points - right_team.points).abs();
    let rank_gap = left_rank.abs_diff(right_rank) as i64;

    RoundPairingScore {
        same_score_pairs: i64::from(point_gap == 0),
        reverse_point_gap_sum: -point_gap,
        reverse_rank_gap_sum: -rank_gap,
        natural_pairs: i64::from(rank_gap == 1),
        boundary_band_score: boundary_band_pair_score(pairing, boundary_state) * settings.band_weight,
        lookahead: LookaheadScore::default(),
    }
}

fn combine_round_pairing_scores(
    left: RoundPairingScore,
    right: RoundPairingScore,
) -> RoundPairingScore {
    RoundPairingScore {
        same_score_pairs: left.same_score_pairs + right.same_score_pairs,
        reverse_point_gap_sum: left.reverse_point_gap_sum + right.reverse_point_gap_sum,
        reverse_rank_gap_sum: left.reverse_rank_gap_sum + right.reverse_rank_gap_sum,
        natural_pairs: left.natural_pairs + right.natural_pairs,
        boundary_band_score: left.boundary_band_score + right.boundary_band_score,
        lookahead: left.lookahead,
    }
}

fn compare_round_pairing_candidates(
    left: &RoundPairingCandidate,
    right: &RoundPairingCandidate,
) -> std::cmp::Ordering {
    left.score
        .lookahead
        .cmp(&right.score.lookahead)
        .then_with(|| left.score.same_score_pairs.cmp(&right.score.same_score_pairs))
        .then_with(|| left.score.reverse_point_gap_sum.cmp(&right.score.reverse_point_gap_sum))
        .then_with(|| left.score.reverse_rank_gap_sum.cmp(&right.score.reverse_rank_gap_sum))
        .then_with(|| left.score.natural_pairs.cmp(&right.score.natural_pairs))
        .then_with(|| left.score.boundary_band_score.cmp(&right.score.boundary_band_score))
}

fn remaining_mask(team_count: usize) -> u32 {
    if team_count == 32 {
        u32::MAX
    } else {
        (1u32 << team_count) - 1
    }
}

fn first_remaining_index(remaining: u32) -> usize {
    remaining.trailing_zeros() as usize
}

fn mask_contains(remaining: u32, index: usize) -> bool {
    remaining & (1u32 << index) != 0
}

fn remove_pair_from_mask(remaining: u32, left_index: usize, right_index: usize) -> u32 {
    remaining & !(1u32 << left_index) & !(1u32 << right_index)
}

fn boundary_band_pair_score(pairing: &Pairing, boundary_state: &BoundaryState) -> i64 {
    let left_rank = *boundary_state
        .rank_by_team
        .get(&pairing.t1)
        .expect("missing left rank for boundary band score");
    let right_rank = *boundary_state
        .rank_by_team
        .get(&pairing.t2)
        .expect("missing right rank for boundary band score");
    let (upper_rank, lower_rank) = if left_rank < right_rank {
        (left_rank, right_rank)
    } else {
        (right_rank, left_rank)
    };

    boundary_state
        .boundary_bands
        .iter()
        .filter_map(|band| {
            if upper_rank < band.start_rank || lower_rank > band.end_rank {
                return None;
            }
            if !(upper_rank <= band.cutline && lower_rank > band.cutline) {
                return None;
            }

            let top_span = (band.cutline - band.start_rank + 1) as i64;
            let bottom_span = (band.end_rank - band.cutline) as i64;
            let top_score = top_span - (band.cutline - upper_rank) as i64;
            let bottom_score = bottom_span - (lower_rank - (band.cutline + 1)) as i64;
            Some(top_score + bottom_score)
        })
        .max()
        .unwrap_or(0)
}

fn compare_optional_probability(left: Option<f64>, right: Option<f64>) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left
            .partial_cmp(&right)
            .unwrap_or(std::cmp::Ordering::Equal),
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn evaluate_round_lookahead(
    sorted_teams: &[TeamSortData],
    history: &HashMap<i64, HashSet<i64>>,
    round: i64,
    pairings: &[Pairing],
    scenario_count: usize,
) -> LookaheadEvaluation {
    let mut total_evaluation = LookaheadEvaluation::default();

    for scenario_index in 0..scenario_count.max(1) {
        let mut simulated_teams = sorted_teams.to_vec();
        let mut simulated_history = history.clone();
        apply_simulated_round(
            &mut simulated_teams,
            &mut simulated_history,
            round,
            pairings,
            scenario_index as u64,
        );

        for future_round in (round + 1)..=6 {
            let mut future_sorted = simulated_teams.clone();
            crate::helpers::sorting::sort_teams(&mut future_sorted);
            let future_settings = round_pairing_settings(future_round, false);
            let future_pairings = generate_round_pairings_with_settings(
                &future_sorted,
                &simulated_history,
                future_round,
                future_settings,
            )
            .pairings;
            apply_simulated_round(
                &mut simulated_teams,
                &mut simulated_history,
                future_round,
                &future_pairings,
                scenario_index as u64,
            );
        }

        let mut final_sorted = simulated_teams.clone();
        crate::helpers::sorting::sort_teams(&mut final_sorted);
        let scenario_score = score_final_boundary_meetings(&final_sorted, &simulated_history);
        total_evaluation.same_point_boundary_stats.total_slots +=
            scenario_score.same_point_boundary_stats.total_slots;
        total_evaluation.same_point_boundary_stats.cases +=
            scenario_score.same_point_boundary_stats.cases;
        total_evaluation.same_point_boundary_stats.met +=
            scenario_score.same_point_boundary_stats.met;
        for (index, boundary_outcome) in scenario_score.boundary_outcomes.iter().enumerate() {
            total_evaluation.boundary_outcomes[index].met += boundary_outcome.met;
            total_evaluation.boundary_outcomes[index].missed += boundary_outcome.missed;
            total_evaluation.boundary_outcomes[index].split += boundary_outcome.split;
        }
    }

    total_evaluation.score = score_lookahead_evaluation(&total_evaluation);

    total_evaluation
}

fn score_lookahead_evaluation(evaluation: &LookaheadEvaluation) -> LookaheadScore {
    let worst_missed = evaluation
        .boundary_outcomes
        .iter()
        .filter(|outcome| outcome.total() > 0)
        .map(|outcome| outcome.missed as i64)
        .max()
        .unwrap_or(0);
    let total_missed: i64 = evaluation
        .boundary_outcomes
        .iter()
        .map(|outcome| outcome.missed as i64)
        .sum::<i64>();
    let total_met: i64 = evaluation
        .boundary_outcomes
        .iter()
        .map(|outcome| outcome.met as i64)
        .sum::<i64>();

    LookaheadScore {
        reverse_worst_same_point_boundary_missed: -worst_missed,
        reverse_same_point_boundary_missed: -total_missed,
        same_point_boundary_met: total_met,
    }
}

fn apply_simulated_round(
    teams: &mut [TeamSortData],
    history: &mut HashMap<i64, HashSet<i64>>,
    round: i64,
    pairings: &[Pairing],
    scenario_index: u64,
) {
    let mut current_sorted = teams.to_vec();
    crate::helpers::sorting::sort_teams(&mut current_sorted);
    let rank_by_team: HashMap<i64, usize> = current_sorted
        .iter()
        .enumerate()
        .map(|(index, team)| (team.team_id, index + 1))
        .collect();
    let team_index: HashMap<i64, usize> = teams
        .iter()
        .enumerate()
        .map(|(index, team)| (team.team_id, index))
        .collect();

    for pairing in pairings {
        let result = forecast_match_result(pairing, &rank_by_team, round, scenario_index);
        let left_index = *team_index
            .get(&pairing.t1)
            .expect("missing left team for simulated round");
        let right_index = *team_index
            .get(&pairing.t2)
            .expect("missing right team for simulated round");

        if left_index < right_index {
            let (left_slice, right_slice) = teams.split_at_mut(right_index);
            let left_team = &mut left_slice[left_index];
            let right_team = &mut right_slice[0];
            apply_forecast_result(left_team, right_team, result);
        } else {
            let (left_slice, right_slice) = teams.split_at_mut(left_index);
            let right_team = &mut left_slice[right_index];
            let left_team = &mut right_slice[0];
            apply_forecast_result(left_team, right_team, result);
        }

        history.entry(pairing.t1).or_default().insert(pairing.t2);
        history.entry(pairing.t2).or_default().insert(pairing.t1);
    }
}

#[derive(Clone, Copy)]
enum ForecastResult {
    T1Win,
    T2Win,
    Draw,
}

fn forecast_match_result(
    pairing: &Pairing,
    rank_by_team: &HashMap<i64, usize>,
    round: i64,
    scenario_index: u64,
) -> ForecastResult {
    let t1_rank = *rank_by_team
        .get(&pairing.t1)
        .expect("missing t1 rank for forecast");
    let t2_rank = *rank_by_team
        .get(&pairing.t2)
        .expect("missing t2 rank for forecast");
    let favorite_is_t1 = t1_rank <= t2_rank;
    let gap = t1_rank.abs_diff(t2_rank).min(4) as u8;
    let favorite_percent = 56 + gap * 7 + if round >= 4 { 4 } else { 0 };
    let mut rng = StdRng::seed_from_u64(forecast_seed(pairing, round, scenario_index));
    let draw_roll = (rng.next_u64() % 100) as u8;
    if draw_roll < 20 {
        return ForecastResult::Draw;
    }

    let winner_roll = (rng.next_u64() % 100) as u8;
    if favorite_is_t1 {
        if winner_roll < favorite_percent {
            ForecastResult::T1Win
        } else {
            ForecastResult::T2Win
        }
    } else if winner_roll < favorite_percent {
        ForecastResult::T2Win
    } else {
        ForecastResult::T1Win
    }
}

fn forecast_seed(pairing: &Pairing, round: i64, scenario_index: u64) -> u64 {
    let left = pairing.t1.min(pairing.t2) as u64;
    let right = pairing.t1.max(pairing.t2) as u64;
    0x9E37_79B9_7F4A_7C15_u64
        ^ ((round as u64) << 48)
        ^ (scenario_index.wrapping_mul(0xA24B_AED4_963E_E407))
        ^ left.rotate_left(13)
        ^ right.rotate_left(29)
}

fn apply_forecast_result(
    t1: &mut TeamSortData,
    t2: &mut TeamSortData,
    result: ForecastResult,
) {
    let (t1_score, t2_score, t1_round_result, t2_round_result) = match result {
        ForecastResult::T1Win => (2, 1, 1, -1),
        ForecastResult::T2Win => (1, 2, -1, 1),
        ForecastResult::Draw => (1, 1, 0, 0),
    };

    t1.points_for += t1_score;
    t1.points_against += t2_score;
    t2.points_for += t2_score;
    t2.points_against += t1_score;
    t1.opponents.push(t2.team_id);
    t2.opponents.push(t1.team_id);
    t1.round_results.push(t1_round_result);
    t2.round_results.push(t2_round_result);
    t1.h2h.insert(t2.team_id, t1_round_result);
    t2.h2h.insert(t1.team_id, t2_round_result);

    match result {
        ForecastResult::T1Win => {
            t1.wins += 1;
            t1.points += 2;
            t2.losses += 1;
        }
        ForecastResult::T2Win => {
            t2.wins += 1;
            t2.points += 2;
            t1.losses += 1;
        }
        ForecastResult::Draw => {
            t1.draws += 1;
            t1.points += 1;
            t2.draws += 1;
            t2.points += 1;
        }
    }
}

fn score_final_boundary_meetings(
    final_sorted: &[TeamSortData],
    history: &HashMap<i64, HashSet<i64>>,
) -> LookaheadEvaluation {
    let mut evaluation = LookaheadEvaluation::default();

    for (index, (left_rank, right_rank)) in swiss_pairing::LOOKAHEAD_BOUNDARY_TARGETS.iter().enumerate() {
        if *right_rank > final_sorted.len() {
            continue;
        }

        let left_team = final_sorted[*left_rank - 1].team_id;
        let right_team = final_sorted[*right_rank - 1].team_id;
        let same_points = final_sorted[*left_rank - 1].points == final_sorted[*right_rank - 1].points;
        let met = teams_have_played(history, left_team, right_team);

        evaluation.same_point_boundary_stats.total_slots += 1;
        if same_points {
            evaluation.same_point_boundary_stats.cases += 1;
            if met {
                evaluation.same_point_boundary_stats.met += 1;
                evaluation.boundary_outcomes[index].met += 1;
            } else {
                evaluation.boundary_outcomes[index].missed += 1;
            }
        } else {
            evaluation.boundary_outcomes[index].split += 1;
        }
    }

    evaluation.score = score_lookahead_evaluation(&evaluation);

    evaluation
}

fn teams_have_played(
    history: &HashMap<i64, HashSet<i64>>,
    left_team: i64,
    right_team: i64,
) -> bool {
    history
        .get(&left_team)
        .map(|opponents| opponents.contains(&right_team))
        .unwrap_or(false)
}

// Generate seed-based placement rounds from final swiss standings.
// For groups of 4: first round is 1v4 and 2v3, second round is 1v2 and 3v4.
// For groups of 2: direct match only.
pub fn generate_playoff_pairings(
    sorted_teams: &[TeamSortData],
    bracket_size: usize,
    offset: usize,
) -> Vec<PlayoffRound> {
    if offset + bracket_size > sorted_teams.len() {
        return Vec::new();
    }
    let slice = &sorted_teams[offset..offset + bracket_size];
    let ids: Vec<i64> = slice.iter().map(|t| t.team_id).collect();

    match bracket_size {
        2 => {
            vec![PlayoffRound {
                name: "playoffs".to_string(),
                matches: vec![Pairing {
                    t1: ids[0],
                    t2: ids[1],
                }],
            }]
        }
        4 => {
            vec![
                PlayoffRound {
                    name: "playoffs".to_string(),
                    matches: vec![
                        Pairing {
                            t1: ids[0],
                            t2: ids[3],
                        },
                        Pairing {
                            t1: ids[1],
                            t2: ids[2],
                        },
                    ],
                },
                PlayoffRound {
                    name: "finals".to_string(),
                    matches: vec![
                        Pairing {
                            t1: ids[0],
                            t2: ids[1],
                        },
                        Pairing {
                            t1: ids[2],
                            t2: ids[3],
                        },
                    ],
                },
            ]
        }
        _ => Vec::new(),
    }
}

fn apply_seed_swap_for_result(
    bracket_seed_order: &mut [i64],
    higher_seed_team: i64,
    lower_seed_team: i64,
    results: &HashMap<(i64, i64), i64>,
) {
    let key = if higher_seed_team < lower_seed_team {
        (higher_seed_team, lower_seed_team)
    } else {
        (lower_seed_team, higher_seed_team)
    };

    if results.get(&key) != Some(&lower_seed_team) {
        return;
    }

    let Some(higher_index) = bracket_seed_order
        .iter()
        .position(|team_id| *team_id == higher_seed_team)
    else {
        return;
    };
    let Some(lower_index) = bracket_seed_order
        .iter()
        .position(|team_id| *team_id == lower_seed_team)
    else {
        return;
    };

    bracket_seed_order.swap(higher_index, lower_index);
}

pub fn build_seed_order_after_playoffs(
    sorted_teams: &[TeamSortData],
    playoff_results: &[PlayedMatchResult],
) -> Vec<i64> {
    let result_lookup: HashMap<(i64, i64), i64> = playoff_results
        .iter()
        .map(|result| {
            let key = if result.t1 < result.t2 {
                (result.t1, result.t2)
            } else {
                (result.t2, result.t1)
            };
            (key, result.winner)
        })
        .collect();

    let mut seed_order = Vec::with_capacity(sorted_teams.len());
    let mut offset = 0;
    while offset < sorted_teams.len() {
        let remaining = sorted_teams.len() - offset;
        if remaining >= 4 {
            let mut bracket_seed_order: Vec<i64> = sorted_teams[offset..offset + 4]
                .iter()
                .map(|team| team.team_id)
                .collect();
            let first_pair = (bracket_seed_order[0], bracket_seed_order[3]);
            let second_pair = (bracket_seed_order[1], bracket_seed_order[2]);

            apply_seed_swap_for_result(
                &mut bracket_seed_order,
                first_pair.0,
                first_pair.1,
                &result_lookup,
            );
            apply_seed_swap_for_result(
                &mut bracket_seed_order,
                second_pair.0,
                second_pair.1,
                &result_lookup,
            );

            seed_order.extend(bracket_seed_order);
            offset += 4;
            continue;
        }

        if remaining >= 2 {
            seed_order.extend(
                sorted_teams[offset..offset + 2]
                    .iter()
                    .map(|team| team.team_id),
            );
            offset += 2;
            continue;
        }

        seed_order.push(sorted_teams[offset].team_id);
        offset += 1;
    }

    seed_order
}

pub fn build_seed_order_after_elimination_results(
    sorted_teams: &[TeamSortData],
    playoff_results: &[PlayedMatchResult],
    final_results: &[PlayedMatchResult],
) -> Vec<i64> {
    let playoff_lookup: HashMap<(i64, i64), i64> = playoff_results
        .iter()
        .map(|result| {
            let key = if result.t1 < result.t2 {
                (result.t1, result.t2)
            } else {
                (result.t2, result.t1)
            };
            (key, result.winner)
        })
        .collect();
    let final_lookup: HashMap<(i64, i64), i64> = final_results
        .iter()
        .map(|result| {
            let key = if result.t1 < result.t2 {
                (result.t1, result.t2)
            } else {
                (result.t2, result.t1)
            };
            (key, result.winner)
        })
        .collect();

    let mut seed_order = Vec::with_capacity(sorted_teams.len());
    let mut offset = 0;
    while offset < sorted_teams.len() {
        let remaining = sorted_teams.len() - offset;
        if remaining >= 4 {
            let mut bracket_seed_order: Vec<i64> = sorted_teams[offset..offset + 4]
                .iter()
                .map(|team| team.team_id)
                .collect();
            let first_playoff_pair = (bracket_seed_order[0], bracket_seed_order[3]);
            let second_playoff_pair = (bracket_seed_order[1], bracket_seed_order[2]);

            apply_seed_swap_for_result(
                &mut bracket_seed_order,
                first_playoff_pair.0,
                first_playoff_pair.1,
                &playoff_lookup,
            );
            apply_seed_swap_for_result(
                &mut bracket_seed_order,
                second_playoff_pair.0,
                second_playoff_pair.1,
                &playoff_lookup,
            );

            let first_final_pair = (bracket_seed_order[0], bracket_seed_order[1]);
            let second_final_pair = (bracket_seed_order[2], bracket_seed_order[3]);

            apply_seed_swap_for_result(
                &mut bracket_seed_order,
                first_final_pair.0,
                first_final_pair.1,
                &final_lookup,
            );
            apply_seed_swap_for_result(
                &mut bracket_seed_order,
                second_final_pair.0,
                second_final_pair.1,
                &final_lookup,
            );

            seed_order.extend(bracket_seed_order);
            offset += 4;
            continue;
        }

        if remaining >= 2 {
            let mut bracket_seed_order: Vec<i64> = sorted_teams[offset..offset + 2]
                .iter()
                .map(|team| team.team_id)
                .collect();
            let direct_pair = (bracket_seed_order[0], bracket_seed_order[1]);
            apply_seed_swap_for_result(
                &mut bracket_seed_order,
                direct_pair.0,
                direct_pair.1,
                &playoff_lookup,
            );
            apply_seed_swap_for_result(
                &mut bracket_seed_order,
                direct_pair.0,
                direct_pair.1,
                &final_lookup,
            );
            seed_order.extend(bracket_seed_order);
            offset += 2;
            continue;
        }

        seed_order.push(sorted_teams[offset].team_id);
        offset += 1;
    }

    seed_order
}

pub fn build_final_pairings_from_playoff_results(
    sorted_teams: &[TeamSortData],
    playoff_results: &[PlayedMatchResult],
) -> Vec<Pairing> {
    let seed_order = build_seed_order_after_playoffs(sorted_teams, playoff_results);
    let mut finals = Vec::new();
    let mut offset = 0;

    while offset + 3 < seed_order.len() {
        finals.push(Pairing {
            t1: seed_order[offset],
            t2: seed_order[offset + 1],
        });
        finals.push(Pairing {
            t1: seed_order[offset + 2],
            t2: seed_order[offset + 3],
        });
        offset += 4;
    }

    finals
}

pub fn build_direct_final_pairings(sorted_teams: &[TeamSortData]) -> Vec<Pairing> {
    let mut finals = Vec::new();
    let mut offset = 0;

    while offset + 1 < sorted_teams.len() {
        finals.push(Pairing {
            t1: sorted_teams[offset].team_id,
            t2: sorted_teams[offset + 1].team_id,
        });
        offset += 2;
    }

    finals
}

#[derive(Clone, Debug)]
pub struct PlayoffRound {
    pub name: String,
    pub matches: Vec<Pairing>,
}

// Build full post-swiss structure for a division.
// Every 4-team bracket gets two seed-based rounds. A 2-team bracket gets one match.
pub fn build_playoff_brackets(sorted_teams: &[TeamSortData]) -> Vec<PlayoffRound> {
    let n = sorted_teams.len();
    let mut playoff_matches = Vec::new();
    let mut final_matches = Vec::new();
    let mut offset = 0;

    while offset + 1 < n {
        let remaining = n - offset;
        let bracket_size = if remaining >= 4 { 4 } else { 2 };
        let bracket_rounds = generate_playoff_pairings(sorted_teams, bracket_size, offset);

        for round in bracket_rounds {
            if round.name == "playoffs" {
                playoff_matches.extend(round.matches);
            } else if round.name == "finals" {
                final_matches.extend(round.matches);
            }
        }

        offset += bracket_size;
    }

    let mut rounds = Vec::new();
    if !playoff_matches.is_empty() {
        rounds.push(PlayoffRound {
            name: "playoffs".to_string(),
            matches: playoff_matches,
        });
    }
    if !final_matches.is_empty() {
        rounds.push(PlayoffRound {
            name: "finals".to_string(),
            matches: final_matches,
        });
    }

    rounds
}

// Get match history map from DB matches
pub async fn fetch_match_history(
    db: &sqlx::SqlitePool,
    division: i64,
) -> HashMap<i64, HashSet<i64>> {
    let matches: Vec<(i64, i64)> = sqlx::query_as(
        r#"SELECT m.t1_id, m.t2_id FROM matches m
           JOIN teams t ON m.t1_id = t.id
           WHERE t.division = ? AND m.type < 1000 AND m.possession >= 3 AND m.deleted_at IS NULL"#,
    )
    .bind(division)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let mut history: HashMap<i64, HashSet<i64>> = HashMap::new();
    for (t1, t2) in matches {
        history.entry(t1).or_default().insert(t2);
        history.entry(t2).or_default().insert(t1);
    }
    history
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::sorting::TeamSortData;

    fn team(team_id: i64, wins: i64, points: i64, seed: i64) -> TeamSortData {
        TeamSortData {
            team_id,
            name: format!("Team {team_id}"),
            abbreviation: None,
            small_logo: None,
            init_rank: seed,
            wins,
            losses: 0,
            draws: 0,
            points,
            points_for: 0,
            points_against: 0,
            spirit_avg: 0.0,
            round_results: Vec::new(),
            opponents: Vec::new(),
            h2h: HashMap::new(),
        }
    }

    #[test]
    fn generate_round_pairings_widens_minimally_and_prefers_closer_ranks() {
        let teams = vec![
            team(1, 1, 2, 1),
            team(2, 1, 2, 2),
            team(3, 1, 2, 3),
            team(4, 0, 1, 4),
            team(5, 0, 1, 5),
            team(6, 0, 1, 6),
            team(7, 0, 1, 7),
            team(8, 0, 0, 8),
            team(9, 0, 0, 9),
            team(10, 0, 0, 10),
        ];

        let pairings = generate_round_pairings(&teams, &HashMap::new(), 2);

        assert_eq!(pairings.len(), 5);
        assert!(pairings.iter().any(|pair| pair.t1 == 1 && pair.t2 == 2));
        assert!(pairings.iter().any(|pair| pair.t1 == 3 && pair.t2 == 4));
        assert!(pairings.iter().any(|pair| pair.t1 == 5 && pair.t2 == 6));
        assert!(pairings.iter().any(|pair| pair.t1 == 7 && pair.t2 == 8));
        assert!(pairings.iter().any(|pair| pair.t1 == 9 && pair.t2 == 10));
    }

    #[test]
    fn generate_round_five_prefers_same_point_neighbours_when_scores_are_equal() {
        let teams = vec![
            team(1, 3, 6, 1),
            team(2, 3, 6, 2),
            team(3, 2, 4, 3),
            team(4, 2, 4, 4),
            team(5, 2, 4, 5),
            team(6, 2, 4, 6),
        ];

        let pairings = generate_round_pairings(&teams, &HashMap::new(), 5);

        assert_eq!(pairings.len(), 3);
        assert!(pairings.iter().any(|pair| pair.t1 == 1 && pair.t2 == 2));
        assert!(pairings.iter().any(|pair| pair.t1 == 3 && pair.t2 == 4));
        assert!(pairings.iter().any(|pair| pair.t1 == 5 && pair.t2 == 6));
    }

    #[test]
    fn generate_round_five_keeps_no_rematch_priority_over_neighbour_preference() {
        let teams = vec![
            team(1, 3, 6, 1),
            team(2, 3, 6, 2),
            team(3, 2, 4, 3),
            team(4, 2, 4, 4),
            team(5, 2, 4, 5),
            team(6, 2, 4, 6),
        ];
        let mut history = HashMap::new();
        history.insert(4, HashSet::from([5]));
        history.insert(5, HashSet::from([4]));

        let pairings = generate_round_pairings(&teams, &history, 5);

        assert!(pairings.iter().any(|pair| pair.t1 == 3 && pair.t2 == 4));
        assert!(pairings.iter().any(|pair| pair.t1 == 5 && pair.t2 == 6));
        assert!(!pairings.iter().any(|pair| pair.t1 == 4 && pair.t2 == 5));
    }

    #[test]
    fn generate_round_five_widens_before_repeating_in_small_division() {
        let teams = vec![
            team(1, 4, 8, 1),
            team(2, 3, 6, 2),
            team(3, 2, 5, 3),
            team(4, 2, 5, 4),
            team(5, 2, 4, 5),
            team(6, 2, 4, 6),
            team(7, 1, 3, 7),
            team(8, 1, 2, 8),
            team(9, 1, 2, 9),
            team(10, 0, 1, 10),
        ];
        let mut history = HashMap::new();
        for (left, right) in [
            (3, 5),
            (6, 10),
            (4, 8),
            (1, 9),
            (2, 7),
            (3, 4),
            (5, 8),
            (2, 10),
            (1, 6),
            (7, 9),
            (1, 2),
            (3, 6),
            (4, 10),
            (5, 9),
            (7, 8),
            (1, 3),
            (2, 5),
            (4, 6),
            (7, 8),
            (9, 10),
        ] {
            history.entry(left).or_insert_with(HashSet::new).insert(right);
            history.entry(right).or_insert_with(HashSet::new).insert(left);
        }

        let pairings = generate_round_pairings(&teams, &history, 5);

        assert_eq!(pairings.len(), 5);
        for pairing in &pairings {
            assert!(
                !history
                    .get(&pairing.t1)
                    .map(|opponents| opponents.contains(&pairing.t2))
                    .unwrap_or(false),
                "unexpected rematch: {} vs {}",
                pairing.t1,
                pairing.t2,
            );
        }
    }

    #[test]
    fn round_five_lookahead_scores_final_boundary_meetings_higher() {
        let teams = vec![
            team(1, 4, 8, 1),
            team(2, 4, 8, 2),
            team(3, 3, 6, 3),
            team(4, 3, 6, 4),
            team(5, 3, 6, 5),
            team(6, 3, 6, 6),
        ];
        let history = HashMap::new();
        let boundary_pairings = vec![
            Pairing { t1: 1, t2: 2 },
            Pairing { t1: 3, t2: 6 },
            Pairing { t1: 4, t2: 5 },
        ];
        let off_boundary_pairings = vec![
            Pairing { t1: 1, t2: 2 },
            Pairing { t1: 3, t2: 5 },
            Pairing { t1: 4, t2: 6 },
        ];

        let boundary_score =
            evaluate_round_lookahead(&teams, &history, 5, &boundary_pairings, 12);
        let off_boundary_score =
            evaluate_round_lookahead(&teams, &history, 5, &off_boundary_pairings, 12);

        assert!(boundary_score.score > off_boundary_score.score);
    }

    #[test]
    fn lookahead_score_penalizes_same_point_boundary_misses() {
        let final_sorted = vec![
            team(1, 4, 8, 1),
            team(2, 4, 8, 2),
            team(3, 3, 6, 3),
            team(4, 3, 6, 4),
            team(5, 3, 6, 5),
            team(6, 3, 6, 6),
        ];

        let met_history = HashMap::from([
            (4, HashSet::from([5])),
            (5, HashSet::from([4])),
            (3, HashSet::from([6])),
            (6, HashSet::from([3])),
        ]);
        let missed_history = HashMap::new();

        let met_score = score_final_boundary_meetings(&final_sorted, &met_history);
        let missed_score = score_final_boundary_meetings(&final_sorted, &missed_history);

        assert!(met_score.score > missed_score.score);
        assert_eq!(met_score.same_point_boundary_stats.met, 2);
        assert_eq!(
            met_score.same_point_boundary_stats.cases - met_score.same_point_boundary_stats.met,
            0
        );
        assert_eq!(missed_score.same_point_boundary_stats.met, 0);
        assert_eq!(
            missed_score.same_point_boundary_stats.cases
                - missed_score.same_point_boundary_stats.met,
            2
        );
    }

    #[test]
    fn generate_round_pairings_with_diagnostics_reports_same_point_probability() {
        let teams = vec![
            team(1, 3, 6, 1),
            team(2, 3, 6, 2),
            team(3, 2, 4, 3),
            team(4, 2, 4, 4),
            team(5, 2, 4, 5),
            team(6, 2, 4, 6),
        ];

        let result = generate_round_pairings_with_diagnostics(&teams, &HashMap::new(), 5);

        assert_eq!(result.pairings.len(), 3);
        assert_eq!(result.diagnostics.point_gap_limit, Some(0));
        assert_eq!(result.diagnostics.simulations_per_candidate, 24);
        assert!(result.diagnostics.total_simulations_run >= 24);
    }

    #[test]
    fn build_playoff_brackets_handles_twenty_two_teams() {
        let teams: Vec<TeamSortData> = (1..=22)
            .map(|team_id| team(team_id, 0, 0, team_id))
            .collect();

        let rounds = build_playoff_brackets(&teams);
        let playoff_round = rounds
            .iter()
            .find(|round| round.name == "playoffs")
            .expect("playoff round");
        let final_round = rounds
            .iter()
            .find(|round| round.name == "finals")
            .expect("final round");

        assert_eq!(playoff_round.matches.len(), 11);
        assert_eq!(final_round.matches.len(), 10);
        assert!(
            playoff_round
                .matches
                .iter()
                .any(|pair| pair.t1 == 21 && pair.t2 == 22)
        );
        assert!(
            !final_round
                .matches
                .iter()
                .any(|pair| [21, 22].contains(&pair.t1) || [21, 22].contains(&pair.t2))
        );
    }

    #[test]
    fn build_direct_final_pairings_handles_ten_teams() {
        let teams: Vec<TeamSortData> = (1..=10)
            .map(|team_id| team(team_id, 0, 0, team_id))
            .collect();

        let finals = build_direct_final_pairings(&teams);

        assert_eq!(finals.len(), 5);
        assert_eq!(finals[0], Pairing { t1: 1, t2: 2 });
        assert_eq!(finals[4], Pairing { t1: 9, t2: 10 });
    }

    #[test]
    fn build_seed_order_after_elimination_results_swaps_direct_final_pair() {
        let teams: Vec<TeamSortData> = (1..=10)
            .map(|team_id| team(team_id, 0, 0, team_id))
            .collect();
        let final_results = vec![PlayedMatchResult {
            t1: 9,
            t2: 10,
            winner: 10,
        }];

        let seed_order = build_seed_order_after_elimination_results(&teams, &[], &final_results);

        assert_eq!(&seed_order[8..10], &[10, 9]);
    }

    #[test]
    fn build_final_pairings_swaps_seed_labels_after_upset() {
        let teams: Vec<TeamSortData> = (1..=4)
            .map(|team_id| team(team_id, 0, 0, team_id))
            .collect();
        let playoff_results = vec![
            PlayedMatchResult {
                t1: 1,
                t2: 4,
                winner: 4,
            },
            PlayedMatchResult {
                t1: 2,
                t2: 3,
                winner: 2,
            },
        ];

        let finals = build_final_pairings_from_playoff_results(&teams, &playoff_results);

        assert_eq!(
            finals,
            vec![Pairing { t1: 4, t2: 2 }, Pairing { t1: 3, t2: 1 },]
        );
    }
}
