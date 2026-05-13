use crate::tournament::assertions::{assert_player_stats_match_tracker, assert_public_match_views};
use crate::tournament::config::RunConfig;
use crate::tournament::harness::{Harness, TestResult};
use crate::tournament::reporting::ReportWriter;
use crate::tournament::support::{
    login_poc, login_staff, players_for_team, record_event, submit_spirit_payload,
};
use axum::http::{Method, StatusCode};

pub(crate) async fn run(config: &RunConfig, reporter: &mut ReportWriter) -> TestResult {
    reporter.start_scenario(
        "match-flow",
        "Exercises one full reported match, including visibility checks, score confirmation, and spirit submissions.",
    )?;

    let mut harness = Harness::new(
        &config.scenario_label("match-flow"),
        config.scenario_seed("match-flow-stage-seeds"),
    )
    .await?;
    let mut tracker = harness.load_initial_tracker().await?;
    let staff = login_staff(&mut harness).await?;

    let target_match = harness
        .get_schedule_matches(0, None, None)
        .await?
        .into_iter()
        .find(|match_row| match_row.match_type > 0 && match_row.match_type < 1000 && match_row.possession.is_none())
        .expect("expected an unstarted Open Swiss match");

    let team_one_poc = login_poc(&mut harness, target_match.t1_id).await?;
    let team_two_poc = login_poc(&mut harness, target_match.t2_id).await?;

    for token in [
        staff.admin_one.as_str(),
        staff.admin_two.as_str(),
        staff.super_admin.as_str(),
        team_one_poc.as_str(),
        team_two_poc.as_str(),
    ] {
        let visible = harness.get_upcoming_matches(token).await?;
        assert!(
            visible
                .iter()
                .any(|match_row| match_row.id == target_match.id),
            "match {} should be visible to reporter token {}",
            target_match.id,
            token
        );
    }

    let round_settings = harness
        .get_reporting_round_settings(staff.admin_one.as_str())
        .await?;
    assert_eq!(round_settings.len(), 8);
    let round_one_setting = round_settings
        .iter()
        .find(|setting| setting.round_key == target_match.match_type)
        .expect("selected round setting should exist");
    assert_eq!(
        round_one_setting.label,
        format!("Round {}", target_match.match_type)
    );
    if round_one_setting.is_enabled {
        let disabled_round = harness
            .update_reporting_round_setting(
                staff.super_admin.as_str(),
                target_match.match_type,
                false,
            )
            .await?;
        assert!(!disabled_round.is_enabled);
    }

    harness
        .expect_status(
            Method::POST,
            &format!("/v1/admin/matches/{}/start", target_match.id),
            Some(team_one_poc.as_str()),
            Some(serde_json::json!({ "possession": 1 })),
            StatusCode::FORBIDDEN,
        )
        .await?;

    let enabled_round = harness
        .update_reporting_round_setting(
            staff.super_admin.as_str(),
            target_match.match_type,
            true,
        )
        .await?;
    assert!(enabled_round.is_enabled);
    assert_eq!(
        enabled_round.label,
        format!("Round {}", target_match.match_type)
    );

    let detail = harness.get_match_detail(target_match.id).await?;
    assert_eq!(detail.possession, None);
    assert_eq!(detail.match_type, target_match.match_type);
    assert!(detail.reporting_enabled);

    let t1_players = players_for_team(&detail, detail.t1_id);
    let t2_players = players_for_team(&detail, detail.t2_id);

    let response = harness
        .mutation(
            Method::POST,
            &format!("/v1/admin/matches/{}/start", target_match.id),
            staff.admin_one.as_str(),
            Some(serde_json::json!({ "possession": 1 })),
        )
        .await?;
    assert!(response.success);
    tracker.set_match_state(
        target_match.id,
        detail.t1_score,
        detail.t2_score,
        response.possession,
    );

    record_event(
        &harness,
        &mut tracker,
        staff.admin_two.as_str(),
        target_match.id,
        Some(t1_players[1].id),
        1,
    )
    .await?;
    record_event(
        &harness,
        &mut tracker,
        staff.super_admin.as_str(),
        target_match.id,
        Some(t1_players[0].id),
        0,
    )
    .await?;
    record_event(
        &harness,
        &mut tracker,
        staff.admin_two.as_str(),
        target_match.id,
        Some(t2_players[1].id),
        1,
    )
    .await?;
    let goal_two = record_event(
        &harness,
        &mut tracker,
        staff.admin_one.as_str(),
        target_match.id,
        Some(t2_players[0].id),
        0,
    )
    .await?;
    assert_eq!(goal_two.t1_score, Some(1));
    assert_eq!(goal_two.t2_score, Some(1));

    let end = harness
        .mutation(
            Method::POST,
            &format!("/v1/admin/matches/{}/end", target_match.id),
            staff.super_admin.as_str(),
            None,
        )
        .await?;
    assert!(end.success);
    tracker.set_match_state(target_match.id, 1, 1, Some(3));

    let confirm_one = harness
        .mutation(
            Method::POST,
            &format!("/v1/poc/matches/{}/confirm-score", target_match.id),
            team_one_poc.as_str(),
            Some(serde_json::json!({ "t1_score": 1, "t2_score": 1 })),
        )
        .await?;
    tracker.set_score_confirmation(target_match.id, target_match.t1_id, 1, 1);
    assert_eq!(confirm_one.finalized, Some(false));

    let confirm_two = harness
        .mutation(
            Method::POST,
            &format!("/v1/poc/matches/{}/confirm-score", target_match.id),
            team_two_poc.as_str(),
            Some(serde_json::json!({ "t1_score": 1, "t2_score": 1 })),
        )
        .await?;
    tracker.set_score_confirmation(target_match.id, target_match.t2_id, 1, 1);
    assert_eq!(confirm_two.finalized, Some(true));

    harness
        .expect_status(
            Method::POST,
            &format!("/v1/admin/matches/{}/event", target_match.id),
            Some(staff.admin_two.as_str()),
            Some(serde_json::json!({ "player_id": t1_players[0].id, "event_type": 0 })),
            StatusCode::CONFLICT,
        )
        .await?;

    let opponent_for_team_one = harness
        .get_opponent_players(team_one_poc.as_str(), target_match.id)
        .await?;
    assert!(
        opponent_for_team_one
            .iter()
            .all(|player| player.team_id == target_match.t2_id)
    );
    let opponent_for_team_two = harness
        .get_opponent_players(team_two_poc.as_str(), target_match.id)
        .await?;
    assert!(
        opponent_for_team_two
            .iter()
            .all(|player| player.team_id == target_match.t1_id)
    );

    submit_spirit_payload(
        &harness,
        &mut tracker,
        team_one_poc.as_str(),
        target_match.id,
        target_match.t2_id,
        target_match.t1_id,
        &opponent_for_team_one,
    )
    .await?;
    submit_spirit_payload(
        &harness,
        &mut tracker,
        team_two_poc.as_str(),
        target_match.id,
        target_match.t1_id,
        target_match.t2_id,
        &opponent_for_team_two,
    )
    .await?;
    submit_spirit_payload(
        &harness,
        &mut tracker,
        team_one_poc.as_str(),
        target_match.id,
        target_match.t1_id,
        target_match.t1_id,
        &t1_players,
    )
    .await?;
    submit_spirit_payload(
        &harness,
        &mut tracker,
        team_two_poc.as_str(),
        target_match.id,
        target_match.t2_id,
        target_match.t2_id,
        &t2_players,
    )
    .await?;

    assert_public_match_views(&harness, &tracker, target_match.id, 4, 2).await?;
    assert_player_stats_match_tracker(&harness, &tracker).await?;

    let standings = harness.get_standings(0).await?;
    let tracked = tracker.match_state(target_match.id);
    let tracked_rows = tracker
        .swiss_summary(0)
        .into_iter()
        .map(|row| (row.team_id, row))
        .collect::<std::collections::HashMap<_, _>>();
    for team_id in [tracked.t1_id, tracked.t2_id] {
        let row = standings
            .iter()
            .find(|row| row.id == team_id)
            .expect("tracked tied team should appear in standings");
        let tracked_row = tracked_rows
            .get(&team_id)
            .expect("tracked team should appear in tracker standings");
        assert_eq!(row.wins, tracked_row.wins);
        assert_eq!(row.losses, tracked_row.losses);
        assert_eq!(row.draws, tracked_row.draws);
        assert_eq!(row.points_for, tracked_row.points_for);
        assert_eq!(row.points_against, tracked_row.points_against);
    }

    reporter.record_match_result(
        "match-flow",
        0,
        &format!("Swiss round {}", target_match.match_type),
        target_match.id,
        &tracker,
        "1-1 draw with dual confirmations and both opponent/self spirit rows",
    )?;
    reporter.record_standings("match-flow", 0, "after match-flow verification", &tracker)?;
    reporter.note(
        "[match-flow] verified admin/super visibility, score-lock conflict handling, and public post-match state.",
    )?;

    Ok(())
}
