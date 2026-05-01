use crate::tournament::assertions::{
    assert_display_standings_and_team_ranks, assert_final_public_state,
    assert_playoff_grid_seed_labels, assert_playoff_pairings, assert_standings_match_tracker,
    assert_swiss_grid_seed_labels, assert_swiss_round_pairings,
};
use crate::tournament::config::RunConfig;
use crate::tournament::harness::{Harness, TestResult};
use crate::tournament::model::{ExpectedPlayoffMatch, SortMetrics, TournamentTracker};
use crate::tournament::reporting::ReportWriter;
use crate::tournament::simulation::TournamentSimulation;
use crate::tournament::support::{
    ensure_swiss_round_exists, finish_match_to_outcome, load_round_matches, login_staff,
    submit_standard_post_match,
};
use api::helpers::rounds::RoundPairingDiagnostics;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub(crate) struct BoundaryMeetingSummary {
    pub(crate) label: &'static str,
    pub(crate) same_points: bool,
    pub(crate) met: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct LookaheadRoundSummary {
    pub(crate) round: i64,
    pub(crate) diagnostics: RoundPairingDiagnostics,
}

#[derive(Debug, Clone)]
pub(crate) struct DivisionTournamentSummary {
    pub(crate) division: i64,
    pub(crate) boundary_meetings: Vec<BoundaryMeetingSummary>,
    pub(crate) lookahead_rounds: Vec<LookaheadRoundSummary>,
    pub(crate) game_gap_minutes: Vec<i64>,
    pub(crate) ground_distribution_score: f64,
    pub(crate) final_swiss_points_by_rank: Vec<i64>,
    pub(crate) swiss_rematches: usize,
    pub(crate) tournament_rematches: usize,
    pub(crate) team_tournament_rematches: Vec<(i64, usize)>,
}

#[derive(Debug, Clone)]
pub(crate) struct FullTournamentSummary {
    pub(crate) divisions: Vec<DivisionTournamentSummary>,
    pub(crate) ground_distribution_score: f64,
}

fn describe_boundary_meetings(boundary_meetings: &[BoundaryMeetingSummary]) -> String {
    boundary_meetings
        .iter()
        .map(|meeting| format!("{}={}", meeting.label, boundary_meeting_status(meeting)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn boundary_meeting_status(meeting: &BoundaryMeetingSummary) -> &'static str {
    if !meeting.same_points {
        "points-split"
    } else if meeting.met {
        "same-points-met"
    } else {
        "same-points-missed"
    }
}

fn format_optional_probability(probability: Option<f64>) -> String {
    probability
        .map(|probability| format!("{:.1}%", probability * 100.0))
        .unwrap_or_else(|| "n/a".to_string())
}

fn format_duration_ms(value: f64) -> String {
    format!("{value:.1}ms")
}

fn describe_gap_stats(gaps: &[i64]) -> String {
    let Some(lowest) = gaps.iter().min() else {
        return "mean n/a, median n/a, lowest n/a, highest n/a".to_string();
    };
    let highest = gaps.iter().max().expect("non-empty gap sample");

    format!(
        "mean {:.1} min, median {:.1} min, lowest {} min, highest {} min",
        mean_i64(gaps),
        median_i64(gaps),
        lowest,
        highest,
    )
}

fn describe_round_generation_diagnostics(diagnostics: &RoundPairingDiagnostics) -> String {
    let timing = format!(
        "generated in {} total (exploration {}, simulation {})",
        format_duration_ms(diagnostics.generation_time_ms),
        format_duration_ms(diagnostics.exploration_time_ms),
        format_duration_ms(diagnostics.simulation_time_ms),
    );

    if diagnostics.total_simulations_run == 0 {
        return format!(
            "gap_limit={}, explored {} no-rematch pairings, kept {}, {}",
            diagnostics
                .point_gap_limit
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
            diagnostics.explored_candidates,
            diagnostics.kept_candidates,
            timing,
        );
    }

    format!(
        "gap_limit={}, explored {} no-rematch pairings, kept {}, ran {} simulations, {}, chosen worst-case same-point miss {}, best kept {}, avg kept {}",
        diagnostics
            .point_gap_limit
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        diagnostics.explored_candidates,
        diagnostics.kept_candidates,
        diagnostics.total_simulations_run,
        timing,
        format_optional_probability(
            diagnostics.chosen_worst_case_same_point_miss_probability,
        ),
        format_optional_probability(
            diagnostics.best_kept_worst_case_same_point_miss_probability,
        ),
        format_optional_probability(
            diagnostics.average_kept_worst_case_same_point_miss_probability,
        ),
    )
}

pub(crate) async fn run(config: &RunConfig, reporter: &mut ReportWriter) -> TestResult {
    run_with_summary(config, reporter).await.map(|_| ())
}

pub(crate) async fn run_with_summary(
    config: &RunConfig,
    reporter: &mut ReportWriter,
) -> TestResult<FullTournamentSummary> {
    reporter.start_scenario(
        "full-tournament",
        "Runs all swiss and playoff rounds against the live router, with seeded random winners and markdown reporting.",
    )?;

    let mut harness = Harness::new(&config.scenario_label("full-tournament")).await?;
    let baseline_stats = harness.get_stats().await?;
    let mut tracker = harness.load_initial_tracker().await?;
    let staff = login_staff(&mut harness).await?;
    let mut simulation = TournamentSimulation::new(config.scenario_seed("full-tournament"));

    let mut backtracking_evidence = 0usize;
    let mut lookahead_rounds: HashMap<(i64, i64), RoundPairingDiagnostics> = HashMap::new();

    for round in 1..=6 {
        for division in [0, 1] {
            let round_matches = load_round_matches(&harness, &mut tracker, division, round).await?;
            enable_reporting_round(&harness, staff.super_admin.as_str(), round).await?;

            if round > 1 {
                let expected = tracker.expected_swiss_round(division);
                assert_swiss_round_pairings(&round_matches, &expected);
                assert_swiss_grid_seed_labels(&harness, &tracker, &round_matches, division).await?;
                if expected.naive_had_rematch {
                    backtracking_evidence += 1;
                    reporter.note(format!(
                        "[full-tournament] swiss round {round} for division {division} required no-rematch widening beyond the naive grouped draw."
                    ))?;
                }
                if round >= 4 && expected.diagnostics.explored_candidates > 0 {
                    reporter.note(format!(
                        "[full-tournament] {} Swiss round {round} generation: {}",
                        division_label(division),
                        describe_round_generation_diagnostics(&expected.diagnostics),
                    ))?;
                    lookahead_rounds.insert((division, round), expected.diagnostics.clone());
                }
            }

            reporter.record_pairings(
                "full-tournament",
                division,
                &format!("Swiss round {round}"),
                &round_matches,
                &tracker,
            )?;

            for schedule_match in &round_matches {
                let note = if schedule_match.possession.unwrap_or(0) < 3 {
                    let outcome =
                        simulation.swiss_outcome(round, division, schedule_match, &tracker);
                    finish_match_to_outcome(
                        &harness,
                        &mut tracker,
                        &staff,
                        schedule_match.id,
                        outcome,
                    )
                    .await?;
                    format!("seeded {}", outcome.label())
                } else {
                    "pre-existing completed match snapshot".to_string()
                };

                submit_standard_post_match(&mut harness, &mut tracker, schedule_match.id, true)
                    .await?;
                reporter.record_match_result(
                    "full-tournament",
                    division,
                    &format!("Swiss round {round}"),
                    schedule_match.id,
                    &tracker,
                    &note,
                )?;
            }

            assert_standings_match_tracker(&harness, &tracker, division).await?;
            reporter.record_standings(
                "full-tournament",
                division,
                &format!("after swiss round {round}"),
                &tracker,
            )?;

            if round < 6 {
                let next_round =
                    ensure_swiss_round_exists(&harness, &staff, division, round + 1).await?;
                for schedule_match in &next_round {
                    tracker.register_schedule_match(division, schedule_match);
                }
                let expected = tracker.expected_swiss_round(division);
                assert_swiss_round_pairings(&next_round, &expected);
                assert_swiss_grid_seed_labels(&harness, &tracker, &next_round, division).await?;
            }
        }
    }

    for division in [0, 1] {
        if division == 0 {
            let playoff_round_one =
                load_round_matches(&harness, &mut tracker, division, 1001).await?;
            enable_reporting_round(&harness, staff.super_admin.as_str(), 1001).await?;
            let expected_round_one = tracker.expected_playoff_round_one(division);
            assert_playoff_pairings(&playoff_round_one, &expected_round_one);
            assert_playoff_grid_seed_labels(&harness, &playoff_round_one, &expected_round_one)
                .await?;
            reporter.record_pairings(
                "full-tournament",
                division,
                "Playoff round 1",
                &playoff_round_one,
                &tracker,
            )?;

            for schedule_match in &playoff_round_one {
                let note = if schedule_match.possession.unwrap_or(0) < 3 {
                    let expected_match = find_expected_match(&expected_round_one, schedule_match);
                    let outcome = simulation.playoff_round_one_outcome(expected_match);
                    finish_match_to_outcome(
                        &harness,
                        &mut tracker,
                        &staff,
                        schedule_match.id,
                        outcome,
                    )
                    .await?;
                    format!("seeded {}", outcome.label())
                } else {
                    "pre-existing completed match snapshot".to_string()
                };

                submit_standard_post_match(&mut harness, &mut tracker, schedule_match.id, true)
                    .await?;
                reporter.record_match_result(
                    "full-tournament",
                    division,
                    "Playoff round 1",
                    schedule_match.id,
                    &tracker,
                    &note,
                )?;
            }

            let playoff_display_order = tracker.expected_display_order_after_playoffs(division);
            assert_display_standings_and_team_ranks(
                &harness,
                &tracker,
                division,
                &playoff_display_order,
            )
            .await?;
            reporter.record_ordered_standings(
                "full-tournament",
                division,
                "after playoff round 1 seed swaps",
                &tracker,
                &playoff_display_order,
            )?;
        }

        let playoff_round_two = load_round_matches(&harness, &mut tracker, division, 1002).await?;
        enable_reporting_round(&harness, staff.super_admin.as_str(), 1002).await?;
        let expected_round_two = tracker.expected_playoff_round_two(division);
        assert_playoff_pairings(&playoff_round_two, &expected_round_two);
        assert_playoff_grid_seed_labels(&harness, &playoff_round_two, &expected_round_two).await?;
        let final_stage_label = if division == 0 {
            "Playoff round 2"
        } else {
            "Final round"
        };
        reporter.record_pairings(
            "full-tournament",
            division,
            final_stage_label,
            &playoff_round_two,
            &tracker,
        )?;

        let final_swiss_order = tracker.swiss_summary(division);
        if division == 0 && final_swiss_order.len() >= 2 {
            let bottom_one = final_swiss_order[final_swiss_order.len() - 2].team_id;
            let bottom_two = final_swiss_order[final_swiss_order.len() - 1].team_id;
            assert!(
                tracker
                    .match_ids_for_team_and_type(bottom_one, 1002)
                    .is_empty()
            );
            assert!(
                tracker
                    .match_ids_for_team_and_type(bottom_two, 1002)
                    .is_empty()
            );
        }

        for schedule_match in &playoff_round_two {
            let note = if schedule_match.possession.unwrap_or(0) < 3 {
                let outcome = simulation.playoff_round_two_outcome(&tracker, schedule_match);
                finish_match_to_outcome(&harness, &mut tracker, &staff, schedule_match.id, outcome)
                    .await?;
                format!("seeded {}", outcome.label())
            } else {
                "pre-existing completed match snapshot".to_string()
            };

            submit_standard_post_match(&mut harness, &mut tracker, schedule_match.id, true)
                .await?;
            reporter.record_match_result(
                "full-tournament",
                division,
                final_stage_label,
                schedule_match.id,
                &tracker,
                &note,
            )?;
        }

        let final_display_order = tracker.expected_display_order_after_elimination(division);
        reporter.record_ordered_standings(
            "full-tournament",
            division,
            "final display standings",
            &tracker,
            &final_display_order,
        )?;
    }

    assert_final_public_state(&harness, &tracker, &baseline_stats).await?;
    let summary = build_full_tournament_summary(&tracker, &lookahead_rounds);
    for division_summary in &summary.divisions {
        reporter.note(format!(
            "[full-tournament] {} final swiss equal-point boundary status: {}",
            division_label(division_summary.division),
            describe_boundary_meetings(&division_summary.boundary_meetings),
        ))?;
        reporter.note(format!(
            "[full-tournament] {} time gaps between games: {}",
            division_label(division_summary.division),
            describe_gap_stats(&division_summary.game_gap_minutes),
        ))?;
        reporter.note(format!(
            "[full-tournament] {} ground distribution MSD score: {:.4}",
            division_label(division_summary.division),
            division_summary.ground_distribution_score,
        ))?;
    }
    reporter.note(format!(
        "[full-tournament] tournament ground distribution MSD score: {:.4}",
        summary.ground_distribution_score,
    ))?;
    reporter.note(format!(
        "[full-tournament] completed {} matches, scored {} total points, and observed {} rematch-avoidance rounds.",
        tracker.completed_match_count(),
        tracker.total_points_scored(),
        backtracking_evidence,
    ))?;

    Ok(summary)
}

async fn enable_reporting_round(
    harness: &Harness,
    super_token: &str,
    round_key: i64,
) -> TestResult {
    let response = harness
        .update_reporting_round_setting(super_token, round_key, true)
        .await?;
    assert!(response.is_enabled);
    Ok(())
}

fn build_full_tournament_summary(
    tracker: &TournamentTracker,
    lookahead_rounds: &HashMap<(i64, i64), RoundPairingDiagnostics>,
) -> FullTournamentSummary {
    FullTournamentSummary {
        ground_distribution_score: tracker.ground_distribution_score(None),
        divisions: [0, 1]
            .into_iter()
            .map(|division| build_division_summary(tracker, division, lookahead_rounds))
            .collect(),
    }
}

fn build_division_summary(
    tracker: &TournamentTracker,
    division: i64,
    lookahead_rounds: &HashMap<(i64, i64), RoundPairingDiagnostics>,
) -> DivisionTournamentSummary {
    let final_swiss = tracker.swiss_summary(division);
    let swiss_history = match_history(tracker, division, true);
    let (swiss_rematches, _swiss_team_rematches) = count_rematches(tracker, division, true);
    let (tournament_rematches, tournament_team_rematches) = count_rematches(tracker, division, false);
    let mut team_tournament_rematches: Vec<_> = tournament_team_rematches.into_iter().collect();
    team_tournament_rematches.sort_by_key(|(team_id, _)| *team_id);

    DivisionTournamentSummary {
        division,
        boundary_meetings: build_boundary_meetings(&final_swiss, &swiss_history),
        lookahead_rounds: [4_i64, 5_i64, 6_i64]
            .into_iter()
            .filter_map(|round| {
                lookahead_rounds
                    .get(&(division, round))
                    .cloned()
                    .map(|diagnostics| LookaheadRoundSummary { round, diagnostics })
            })
            .collect(),
        game_gap_minutes: tracker.game_gap_minutes(division),
        ground_distribution_score: tracker.ground_distribution_score(Some(division)),
        final_swiss_points_by_rank: final_swiss.iter().map(|row| row.points).collect(),
        swiss_rematches,
        tournament_rematches,
        team_tournament_rematches,
    }
}

fn mean_i64(values: &[i64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<i64>() as f64 / values.len() as f64
    }
}

fn median_i64(values: &[i64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) as f64 / 2.0
    } else {
        sorted[mid] as f64
    }
}

fn build_boundary_meetings(
    final_swiss: &[SortMetrics],
    swiss_history: &HashMap<i64, HashSet<i64>>,
) -> Vec<BoundaryMeetingSummary> {
    let targets = [(4usize, 5usize, "4v5"), (3, 6, "3v6"), (8, 9, "8v9"), (7, 10, "7v10")];

    targets
        .into_iter()
        .filter_map(|(left_rank, right_rank, label)| {
            if right_rank > final_swiss.len() {
                return None;
            }

            let left_team = final_swiss[left_rank - 1].team_id;
            let right_team = final_swiss[right_rank - 1].team_id;
            Some(BoundaryMeetingSummary {
                label,
                same_points: final_swiss[left_rank - 1].points == final_swiss[right_rank - 1].points,
                met: swiss_history
                    .get(&left_team)
                    .map(|opponents| opponents.contains(&right_team))
                    .unwrap_or(false),
            })
        })
        .collect()
}

fn match_history(
    tracker: &TournamentTracker,
    division: i64,
    swiss_only: bool,
) -> HashMap<i64, HashSet<i64>> {
    let mut history = HashMap::new();

    for state in tracker.matches.values() {
        if state.division != division {
            continue;
        }
        if swiss_only && !(state.match_type > 0 && state.match_type < 1000) {
            continue;
        }

        history.entry(state.t1_id).or_insert_with(HashSet::new).insert(state.t2_id);
        history.entry(state.t2_id).or_insert_with(HashSet::new).insert(state.t1_id);
    }

    history
}

fn count_rematches(
    tracker: &TournamentTracker,
    division: i64,
    swiss_only: bool,
) -> (usize, HashMap<i64, usize>) {
    let mut matches: Vec<_> = tracker
        .matches
        .values()
        .filter(|state| {
            state.division == division
                && (!swiss_only || (state.match_type > 0 && state.match_type < 1000))
        })
        .collect();
    matches.sort_by_key(|state| (state.match_type, state.id));

    let mut seen_pairs = HashSet::new();
    let mut rematch_count = 0usize;
    let mut team_counts = HashMap::new();

    for state in matches {
        let pair = if state.t1_id < state.t2_id {
            (state.t1_id, state.t2_id)
        } else {
            (state.t2_id, state.t1_id)
        };

        if !seen_pairs.insert(pair) {
            rematch_count += 1;
            *team_counts.entry(state.t1_id).or_insert(0) += 1;
            *team_counts.entry(state.t2_id).or_insert(0) += 1;
        }
    }

    (rematch_count, team_counts)
}

fn division_label(division: i64) -> &'static str {
    if division == 0 {
        "Open"
    } else {
        "Women"
    }
}

fn find_expected_match<'a>(
    expected: &'a [ExpectedPlayoffMatch],
    schedule_match: &crate::tournament::model::ScheduleMatchResponse,
) -> &'a ExpectedPlayoffMatch {
    expected
        .iter()
        .find(|expected_match| {
            expected_match.team_a == schedule_match.t1_id
                && expected_match.team_b == schedule_match.t2_id
        })
        .expect("playoff round match should match the expected bracket")
}
