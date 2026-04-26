use crate::{AppState, build_api_only_app};
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use chrono::{Duration, Utc};
use jsonwebtoken::{EncodingKey, Header, encode};
use rand::{Rng, SeedableRng, rngs::StdRng, seq::SliceRandom};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::json;
use sqlx::SqlitePool;
use std::io;
use tower::ServiceExt;

const ADMIN_ONE_EMAIL: &str = "admin1@sakkath.com";
const ADMIN_TWO_EMAIL: &str = "admin2@sakkath.com";
const SUPER_EMAIL: &str = "super@sakkath.com";
const CLI_JWT_SECRET: &str = "mock-database-cli-secret";
const MAX_PARTIAL_GAMES: usize = 15;

type DynError = Box<dyn std::error::Error + Send + Sync>;
type MockResult<T = ()> = Result<T, DynError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MockRoundTarget {
    Swiss(i64),
    Playoffs,
    Finals,
}

impl MockRoundTarget {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "1" => Ok(Self::Swiss(1)),
            "2" => Ok(Self::Swiss(2)),
            "3" => Ok(Self::Swiss(3)),
            "4" => Ok(Self::Swiss(4)),
            "5" => Ok(Self::Swiss(5)),
            "6" => Ok(Self::Swiss(6)),
            "P" | "PLAYOFF" | "PLAYOFFS" => Ok(Self::Playoffs),
            "F" | "FINAL" | "FINALS" => Ok(Self::Finals),
            _ => Err("round must be one of 1, 2, 3, 4, 5, 6, P, or F".to_string()),
        }
    }

    pub fn round_key(self) -> i64 {
        match self {
            Self::Swiss(round) => round,
            Self::Playoffs => 1001,
            Self::Finals => 1002,
        }
    }

    pub fn label(self) -> String {
        stage_label(self.round_key())
    }

    fn order(self) -> usize {
        stage_order(self.round_key())
    }

    fn prior_stage_keys(self) -> Vec<i64> {
        match self {
            Self::Swiss(round) => (1..round).collect(),
            Self::Playoffs => (1..=crate::OPEN_ROUNDS.max(crate::WOMEN_ROUNDS)).collect(),
            Self::Finals => {
                let mut keys: Vec<i64> =
                    (1..=crate::OPEN_ROUNDS.max(crate::WOMEN_ROUNDS)).collect();
                keys.push(1001);
                keys
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct MockDatabaseRequest {
    pub target: MockRoundTarget,
    pub games: Option<usize>,
}

impl MockDatabaseRequest {
    pub fn new(target: MockRoundTarget, games: Option<usize>) -> Result<Self, String> {
        if let Some(games) = games
            && (games == 0 || games > MAX_PARTIAL_GAMES)
        {
            return Err(format!(
                "games must be an integer between 1 and {MAX_PARTIAL_GAMES}"
            ));
        }

        Ok(Self { target, games })
    }
}

#[derive(Clone, Debug)]
pub struct MockDatabaseSummary {
    pub target_label: String,
    pub newly_completed_matches: usize,
    pub post_match_updates: usize,
    pub target_stage_completed: usize,
    pub target_stage_total: usize,
}

#[derive(Clone)]
struct RouterClient {
    app: axum::Router,
}

impl RouterClient {
    fn new(pool: &SqlitePool) -> Self {
        Self {
            app: build_api_only_app(AppState {
                db: pool.clone(),
                telemetry_enabled: false,
                live_updates: crate::helpers::live_updates::LiveUpdates::new(),
            }),
        }
    }

    async fn request_json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<serde_json::Value>,
        expected_status: StatusCode,
    ) -> MockResult<T> {
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            builder = builder.header("Authorization", format!("Bearer {token}"));
        }

        let request = if let Some(body) = body {
            builder
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?
        } else {
            builder.body(Body::empty())?
        };

        let response = self.app.clone().oneshot(request).await?;
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;

        if status != expected_status {
            let body_text = String::from_utf8_lossy(&bytes).to_string();
            return Err(error(format!(
                "request {} {} returned {} instead of {} with body {}",
                path,
                status.as_u16(),
                status,
                expected_status,
                body_text
            )));
        }

        if bytes.is_empty() {
            Ok(serde_json::from_value(serde_json::json!({}))?)
        } else {
            Ok(serde_json::from_slice(&bytes)?)
        }
    }

    async fn get_schedule_matches(
        &self,
        division: i64,
        round_key: i64,
    ) -> MockResult<Vec<ScheduleMatchResponse>> {
        self.request_json(
            Method::GET,
            &format!("/v1/schedule/matches?division={division}&round={round_key}"),
            None,
            None,
            StatusCode::OK,
        )
        .await
    }

    async fn get_match_detail(&self, match_id: i64) -> MockResult<MatchDetailResponse> {
        self.request_json(
            Method::GET,
            &format!("/v1/matches/{match_id}"),
            None,
            None,
            StatusCode::OK,
        )
        .await
    }

    async fn get_match_spirits(&self, match_id: i64) -> MockResult<Vec<SpiritScoreRowResponse>> {
        self.request_json(
            Method::GET,
            &format!("/v1/matches/{match_id}/spirits"),
            None,
            None,
            StatusCode::OK,
        )
        .await
    }

    async fn get_score_confirmations(
        &self,
        match_id: i64,
    ) -> MockResult<Vec<ScoreConfirmRowResponse>> {
        self.request_json(
            Method::GET,
            &format!("/v1/matches/{match_id}/score-confirmations"),
            None,
            None,
            StatusCode::OK,
        )
        .await
    }

    async fn mutation(
        &self,
        method: Method,
        path: &str,
        token: &str,
        body: Option<serde_json::Value>,
    ) -> MockResult<MutationResponse> {
        self.request_json(method, path, Some(token), body, StatusCode::OK)
            .await
    }
}

struct StaffTokens {
    admin_one: String,
    admin_two: String,
    super_admin: String,
}

impl StaffTokens {
    fn reporter_token(&self, index: usize) -> &str {
        if index.is_multiple_of(2) {
            self.admin_one.as_str()
        } else {
            self.admin_two.as_str()
        }
    }

    async fn load(pool: &SqlitePool) -> MockResult<Self> {
        Ok(Self {
            admin_one: build_staff_token(pool, ADMIN_ONE_EMAIL, 1).await?,
            admin_two: build_staff_token(pool, ADMIN_TWO_EMAIL, 1).await?,
            super_admin: build_staff_token(pool, SUPER_EMAIL, 0).await?,
        })
    }
}

enum StageMode {
    CompleteAll,
    UpTo(usize),
}

struct StageExecutionSummary {
    newly_completed_matches: usize,
    post_match_updates: usize,
    completed_matches: usize,
    total_matches: usize,
}

#[derive(Deserialize, Clone)]
struct ScheduleMatchResponse {
    id: i64,
    possession: Option<i64>,
}

#[derive(Deserialize, Clone)]
struct MatchPlayerResponse {
    id: i64,
    team_id: i64,
}

#[derive(Deserialize, Clone)]
struct MatchDetailResponse {
    t1_id: i64,
    t2_id: i64,
    t1_score: i64,
    t2_score: i64,
    possession: Option<i64>,
    match_type: i64,
    players: Vec<MatchPlayerResponse>,
}

#[derive(Deserialize)]
struct ScoreConfirmRowResponse {
    team_id: i64,
    t1_score: i64,
    t2_score: i64,
}

#[derive(Deserialize)]
struct SpiritScoreRowResponse {
    team_id: i64,
    submitted_by_team_id: i64,
    mvp_player_id: Option<i64>,
    msp_player_id: Option<i64>,
}

#[derive(Deserialize)]
struct ReportingRoundSettingResponse {
    is_enabled: bool,
}

#[derive(Deserialize)]
struct MutationResponse {
    success: bool,
    message: Option<String>,
    possession: Option<i64>,
}

pub async fn mock_existing_database(
    pool: &SqlitePool,
    request: MockDatabaseRequest,
) -> MockResult<MockDatabaseSummary> {
    ensure_cli_jwt_secret();
    validate_existing_progress(pool, request.target).await?;

    let client = RouterClient::new(pool);
    let staff = StaffTokens::load(pool).await?;
    let mut rng = StdRng::seed_from_u64(rand::random::<u64>());

    let mut summary = MockDatabaseSummary {
        target_label: request.target.label(),
        newly_completed_matches: 0,
        post_match_updates: 0,
        target_stage_completed: 0,
        target_stage_total: 0,
    };

    for round_key in request.target.prior_stage_keys() {
        let stage_summary = process_stage(
            pool,
            &client,
            &staff,
            round_key,
            StageMode::CompleteAll,
            &mut rng,
        )
        .await?;
        summary.newly_completed_matches += stage_summary.newly_completed_matches;
        summary.post_match_updates += stage_summary.post_match_updates;
    }

    let target_summary = process_stage(
        pool,
        &client,
        &staff,
        request.target.round_key(),
        match request.games {
            Some(games) => StageMode::UpTo(games),
            None => StageMode::CompleteAll,
        },
        &mut rng,
    )
    .await?;

    summary.newly_completed_matches += target_summary.newly_completed_matches;
    summary.post_match_updates += target_summary.post_match_updates;
    summary.target_stage_completed = target_summary.completed_matches;
    summary.target_stage_total = target_summary.total_matches;

    Ok(summary)
}

async fn process_stage(
    pool: &SqlitePool,
    client: &RouterClient,
    staff: &StaffTokens,
    round_key: i64,
    mode: StageMode,
    rng: &mut StdRng,
) -> MockResult<StageExecutionSummary> {
    enable_reporting_round(client, staff.super_admin.as_str(), round_key).await?;

    let stage_matches = ensure_stage_matches(pool, client, staff, round_key).await?;
    let completed_before = stage_matches
        .iter()
        .filter(|m| is_match_complete(m))
        .count();
    let mut pending_matches: Vec<_> = stage_matches
        .iter()
        .filter(|m| !is_match_complete(m))
        .cloned()
        .collect();

    let selected_matches = match mode {
        StageMode::CompleteAll => pending_matches,
        StageMode::UpTo(target_completed) => {
            if completed_before > target_completed {
                return Err(error(format!(
                    "{} already has {completed_before} completed matches, which is greater than the requested {target_completed}",
                    stage_label(round_key)
                )));
            }

            let needed = target_completed.saturating_sub(completed_before);
            if needed > pending_matches.len() {
                return Err(error(format!(
                    "{} only has {} unfinished matches available, but {} were requested",
                    stage_label(round_key),
                    pending_matches.len(),
                    needed
                )));
            }

            pending_matches.shuffle(rng);
            pending_matches.truncate(needed);
            pending_matches
        }
    };

    let mut newly_completed_matches = 0usize;
    for schedule_match in selected_matches {
        finish_match(pool, client, staff, schedule_match.id, rng).await?;
        newly_completed_matches += 1;
    }

    let refreshed_stage_matches = fetch_stage_matches(client, round_key).await?;
    let mut post_match_updates = 0usize;
    let mut completed_matches = 0usize;
    for schedule_match in &refreshed_stage_matches {
        if is_match_complete(schedule_match) {
            completed_matches += 1;
            post_match_updates += ensure_post_match_data(client, staff, schedule_match.id).await?;
        }
    }

    Ok(StageExecutionSummary {
        newly_completed_matches,
        post_match_updates,
        completed_matches,
        total_matches: refreshed_stage_matches.len(),
    })
}

async fn ensure_stage_matches(
    pool: &SqlitePool,
    client: &RouterClient,
    staff: &StaffTokens,
    round_key: i64,
) -> MockResult<Vec<ScheduleMatchResponse>> {
    let mut stage_matches = Vec::new();
    for division in [0, 1] {
        stage_matches.extend(
            ensure_stage_matches_for_division(pool, client, staff, division, round_key).await?,
        );
    }

    if stage_matches.is_empty() {
        return Err(error(format!(
            "no matches available for {} across either division",
            stage_label(round_key)
        )));
    }

    Ok(stage_matches)
}

async fn ensure_stage_matches_for_division(
    pool: &SqlitePool,
    client: &RouterClient,
    staff: &StaffTokens,
    division: i64,
    round_key: i64,
) -> MockResult<Vec<ScheduleMatchResponse>> {
    let mut stage_matches = client.get_schedule_matches(division, round_key).await?;
    if !stage_matches.is_empty() {
        return Ok(stage_matches);
    }

    if round_key < 1000 {
        let response = client
            .mutation(
                Method::POST,
                &format!("/v1/admin/schedule/{division}/generate"),
                staff.admin_one.as_str(),
                None,
            )
            .await?;
        ensure_success(
            &response,
            &format!(
                "generate {} for {} division",
                stage_label(round_key),
                division_label(division)
            ),
        )?;
    } else {
        let _ =
            crate::controllers::scheduling::auto_advance_division_if_ready(pool, division).await;
    }

    stage_matches = client.get_schedule_matches(division, round_key).await?;
    if stage_matches.is_empty() {
        return Err(error(format!(
            "failed to materialize {} for {} division",
            stage_label(round_key),
            division_label(division)
        )));
    }

    Ok(stage_matches)
}

async fn fetch_stage_matches(
    client: &RouterClient,
    round_key: i64,
) -> MockResult<Vec<ScheduleMatchResponse>> {
    let mut stage_matches = Vec::new();
    for division in [0, 1] {
        stage_matches.extend(client.get_schedule_matches(division, round_key).await?);
    }
    Ok(stage_matches)
}

async fn enable_reporting_round(
    client: &RouterClient,
    super_token: &str,
    round_key: i64,
) -> MockResult {
    let response: ReportingRoundSettingResponse = client
        .request_json(
            Method::PUT,
            &format!("/v1/super/reporting-rounds/{round_key}"),
            Some(super_token),
            Some(json!({ "is_enabled": true })),
            StatusCode::OK,
        )
        .await?;

    if !response.is_enabled {
        return Err(error(format!(
            "failed to enable reporting for {}",
            stage_label(round_key)
        )));
    }

    Ok(())
}

async fn finish_match(
    pool: &SqlitePool,
    client: &RouterClient,
    staff: &StaffTokens,
    match_id: i64,
    rng: &mut StdRng,
) -> MockResult {
    let detail = client.get_match_detail(match_id).await?;
    if detail.possession.unwrap_or(0) >= 3 {
        return Ok(());
    }

    let t1_players = players_for_team(&detail, detail.t1_id)?;
    let t2_players = players_for_team(&detail, detail.t2_id)?;
    let (t1_rank, t2_rank) = fetch_init_ranks(pool, detail.t1_id, detail.t2_id).await?;
    let t1_wins = choose_t1_winner(detail.match_type, t1_rank, t2_rank, rng);
    let (target_t1_score, target_t2_score) = build_target_scores(&detail, t1_wins, rng);
    let scoring_sequence = build_scoring_sequence(
        detail.t1_id,
        detail.t2_id,
        detail.t1_score,
        detail.t2_score,
        target_t1_score,
        target_t2_score,
    );

    let mut possession = detail.possession;
    let mut reporter_index = match_id as usize;

    if possession.is_none() {
        let first_scoring_team = scoring_sequence.first().copied().ok_or_else(|| {
            error(format!(
                "{} match {match_id} did not produce a valid scoring sequence",
                stage_label(detail.match_type)
            ))
        })?;
        let start_possession = if first_scoring_team == detail.t1_id {
            1
        } else {
            2
        };
        let response = client
            .mutation(
                Method::POST,
                &format!("/v1/admin/matches/{match_id}/start"),
                staff.reporter_token(reporter_index),
                Some(json!({ "possession": start_possession })),
            )
            .await?;
        ensure_success(&response, &format!("start match {match_id}"))?;
        possession = response.possession;
        reporter_index += 1;
    }

    for scoring_team in scoring_sequence {
        while offense_team_id(possession, detail.t1_id, detail.t2_id) != Some(scoring_team) {
            let response = client
                .mutation(
                    Method::POST,
                    &format!("/v1/admin/matches/{match_id}/switch-possession"),
                    staff.reporter_token(reporter_index),
                    None,
                )
                .await?;
            ensure_success(
                &response,
                &format!("switch possession for match {match_id}"),
            )?;
            possession = response.possession;
            reporter_index += 1;
        }

        let players = if scoring_team == detail.t1_id {
            &t1_players
        } else {
            &t2_players
        };
        let (goal_player_id, assist_player_id) = choose_scoring_players(players, rng)?;

        let assist_response = client
            .mutation(
                Method::POST,
                &format!("/v1/admin/matches/{match_id}/event"),
                staff.reporter_token(reporter_index),
                Some(json!({
                    "player_id": assist_player_id,
                    "event_type": 1,
                })),
            )
            .await?;
        ensure_success(
            &assist_response,
            &format!("record assist for match {match_id}"),
        )?;
        reporter_index += 1;

        let goal_response = client
            .mutation(
                Method::POST,
                &format!("/v1/admin/matches/{match_id}/event"),
                staff.reporter_token(reporter_index),
                Some(json!({
                    "player_id": goal_player_id,
                    "event_type": 0,
                })),
            )
            .await?;
        ensure_success(&goal_response, &format!("record goal for match {match_id}"))?;
        possession = goal_response.possession;
        reporter_index += 1;
    }

    let final_detail = client.get_match_detail(match_id).await?;
    if final_detail.t1_score != target_t1_score || final_detail.t2_score != target_t2_score {
        return Err(error(format!(
            "match {match_id} ended at {}-{} instead of the target {}-{}",
            final_detail.t1_score, final_detail.t2_score, target_t1_score, target_t2_score
        )));
    }

    if final_detail.possession.unwrap_or(0) < 3 {
        let response = client
            .mutation(
                Method::POST,
                &format!("/v1/admin/matches/{match_id}/end"),
                staff.reporter_token(reporter_index),
                None,
            )
            .await?;
        ensure_success(&response, &format!("end match {match_id}"))?;
    }

    Ok(())
}

async fn ensure_post_match_data(
    client: &RouterClient,
    staff: &StaffTokens,
    match_id: i64,
) -> MockResult<usize> {
    let detail = client.get_match_detail(match_id).await?;
    if detail.possession.unwrap_or(0) < 3 {
        return Ok(0);
    }

    let mut updates = 0usize;
    let confirmations = client.get_score_confirmations(match_id).await?;

    if !has_confirmation(
        &confirmations,
        detail.t1_id,
        detail.t1_score,
        detail.t2_score,
    ) {
        let response = client
            .mutation(
                Method::POST,
                &format!("/v1/poc/matches/{match_id}/confirm-score"),
                staff.admin_one.as_str(),
                Some(json!({
                    "team_id": detail.t1_id,
                    "t1_score": detail.t1_score,
                    "t2_score": detail.t2_score,
                })),
            )
            .await?;
        ensure_success(
            &response,
            &format!("confirm score for match {match_id} team {}", detail.t1_id),
        )?;
        updates += 1;
    }

    if !has_confirmation(
        &confirmations,
        detail.t2_id,
        detail.t1_score,
        detail.t2_score,
    ) {
        let response = client
            .mutation(
                Method::POST,
                &format!("/v1/poc/matches/{match_id}/confirm-score"),
                staff.admin_two.as_str(),
                Some(json!({
                    "team_id": detail.t2_id,
                    "t1_score": detail.t1_score,
                    "t2_score": detail.t2_score,
                })),
            )
            .await?;
        ensure_success(
            &response,
            &format!("confirm score for match {match_id} team {}", detail.t2_id),
        )?;
        updates += 1;
    }

    let spirit_rows = client.get_match_spirits(match_id).await?;
    let t1_players = players_for_team(&detail, detail.t1_id)?;
    let t2_players = players_for_team(&detail, detail.t2_id)?;

    if needs_spirit_submission(&spirit_rows, detail.t2_id, detail.t1_id) {
        let payload = build_spirit_payload(match_id, detail.t2_id, detail.t1_id, &t2_players);
        let response = client
            .mutation(
                Method::PUT,
                &format!("/v1/poc/matches/{match_id}/spirit-wfdf"),
                staff.admin_one.as_str(),
                Some(payload),
            )
            .await?;
        ensure_success(
            &response,
            &format!(
                "submit spirit for match {match_id} rated team {}",
                detail.t2_id
            ),
        )?;
        updates += 1;
    }

    if needs_spirit_submission(&spirit_rows, detail.t1_id, detail.t2_id) {
        let payload = build_spirit_payload(match_id, detail.t1_id, detail.t2_id, &t1_players);
        let response = client
            .mutation(
                Method::PUT,
                &format!("/v1/poc/matches/{match_id}/spirit-wfdf"),
                staff.admin_two.as_str(),
                Some(payload),
            )
            .await?;
        ensure_success(
            &response,
            &format!(
                "submit spirit for match {match_id} rated team {}",
                detail.t1_id
            ),
        )?;
        updates += 1;
    }

    Ok(updates)
}

async fn fetch_init_ranks(pool: &SqlitePool, t1_id: i64, t2_id: i64) -> MockResult<(i64, i64)> {
    let rows: Vec<(i64, i64)> =
        sqlx::query_as("SELECT id, init_rank FROM teams WHERE id = ? OR id = ?")
            .bind(t1_id)
            .bind(t2_id)
            .fetch_all(pool)
            .await?;

    let mut t1_rank = None;
    let mut t2_rank = None;
    for (team_id, init_rank) in rows {
        if team_id == t1_id {
            t1_rank = Some(init_rank);
        } else if team_id == t2_id {
            t2_rank = Some(init_rank);
        }
    }

    Ok((
        t1_rank.ok_or_else(|| error(format!("missing init rank for team {t1_id}")))?,
        t2_rank.ok_or_else(|| error(format!("missing init rank for team {t2_id}")))?,
    ))
}

async fn validate_existing_progress(pool: &SqlitePool, target: MockRoundTarget) -> MockResult {
    let completed_types: Vec<(i64, i64)> = sqlx::query_as(
        r#"SELECT type, COUNT(*)
           FROM matches
           WHERE deleted_at IS NULL AND possession >= 3
           GROUP BY type"#,
    )
    .fetch_all(pool)
    .await?;

    for (match_type, completed_count) in completed_types {
        if stage_order(match_type) > target.order() {
            return Err(error(format!(
                "database already has {completed_count} completed matches in {}. mock-database only moves forward",
                stage_label(match_type)
            )));
        }
    }

    Ok(())
}

async fn build_staff_token(
    pool: &SqlitePool,
    email: &str,
    expected_role: i64,
) -> MockResult<String> {
    let user: Option<(i64, String, i64)> =
        sqlx::query_as("SELECT id, email, role FROM users WHERE email = ? AND deleted_at IS NULL")
            .bind(email)
            .fetch_optional(pool)
            .await?;

    let (user_id, email, role) = user.ok_or_else(|| {
        error(format!(
            "required staff account {email} is missing; seed the default staff before running mock-database"
        ))
    })?;
    if role != expected_role {
        return Err(error(format!(
            "staff account {email} has role {role}, expected {expected_role}"
        )));
    }

    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;
    let claims = crate::controllers::user::Claims {
        user_id,
        email,
        exp: expiration,
    };
    let secret = std::env::var("JWT_SECRET")?;

    Ok(encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )?)
}

