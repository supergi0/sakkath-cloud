use super::harness::{Harness, TestResult};
use super::model::{
    ExpectedPlayoffMatch, ExpectedSwissRound, MatchState, ScheduleGridCellResponse,
    ScheduleGridResponse, ScheduleMatchResponse, SpiritScoreRowResponse, StatsResponse,
    TournamentTracker,
};
use std::collections::{HashMap, HashSet};

pub(crate) async fn assert_public_match_views(
    harness: &Harness,
    tracker: &TournamentTracker,
    match_id: i64,
    expected_spirit_rows: usize,
    expected_confirmations: usize,
) -> TestResult {
    let state = tracker.match_state(match_id);
    let detail = harness.get_match_detail(match_id).await?;
    assert_eq!(detail.t1_score, state.t1_score);
    assert_eq!(detail.t2_score, state.t2_score);
    assert_eq!(detail.possession, state.possession);

    let expected_t1_spirit = opponent_spirit_for_team(state, state.t1_id);
    let expected_t2_spirit = opponent_spirit_for_team(state, state.t2_id);
    assert_eq!(detail.t1_spirit, expected_t1_spirit);
    assert_eq!(detail.t2_spirit, expected_t2_spirit);

    let spirits = harness.get_match_spirits(match_id).await?;
    assert_eq!(spirits.len(), expected_spirit_rows);
    assert_spirit_rows_match_tracker(state, &spirits);

    let confirmations = harness.get_score_confirmations(match_id).await?;
    assert_eq!(confirmations.len(), expected_confirmations);
    for row in &confirmations {
        assert_eq!(
            state.score_confirmations.get(&row.team_id),
            Some(&(row.t1_score, row.t2_score))
        );
    }

    for team_id in [state.t1_id, state.t2_id] {
        let team_matches = harness.get_team_matches(team_id).await?;
        let team_row = team_matches
            .iter()
            .find(|team_match| team_match.id == match_id)
            .expect("match should be visible on both team pages");
        assert_eq!(team_row.t1_score, state.t1_score);
        assert_eq!(team_row.t2_score, state.t2_score);
        assert_eq!(team_row.t1_spirit, expected_t1_spirit);
        assert_eq!(team_row.t2_spirit, expected_t2_spirit);
    }

    let schedule_rows = harness
        .get_schedule_matches(state.division, Some(state.match_type), None)
        .await?;
    let schedule_row = schedule_rows
        .iter()
        .find(|schedule_match| schedule_match.id == match_id)
        .expect("match should be visible in schedule listing");
    assert_eq!(schedule_row.t1_score, state.t1_score);
    assert_eq!(schedule_row.t2_score, state.t2_score);
    assert_eq!(schedule_row.t1_spirit, expected_t1_spirit);
    assert_eq!(schedule_row.t2_spirit, expected_t2_spirit);

    let grid = harness.get_schedule_grid().await?;
    let cell = find_grid_cell(&grid, match_id).expect("match should be visible on the public grid");
    assert_eq!(cell.t1_score, Some(state.t1_score));
    assert_eq!(cell.t2_score, Some(state.t2_score));
    assert_eq!(cell.status, expected_status_label(state.possession));

    Ok(())
}

pub(crate) async fn assert_player_stats_match_tracker(
    harness: &Harness,
    tracker: &TournamentTracker,
) -> TestResult {
    let player_stats = harness.get_player_stats().await?;
    let player_map: HashMap<i64, _> = player_stats.iter().map(|row| (row.id, row)).collect();

    let mut tracked_totals = (0, 0, 0, 0);
    for (player_id, totals) in &tracker.players {
        tracked_totals.0 += totals.goals;
        tracked_totals.1 += totals.assists;
        tracked_totals.2 += totals.blocks;
        tracked_totals.3 += totals.turnovers;

        let row = player_map.get(player_id).unwrap_or_else(|| {
            panic!("tracked player {player_id} missing from player stats endpoint")
        });
        assert_eq!(row.goals, totals.goals);
        assert_eq!(row.assists, totals.assists);
        assert_eq!(row.blocks, totals.blocks);
        assert_eq!(row.turnovers, totals.turnovers);
        assert_eq!(row.matches, totals.matches.len() as i64);
    }

    let actual_totals = player_stats.iter().fold((0, 0, 0, 0), |acc, row| {
        (
            acc.0 + row.goals,
            acc.1 + row.assists,
            acc.2 + row.blocks,
            acc.3 + row.turnovers,
        )
    });
    assert_eq!(actual_totals, tracked_totals);

    Ok(())
}

