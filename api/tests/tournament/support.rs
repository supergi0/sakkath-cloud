use super::harness::{Harness, STAFF_PASSWORD, TestResult};
use super::model::{
    MatchDetailResponse, MatchPlayerResponse, MutationResponse, ScheduleMatchResponse,
    TournamentTracker,
};
use super::simulation::OutcomeKind;
use axum::http::Method;

pub(crate) struct StaffTokens {
    pub(crate) admin_one: String,
    pub(crate) admin_two: String,
    pub(crate) super_admin: String,
}

impl StaffTokens {
    pub(crate) fn token(&self, index: usize) -> &str {
        match index % 3 {
            0 => self.admin_one.as_str(),
            1 => self.admin_two.as_str(),
            _ => self.super_admin.as_str(),
        }
    }
}

pub(crate) async fn login_staff(harness: &mut Harness) -> TestResult<StaffTokens> {
    let admin_one_email = harness.credentials.admin_emails[0].clone();
    let admin_two_email = harness.credentials.admin_emails[1].clone();
    let super_email = harness.credentials.super_email.clone();

    let admin_one = harness.login(&admin_one_email, STAFF_PASSWORD).await?;
    let admin_two = harness.login(&admin_two_email, STAFF_PASSWORD).await?;
    let super_admin = harness.login(&super_email, STAFF_PASSWORD).await?;

    Ok(StaffTokens {
        admin_one,
        admin_two,
        super_admin,
    })
}

pub(crate) async fn login_poc(harness: &mut Harness, team_id: i64) -> TestResult<String> {
    let email = harness
        .credentials
        .poc_by_team
        .get(&team_id)
        .unwrap_or_else(|| panic!("missing poc credential for team {team_id}"))
        .clone();
    harness.login(&email, &email).await
}

pub(crate) async fn load_round_matches(
    harness: &Harness,
    tracker: &mut TournamentTracker,
    division: i64,
    round: i64,
) -> TestResult<Vec<ScheduleMatchResponse>> {
    let matches = harness
        .get_schedule_matches(division, Some(round), None)
        .await?;
    for schedule_match in &matches {
        tracker.register_schedule_match(division, schedule_match);
    }
    Ok(matches)
}

pub(crate) async fn ensure_swiss_round_exists(
    harness: &Harness,
    staff: &StaffTokens,
    division: i64,
    round: i64,
) -> TestResult<Vec<ScheduleMatchResponse>> {
    let existing = harness
        .get_schedule_matches(division, Some(round), None)
        .await?;
    if !existing.is_empty() {
        return Ok(existing);
    }

    let response = harness
        .mutation(
            Method::POST,
            &format!("/v1/admin/schedule/{division}/generate"),
            staff.admin_one.as_str(),
            None,
        )
        .await?;
    assert!(
        response.success,
        "round {round} for division {division} should generate"
    );

    let generated = harness
        .get_schedule_matches(division, Some(round), None)
        .await?;
    assert!(
        !generated.is_empty(),
        "generated swiss round should now be visible"
    );
    Ok(generated)
}

pub(crate) async fn record_event(
    harness: &Harness,
    tracker: &mut TournamentTracker,
    token: &str,
    match_id: i64,
    player_id: Option<i64>,
    event_type: i64,
) -> TestResult<MutationResponse> {
    let response = harness
        .mutation(
            Method::POST,
            &format!("/v1/admin/matches/{match_id}/event"),
            token,
            Some(serde_json::json!({
                "player_id": player_id,
                "event_type": event_type,
            })),
        )
        .await?;
    tracker.apply_manual_event(match_id, player_id, event_type);
    tracker.set_match_state(
        match_id,
        response
            .t1_score
            .expect("event response should include t1 score"),
        response
            .t2_score
            .expect("event response should include t2 score"),
        response.possession,
    );
    Ok(response)
}