fn ensure_cli_jwt_secret() {
    let secret_missing = std::env::var("JWT_SECRET")
        .map(|value| value.trim().is_empty())
        .unwrap_or(true);
    if secret_missing {
        unsafe {
            std::env::set_var("JWT_SECRET", CLI_JWT_SECRET);
        }
    }
}

fn choose_t1_winner(match_type: i64, t1_rank: i64, t2_rank: i64, rng: &mut StdRng) -> bool {
    let favorite_is_t1 = t1_rank <= t2_rank;
    let gap = t1_rank.abs_diff(t2_rank).min(6) as i64;
    let base_percent = if match_type >= 1000 { 60 } else { 54 };
    let favorite_percent = (base_percent + gap * 5).clamp(54, 86);
    let roll = rng.random_range(0..100) as i64;

    if favorite_is_t1 {
        roll < favorite_percent
    } else {
        roll >= favorite_percent
    }
}

fn build_target_scores(
    detail: &MatchDetailResponse,
    t1_wins: bool,
    rng: &mut StdRng,
) -> (i64, i64) {
    let winner_floor = if detail.match_type >= 1000 { 10 } else { 8 };
    let winner_ceiling = if detail.match_type >= 1000 { 13 } else { 12 };
    let current_max = detail.t1_score.max(detail.t2_score);
    let winner_score = (rng.random_range(winner_floor..=winner_ceiling)).max(current_max + 1);

    if t1_wins {
        let loser_score = choose_loser_score(detail.t2_score, winner_score, rng);
        (winner_score, loser_score)
    } else {
        let loser_score = choose_loser_score(detail.t1_score, winner_score, rng);
        (loser_score, winner_score)
    }
}