pub(crate) async fn assert_standings_match_tracker(
    harness: &Harness,
    tracker: &TournamentTracker,
    division: i64,
) -> TestResult {
    let expected_order: Vec<i64> = tracker
        .swiss_summary(division)
        .iter()
        .map(|row| row.team_id)
        .collect();
    assert_standings_match_tracker_with_order(harness, tracker, division, &expected_order, false)
        .await
}

pub(crate) async fn assert_display_standings_and_team_ranks(
    harness: &Harness,
    tracker: &TournamentTracker,
    division: i64,
    expected_order: &[i64],
) -> TestResult {
    assert_standings_match_tracker_with_order(harness, tracker, division, expected_order, true)
        .await
}

async fn assert_standings_match_tracker_with_order(
    harness: &Harness,
    tracker: &TournamentTracker,
    division: i64,
    expected_order: &[i64],
    assert_team_ranks: bool,
) -> TestResult {
    let standings = harness.get_standings(division).await?;
    let actual_ids: Vec<i64> = standings.iter().map(|row| row.id).collect();
    assert_eq!(actual_ids, expected_order);

    let expected_by_id: HashMap<i64, _> = tracker
        .display_summary_for_order(division, expected_order)
        .into_iter()
        .map(|row| (row.team_id, row))
        .collect();

    for row in &standings {
        let expected_row = expected_by_id
            .get(&row.id)
            .unwrap_or_else(|| panic!("missing expected standings row for team {}", row.id));
        assert_eq!(row.wins, expected_row.wins);
        assert_eq!(row.losses, expected_row.losses);
        assert_eq!(row.draws, expected_row.draws);
        assert_eq!(row.points_for, expected_row.points_for);
        assert_eq!(row.points_against, expected_row.points_against);
        assert!((row.spirit_avg - expected_row.spirit_avg).abs() < 1e-9);
    }

    if assert_team_ranks {
        for (index, team_id) in expected_order.iter().enumerate() {
            let detail = harness.get_team_detail(*team_id).await?;
            let expected_row = expected_by_id
                .get(team_id)
                .unwrap_or_else(|| panic!("missing expected standings row for team {}", team_id));
            assert_eq!(detail.id, *team_id);
            assert_eq!(detail.games_played, expected_row.wins + expected_row.losses + expected_row.draws);
            assert_eq!(detail.wins, expected_row.wins);
            assert_eq!(detail.losses, expected_row.losses);
            assert_eq!(detail.draws, expected_row.draws);
            assert!((detail.spirit_avg - expected_row.spirit_avg).abs() < 1e-9);
            assert_eq!(detail.current_rank, index as i64 + 1);
        }
    }

    Ok(())
}

pub(crate) fn assert_swiss_round_pairings(
    round_matches: &[ScheduleMatchResponse],
    expected: &ExpectedSwissRound,
) {
    let actual: HashSet<(i64, i64)> = round_matches
        .iter()
        .map(|row| (row.t1_id, row.t2_id))
        .collect();
    let expected_pairs: HashSet<(i64, i64)> = expected.pairings.iter().copied().collect();
    assert_eq!(actual, expected_pairs);
}

pub(crate) async fn assert_swiss_grid_seed_labels(
    harness: &Harness,
    tracker: &TournamentTracker,
    round_matches: &[ScheduleMatchResponse],
    division: i64,
) -> TestResult {
    let rank_map = tracker.standings_rank_map(division);
    let grid = harness.get_schedule_grid().await?;

    for schedule_match in round_matches {
        let cell = find_grid_cell(&grid, schedule_match.id)
            .expect("swiss match should appear in the schedule grid");
        assert_eq!(
            cell.t1_seed_rank,
            rank_map.get(&schedule_match.t1_id).copied()
        );
        assert_eq!(
            cell.t2_seed_rank,
            rank_map.get(&schedule_match.t2_id).copied()
        );
    }

    Ok(())
}

pub(crate) fn assert_playoff_pairings(
    actual: &[ScheduleMatchResponse],
    expected: &[ExpectedPlayoffMatch],
) {
    let actual_pairs: HashSet<(i64, i64)> =
        actual.iter().map(|row| (row.t1_id, row.t2_id)).collect();
    let expected_pairs: HashSet<(i64, i64)> = expected
        .iter()
        .map(|row| (row.team_a, row.team_b))
        .collect();
    assert_eq!(actual_pairs, expected_pairs);
}