pub(crate) async fn finish_match_to_outcome(
    harness: &Harness,
    tracker: &mut TournamentTracker,
    staff: &StaffTokens,
    match_id: i64,
    outcome: OutcomeKind,
) -> TestResult {
    let match_type = tracker.match_state(match_id).match_type;
    let detail = harness.get_match_detail(match_id).await?;
    tracker.apply_detail(match_type, &detail);

    let t1_players = players_for_team(&detail, detail.t1_id);
    let t2_players = players_for_team(&detail, detail.t2_id);
    let (target_t1, target_t2) = resolve_target_scores(detail.t1_score, detail.t2_score, outcome);
    let scoring_sequence = build_scoring_sequence(
        detail.t1_id,
        detail.t2_id,
        detail.t1_score,
        detail.t2_score,
        target_t1,
        target_t2,
    );

    let mut reporter_index = match_id as usize;
    let mut possession = detail.possession;

    if possession.is_none() && !scoring_sequence.is_empty() {
        let desired_possession = if scoring_sequence[0] == detail.t1_id {
            1
        } else {
            2
        };
        let response = harness
            .mutation(
                Method::POST,
                &format!("/v1/admin/matches/{match_id}/start"),
                staff.token(reporter_index),
                Some(serde_json::json!({ "possession": desired_possession })),
            )
            .await?;
        possession = response.possession;
        tracker.set_match_state(match_id, detail.t1_score, detail.t2_score, possession);
        reporter_index += 1;
    }

    for scoring_team in scoring_sequence {
        while offense_team_id(possession, detail.t1_id, detail.t2_id) != Some(scoring_team) {
            let response = harness
                .mutation(
                    Method::POST,
                    &format!("/v1/admin/matches/{match_id}/switch-possession"),
                    staff.token(reporter_index),
                    None,
                )
                .await?;
            assert!(response.success);
            tracker.set_match_state(
                match_id,
                response
                    .t1_score
                    .unwrap_or_else(|| tracker.match_state(match_id).t1_score),
                response
                    .t2_score
                    .unwrap_or_else(|| tracker.match_state(match_id).t2_score),
                response.possession,
            );
            possession = response.possession;
            reporter_index += 1;
        }

        let players = if scoring_team == detail.t1_id {
            &t1_players
        } else {
            &t2_players
        };
        let assist_player = players.get(1).unwrap_or(&players[0]).id;
        record_event(
            harness,
            tracker,
            staff.token(reporter_index),
            match_id,
            Some(assist_player),
            1,
        )
        .await?;
        reporter_index += 1;

        let goal_response = record_event(
            harness,
            tracker,
            staff.token(reporter_index),
            match_id,
            Some(players[0].id),
            0,
        )
        .await?;
        possession = goal_response.possession;
        reporter_index += 1;
    }

    let state = tracker.match_state(match_id);
    assert_eq!(state.t1_score, target_t1);
    assert_eq!(state.t2_score, target_t2);

    if tracker.match_state(match_id).possession.unwrap_or(0) < 3 {
        let response = harness
            .mutation(
                Method::POST,
                &format!("/v1/admin/matches/{match_id}/end"),
                staff.token(reporter_index),
                None,
            )
            .await?;
        assert!(response.success);
        tracker.set_match_state(match_id, target_t1, target_t2, Some(3));
    }

    Ok(())
}

pub(crate) async fn submit_standard_post_match(
    harness: &mut Harness,
    tracker: &mut TournamentTracker,
    match_id: i64,
    include_self_ratings: bool,
) -> TestResult {
    let detail = harness.get_match_detail(match_id).await?;
    let team_one_poc = login_poc(harness, detail.t1_id).await?;
    let team_two_poc = login_poc(harness, detail.t2_id).await?;

    let confirm_one = harness
        .mutation(
            Method::POST,
            &format!("/v1/poc/matches/{match_id}/confirm-score"),
            team_one_poc.as_str(),
            Some(serde_json::json!({
                "t1_score": detail.t1_score,
                "t2_score": detail.t2_score,
            })),
        )
        .await?;
    assert!(confirm_one.success);
    tracker.set_score_confirmation(match_id, detail.t1_id, detail.t1_score, detail.t2_score);

    let confirm_two = harness
        .mutation(
            Method::POST,
            &format!("/v1/poc/matches/{match_id}/confirm-score"),
            team_two_poc.as_str(),
            Some(serde_json::json!({
                "t1_score": detail.t1_score,
                "t2_score": detail.t2_score,
            })),
        )
        .await?;
    assert!(confirm_two.success);
    tracker.set_score_confirmation(match_id, detail.t2_id, detail.t1_score, detail.t2_score);

    let t1_players = players_for_team(&detail, detail.t1_id);
    let t2_players = players_for_team(&detail, detail.t2_id);

    submit_spirit_payload(
        harness,
        tracker,
        team_one_poc.as_str(),
        match_id,
        detail.t2_id,
        detail.t1_id,
        &t2_players,
    )
    .await?;
    submit_spirit_payload(
        harness,
        tracker,
        team_two_poc.as_str(),
        match_id,
        detail.t1_id,
        detail.t2_id,
        &t1_players,
    )
    .await?;

    if include_self_ratings {
        submit_spirit_payload(
            harness,
            tracker,
            team_one_poc.as_str(),
            match_id,
            detail.t1_id,
            detail.t1_id,
            &t1_players,
        )
        .await?;
        submit_spirit_payload(
            harness,
            tracker,
            team_two_poc.as_str(),
            match_id,
            detail.t2_id,
            detail.t2_id,
            &t2_players,
        )
        .await?;
    }

    Ok(())
}