fn choose_loser_score(current_score: i64, winner_score: i64, rng: &mut StdRng) -> i64 {
    let desired_margin = rng.random_range(1..=4) as i64;
    let preferred_score = winner_score.saturating_sub(desired_margin);
    preferred_score.clamp(current_score, winner_score - 1)
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

fn players_for_team(
    detail: &MatchDetailResponse,
    team_id: i64,
) -> MockResult<Vec<MatchPlayerResponse>> {
    let mut players: Vec<_> = detail
        .players
        .iter()
        .filter(|player| player.team_id == team_id)
        .cloned()
        .collect();
    players.sort_by_key(|player| player.id);

    if players.is_empty() {
        return Err(error(format!(
            "match between {} and {} has no eligible players for team {team_id}",
            detail.t1_id, detail.t2_id
        )));
    }

    Ok(players)
}

fn choose_scoring_players(
    players: &[MatchPlayerResponse],
    rng: &mut StdRng,
) -> MockResult<(i64, i64)> {
    if players.is_empty() {
        return Err(error("cannot choose scoring players from an empty roster"));
    }

    let goal_index = rng.random_range(0..players.len());
    let assist_index = if players.len() == 1 {
        goal_index
    } else {
        (goal_index + 1 + rng.random_range(0..players.len() - 1)) % players.len()
    };

    Ok((players[goal_index].id, players[assist_index].id))
}

fn has_confirmation(
    confirmations: &[ScoreConfirmRowResponse],
    team_id: i64,
    t1_score: i64,
    t2_score: i64,
) -> bool {
    confirmations
        .iter()
        .any(|row| row.team_id == team_id && row.t1_score == t1_score && row.t2_score == t2_score)
}

fn needs_spirit_submission(
    spirit_rows: &[SpiritScoreRowResponse],
    rated_team_id: i64,
    submitted_by_team_id: i64,
) -> bool {
    !spirit_rows.iter().any(|row| {
        row.team_id == rated_team_id
            && row.submitted_by_team_id == submitted_by_team_id
            && row.mvp_player_id.is_some()
            && row.msp_player_id.is_some()
    })
}

fn build_spirit_payload(
    match_id: i64,
    rated_team_id: i64,
    submitted_by_team_id: i64,
    players: &[MatchPlayerResponse],
) -> serde_json::Value {
    let seed = match_id + rated_team_id + submitted_by_team_id;
    let rules_knowledge = 1 + (seed % 4);
    let fouls_contact = 1 + ((seed / 2 + 1) % 4);
    let fair_mindedness = 1 + ((seed / 3 + 2) % 4);
    let positive_attitude = 1 + ((seed / 4 + 3) % 4);
    let communication = 1 + ((seed / 5 + 4) % 4);
    let mvp_player_id = players.first().map(|player| player.id);
    let msp_player_id = players
        .get(1)
        .or_else(|| players.first())
        .map(|player| player.id);

    json!({
        "team_id": rated_team_id,
        "submitted_by_team_id": submitted_by_team_id,
        "rules_knowledge": rules_knowledge,
        "fouls_contact": fouls_contact,
        "fair_mindedness": fair_mindedness,
        "positive_attitude": positive_attitude,
        "communication": communication,
        "mvp_player_id": mvp_player_id,
        "msp_player_id": msp_player_id,
    })
}

fn is_match_complete(schedule_match: &ScheduleMatchResponse) -> bool {
    schedule_match.possession.unwrap_or(0) >= 3
}

fn ensure_success(response: &MutationResponse, action: &str) -> MockResult {
    if response.success {
        return Ok(());
    }

    Err(error(format!(
        "{} failed{}",
        action,
        response
            .message
            .as_deref()
            .map(|message| format!(": {message}"))
            .unwrap_or_default()
    )))
}

fn stage_order(match_type: i64) -> usize {
    match match_type {
        1..=6 => match_type as usize,
        1001 => 7,
        1002 => 8,
        _ => usize::MAX,
    }
}

fn stage_label(match_type: i64) -> String {
    match match_type {
        1..=6 => format!("Round {match_type}"),
        1001 => "Playoffs".to_string(),
        1002 => "Finals".to_string(),
        _ => format!("Stage {match_type}"),
    }
}

fn division_label(division: i64) -> &'static str {
    match division {
        0 => "Open",
        1 => "Women",
        _ => "Unknown",
    }
}

fn error(message: impl Into<String>) -> DynError {
    io::Error::other(message.into()).into()
}
