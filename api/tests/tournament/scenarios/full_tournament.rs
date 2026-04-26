use crate::tournament::assertions::{
    assert_display_standings_and_team_ranks, assert_final_public_state,
    assert_playoff_grid_seed_labels, assert_playoff_pairings, assert_standings_match_tracker,
    assert_swiss_cutline_crossovers_completed, assert_swiss_grid_seed_labels,
    assert_swiss_round_pairings,
};
use crate::tournament::config::RunConfig;
use crate::tournament::harness::{Harness, TestResult};
use crate::tournament::model::{ExpectedPlayoffMatch, SeedPairRound};
use crate::tournament::reporting::ReportWriter;
use crate::tournament::simulation::TournamentSimulation;
use crate::tournament::support::{
    ensure_swiss_round_exists, finish_match_to_outcome, load_round_matches, login_staff,
    submit_standard_post_match,
};

fn describe_cutline_pair_rounds(pair_rounds: &[SeedPairRound]) -> String {
    pair_rounds
        .iter()
        .map(|pair| match pair.round {
            Some(round) => format!("{}v{}=R{}", pair.seed_a, pair.seed_b, round),
            None => format!("{}v{}=missing", pair.seed_a, pair.seed_b),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) async fn run(config: &RunConfig, reporter: &mut ReportWriter) -> TestResult {
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
                        "[full-tournament] swiss round {round} for division {division} required rematch-aware backtracking."
                    ))?;
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

                submit_standard_post_match(&mut harness, &mut tracker, schedule_match.id, false)
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
        let pair_rounds = assert_swiss_cutline_crossovers_completed(&tracker, division);
        reporter.note(format!(
            "[full-tournament] {} swiss cutline crossovers by round 6: {}",
            if division == 0 { "Open" } else { "Women" },
            describe_cutline_pair_rounds(&pair_rounds),
        ))?;
    }

    for division in [0, 1] {
        let playoff_round_one = load_round_matches(&harness, &mut tracker, division, 1001).await?;
        enable_reporting_round(&harness, staff.super_admin.as_str(), 1001).await?;
        let expected_round_one = tracker.expected_playoff_round_one(division);
        assert_playoff_pairings(&playoff_round_one, &expected_round_one);
        assert_playoff_grid_seed_labels(&harness, &playoff_round_one, &expected_round_one).await?;
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
                finish_match_to_outcome(&harness, &mut tracker, &staff, schedule_match.id, outcome)
                    .await?;
                format!("seeded {}", outcome.label())
            } else {
                "pre-existing completed match snapshot".to_string()
            };

            submit_standard_post_match(&mut harness, &mut tracker, schedule_match.id, false)
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

        let playoff_round_two = load_round_matches(&harness, &mut tracker, division, 1002).await?;
        enable_reporting_round(&harness, staff.super_admin.as_str(), 1002).await?;
        let expected_round_two = tracker.expected_playoff_round_two(division);
        assert_playoff_pairings(&playoff_round_two, &expected_round_two);
        assert_playoff_grid_seed_labels(&harness, &playoff_round_two, &expected_round_two).await?;
        reporter.record_pairings(
            "full-tournament",
            division,
            "Playoff round 2",
            &playoff_round_two,
            &tracker,
        )?;

        let final_swiss_order = tracker.swiss_summary(division);
        if final_swiss_order.len() >= 2 {
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

            submit_standard_post_match(&mut harness, &mut tracker, schedule_match.id, false)
                .await?;
            reporter.record_match_result(
                "full-tournament",
                division,
                "Playoff round 2",
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
    reporter.note(format!(
        "[full-tournament] completed {} matches, scored {} total points, and observed {} rematch-avoidance rounds.",
        tracker.completed_match_count(),
        tracker.total_points_scored(),
        backtracking_evidence,
    ))?;

    Ok(())
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