pub(crate) async fn submit_spirit_payload(
    harness: &Harness,
    tracker: &mut TournamentTracker,
    token: &str,
    match_id: i64,
    rated_team_id: i64,
    submitted_by_team_id: i64,
    players: &[MatchPlayerResponse],
) -> TestResult {
    let (payload, total, mvp_player_id, msp_player_id) =
        build_spirit_payload(match_id, rated_team_id, submitted_by_team_id, players);
    let response = harness
        .mutation(
            Method::PUT,
            &format!("/v1/poc/matches/{match_id}/spirit-wfdf"),
            token,
            Some(payload),
        )
        .await?;
    assert!(response.success);
    tracker.record_spirit_submission(
        match_id,
        rated_team_id,
        submitted_by_team_id,
        total,
        mvp_player_id,
        msp_player_id,
    );
    Ok(())
}

pub(crate) fn players_for_team(
    detail: &MatchDetailResponse,
    team_id: i64,
) -> Vec<MatchPlayerResponse> {
    let mut players: Vec<_> = detail
        .players
        .iter()
        .filter(|player| player.team_id == team_id)
        .cloned()
        .collect();
    players.sort_by_key(|player| player.id);
    players
}

fn resolve_target_scores(current_t1: i64, current_t2: i64, outcome: OutcomeKind) -> (i64, i64) {
    match outcome {
        OutcomeKind::T1Win => {
            if current_t1 == 0 && current_t2 == 0 {
                (2, 1)
            } else if current_t1 > current_t2 {
                (current_t1, current_t2)
            } else {
                (current_t2 + 1, current_t2)
            }
        }
        OutcomeKind::T2Win => {
            if current_t1 == 0 && current_t2 == 0 {
                (1, 2)
            } else if current_t2 > current_t1 {
                (current_t1, current_t2)
            } else {
                (current_t1, current_t1 + 1)
            }
        }
        OutcomeKind::Draw => {
            if current_t1 == current_t2 {
                if current_t1 == 0 {
                    (1, 1)
                } else {
                    (current_t1, current_t2)
                }
            } else {
                let target = current_t1.max(current_t2);
                (target, target)
            }
        }
    }
}

fn build_scoring_sequence(
    t1_id: i64,
    t2_id: i64,
    current_t1: i64,
    current_t2: i64,
    target_t1: i64,
    target_t2: i64,
) -> Vec<i64> {
    let mut remaining_t1 = target_t1.saturating_sub(current_t1);
    let mut remaining_t2 = target_t2.saturating_sub(current_t2);
    let mut next_team_is_t1 = remaining_t1 >= remaining_t2;
    let mut sequence = Vec::new();

    while remaining_t1 > 0 || remaining_t2 > 0 {
        if next_team_is_t1 && remaining_t1 > 0 {
            sequence.push(t1_id);
            remaining_t1 -= 1;
        } else if !next_team_is_t1 && remaining_t2 > 0 {
            sequence.push(t2_id);
            remaining_t2 -= 1;
        } else if remaining_t1 > 0 {
            sequence.push(t1_id);
            remaining_t1 -= 1;
        } else {
            sequence.push(t2_id);
            remaining_t2 -= 1;
        }

        next_team_is_t1 = !next_team_is_t1;
    }

    sequence
}

fn offense_team_id(possession: Option<i64>, t1_id: i64, t2_id: i64) -> Option<i64> {
    match possession {
        Some(1) => Some(t1_id),
        Some(2) => Some(t2_id),
        _ => None,
    }
}

fn build_spirit_payload(
    match_id: i64,
    rated_team_id: i64,
    submitted_by_team_id: i64,
    players: &[MatchPlayerResponse],
) -> (serde_json::Value, i64, Option<i64>, Option<i64>) {
    let seed = match_id + rated_team_id + submitted_by_team_id;
    let rules_knowledge = 1 + (seed % 4);
    let fouls_contact = 1 + ((seed / 2 + 1) % 4);
    let fair_mindedness = 1 + ((seed / 3 + 2) % 4);
    let positive_attitude = 1 + ((seed / 4 + 3) % 4);
    let communication = 1 + ((seed / 5 + 4) % 4);
    let total =
        rules_knowledge + fouls_contact + fair_mindedness + positive_attitude + communication;
    let mvp_player_id = players.first().map(|player| player.id);
    let msp_player_id = players
        .get(1)
        .or_else(|| players.first())
        .map(|player| player.id);

    (
        serde_json::json!({
            "team_id": rated_team_id,
            "rules_knowledge": rules_knowledge,
            "fouls_contact": fouls_contact,
            "fair_mindedness": fair_mindedness,
            "positive_attitude": positive_attitude,
            "communication": communication,
            "mvp_player_id": mvp_player_id,
            "msp_player_id": msp_player_id,
        }),
        total,
        mvp_player_id,
        msp_player_id,
    )
}