pub(crate) async fn assert_playoff_grid_seed_labels(
    harness: &Harness,
    round_matches: &[ScheduleMatchResponse],
    expected: &[ExpectedPlayoffMatch],
) -> TestResult {
    let grid = harness.get_schedule_grid().await?;
    let expected_by_pair: HashMap<(i64, i64), (i64, i64)> = expected
        .iter()
        .map(|row| ((row.team_a, row.team_b), (row.seed_a, row.seed_b)))
        .collect();

    for schedule_match in round_matches {
        let expected_seeds = expected_by_pair
            .get(&(schedule_match.t1_id, schedule_match.t2_id))
            .unwrap_or_else(|| {
                panic!(
                    "missing expected playoff seeds for match {}",
                    schedule_match.id
                )
            });
        let cell = find_grid_cell(&grid, schedule_match.id)
            .expect("playoff match should appear in the public grid");
        assert_eq!(cell.t1_seed_rank, Some(expected_seeds.0));
        assert_eq!(cell.t2_seed_rank, Some(expected_seeds.1));
    }

    Ok(())
}

pub(crate) async fn assert_final_public_state(
    harness: &Harness,
    tracker: &TournamentTracker,
    baseline_stats: &StatsResponse,
) -> TestResult {
    let final_stats = harness.get_stats().await?;
    assert_eq!(final_stats.teams, baseline_stats.teams);
    assert_eq!(final_stats.players, baseline_stats.players);
    assert_eq!(final_stats.fields, baseline_stats.fields);
    assert_eq!(final_stats.games, tracker.completed_match_count());
    assert_eq!(final_stats.points, tracker.total_points_scored());

    assert_player_stats_match_tracker(harness, tracker).await?;

    for team_id in tracker.teams.keys().copied() {
        let team_matches = harness.get_team_matches(team_id).await?;
        let actual_ids: HashSet<i64> = team_matches.iter().map(|row| row.id).collect();
        let expected_ids = tracker.match_ids_for_team(team_id);
        assert_eq!(actual_ids, expected_ids);

        for row in &team_matches {
            let tracked = tracker.match_state(row.id);
            assert_eq!(row.t1_score, tracked.t1_score);
            assert_eq!(row.t2_score, tracked.t2_score);
            assert_eq!(
                row.t1_spirit,
                opponent_spirit_for_team(tracked, tracked.t1_id)
            );
            assert_eq!(
                row.t2_spirit,
                opponent_spirit_for_team(tracked, tracked.t2_id)
            );
            assert_eq!(row.match_type, tracked.match_type);
        }
    }

    for division in [0, 1] {
        let expected_order = tracker.expected_display_order_after_elimination(division);
        assert_display_standings_and_team_ranks(harness, tracker, division, &expected_order)
            .await?;
    }

    Ok(())
}

fn opponent_spirit_for_team(state: &MatchState, team_id: i64) -> Option<i64> {
    state
        .spirit_rows
        .iter()
        .find_map(|((rated_team_id, submitted_by_team_id), submission)| {
            if *rated_team_id == team_id && *submitted_by_team_id != team_id {
                Some(submission.total)
            } else {
                None
            }
        })
}

fn assert_spirit_rows_match_tracker(state: &MatchState, rows: &[SpiritScoreRowResponse]) {
    let actual: HashMap<(i64, i64), &SpiritScoreRowResponse> = rows
        .iter()
        .map(|row| ((row.team_id, row.submitted_by_team_id), row))
        .collect();

    assert_eq!(actual.len(), state.spirit_rows.len());

    for (key, submission) in &state.spirit_rows {
        let row = actual
            .get(key)
            .unwrap_or_else(|| panic!("missing public spirit row for {:?}", key));
        assert_eq!(row.total, submission.total);
        assert_eq!(row.mvp_player_id, submission.mvp_player_id);
        assert_eq!(row.msp_player_id, submission.msp_player_id);
    }
}

fn expected_status_label(possession: Option<i64>) -> &'static str {
    match possession {
        Some(value) if value >= 3 => "done",
        Some(_) => "live",
        None => "upcoming",
    }
}

fn find_grid_cell(grid: &ScheduleGridResponse, match_id: i64) -> Option<&ScheduleGridCellResponse> {
    grid.rows
        .iter()
        .flat_map(|row| row.cells.iter())
        .find(|cell| cell.match_id == Some(match_id))
}
