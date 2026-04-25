use super::model::{
    LoginResponse, MatchDetailResponse, PlayerStatResponse, ReportingRoundSettingResponse,
    ScheduleGridResponse, ScheduleMatchResponse, ScoreConfirmRowResponse,
    SpiritScoreRowResponse, StandingRowResponse, StatsResponse, TeamMatchResponse,
    TeamResponse, TournamentTracker, UpcomingMatchResponse,
};
use api::{AppState, build_api_only_app, migration};
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use rand::random;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;
use tower::ServiceExt;

type DynError = Box<dyn std::error::Error + Send + Sync>;
pub type TestResult<T = ()> = Result<T, DynError>;

pub const STAFF_PASSWORD: &str = "helloworld";

static TEST_ENV: Once = Once::new();

#[derive(Debug, Clone)]
pub struct Credentials {
    pub super_email: String,
    pub admin_emails: Vec<String>,
    pub poc_by_team: HashMap<i64, String>,
}

pub struct Harness {
    app: axum::Router,
    _db: SqlitePool,
    pub credentials: Credentials,
    temp_dir: PathBuf,
    tokens: HashMap<String, String>,
}

impl Harness {
    pub async fn new(label: &str) -> TestResult<Self> {
        configure_test_env();

        let temp_dir = create_temp_dir(label)?;
        let copied_db_path = copy_database_snapshot(&temp_dir)?;
        let database_url = format!("sqlite:{}", copied_db_path.display());

        let db = migration::init_db_pool(&database_url, false).await?;
        migration::run_migrations(&db).await?;
        migration::verify_migrations(&db).await?;

        let credentials = load_credentials(&db).await?;
        let app = build_api_only_app(AppState {
            db: db.clone(),
            telemetry_enabled: false,
            live_updates: api::helpers::live_updates::LiveUpdates::new(),
        });

        Ok(Self {
            app,
            _db: db,
            credentials,
            temp_dir,
            tokens: HashMap::new(),
        })
    }

    pub async fn load_initial_tracker(&self) -> TestResult<TournamentTracker> {
        let teams = self.get_teams().await?;
        let mut tracker = TournamentTracker::new(&teams);

        for division in [0, 1] {
            let matches = self.get_schedule_matches(division, None, None).await?;
            for schedule_match in &matches {
                tracker.register_schedule_match(division, schedule_match);

                let detail = self.get_match_detail(schedule_match.id).await?;
                tracker.apply_detail(schedule_match.match_type, &detail);

                for row in self.get_match_spirits(schedule_match.id).await? {
                    tracker.record_spirit_row(&row);
                }

                for row in self.get_score_confirmations(schedule_match.id).await? {
                    tracker.record_score_confirmation(&row);
                }
            }
        }

        Ok(tracker)
    }

    pub async fn login(&mut self, email: &str, password: &str) -> TestResult<String> {
        if let Some(token) = self.tokens.get(email) {
            return Ok(token.clone());
        }

        let response: LoginResponse = self
            .request_json(
                Method::POST,
                "/v1/auth/login",
                None,
                Some(serde_json::json!({
                    "email": email,
                    "password": password,
                })),
                StatusCode::OK,
            )
            .await?;

        let token = response.token;
        self.tokens.insert(email.to_string(), token.clone());
        Ok(token)
    }

    pub async fn get_teams(&self) -> TestResult<Vec<TeamResponse>> {
        self.request_json(Method::GET, "/v1/teams", None, None, StatusCode::OK)
            .await
    }

    pub async fn get_stats(&self) -> TestResult<StatsResponse> {
        self.request_json(Method::GET, "/v1/stats", None, None, StatusCode::OK)
            .await
    }

    pub async fn get_player_stats(&self) -> TestResult<Vec<PlayerStatResponse>> {
        self.request_json(Method::GET, "/v1/player-stats", None, None, StatusCode::OK)
            .await
    }

    pub async fn get_match_detail(&self, match_id: i64) -> TestResult<MatchDetailResponse> {
        self.request_json(
            Method::GET,
            &format!("/v1/matches/{match_id}"),
            None,
            None,
            StatusCode::OK,
        )
        .await
    }

    pub async fn get_match_spirits(
        &self,
        match_id: i64,
    ) -> TestResult<Vec<SpiritScoreRowResponse>> {
        self.request_json(
            Method::GET,
            &format!("/v1/matches/{match_id}/spirits"),
            None,
            None,
            StatusCode::OK,
        )
        .await
    }

    pub async fn get_score_confirmations(
        &self,
        match_id: i64,
    ) -> TestResult<Vec<ScoreConfirmRowResponse>> {
        self.request_json(
            Method::GET,
            &format!("/v1/matches/{match_id}/score-confirmations"),
            None,
            None,
            StatusCode::OK,
        )
        .await
    }

    pub async fn get_team_matches(&self, team_id: i64) -> TestResult<Vec<TeamMatchResponse>> {
        self.request_json(
            Method::GET,
            &format!("/v1/teams/{team_id}/matches"),
            None,
            None,
            StatusCode::OK,
        )
        .await
    }

    pub async fn get_standings(&self, division: i64) -> TestResult<Vec<StandingRowResponse>> {
        self.request_json(
            Method::GET,
            &format!("/v1/standings?division={division}"),
            None,
            None,
            StatusCode::OK,
        )
        .await
    }

    pub async fn get_schedule_matches(
        &self,
        division: i64,
        round: Option<i64>,
        status: Option<&str>,
    ) -> TestResult<Vec<ScheduleMatchResponse>> {
        let mut path = format!("/v1/schedule/matches?division={division}");
        if let Some(round) = round {
            path.push_str(&format!("&round={round}"));
        }
        if let Some(status) = status {
            path.push_str(&format!("&status={status}"));
        }

        self.request_json(Method::GET, &path, None, None, StatusCode::OK)
            .await
    }

    pub async fn get_schedule_grid(&self) -> TestResult<ScheduleGridResponse> {
        self.request_json(Method::GET, "/v1/schedule/grid", None, None, StatusCode::OK)
            .await
    }

    pub async fn get_upcoming_matches(
        &self,
        token: &str,
    ) -> TestResult<Vec<UpcomingMatchResponse>> {
        self.request_json(
            Method::GET,
            "/v1/admin/matches",
            Some(token),
            None,
            StatusCode::OK,
        )
        .await
    }

    pub async fn get_reporting_round_settings(
        &self,
        token: &str,
    ) -> TestResult<Vec<ReportingRoundSettingResponse>> {
        self.request_json(
            Method::GET,
            "/v1/admin/reporting-rounds",
            Some(token),
            None,
            StatusCode::OK,
        )
        .await
    }

    pub async fn update_reporting_round_setting(
        &self,
        token: &str,
        round_key: i64,
        is_enabled: bool,
    ) -> TestResult<ReportingRoundSettingResponse> {
        self.request_json(
            Method::PUT,
            &format!("/v1/super/reporting-rounds/{round_key}"),
            Some(token),
            Some(serde_json::json!({ "is_enabled": is_enabled })),
            StatusCode::OK,
        )
        .await
    }

    pub async fn get_opponent_players(
        &self,
        token: &str,
        match_id: i64,
    ) -> TestResult<Vec<super::model::MatchPlayerResponse>> {
        self.request_json(
            Method::GET,
            &format!("/v1/poc/matches/{match_id}/opponent-players"),
            Some(token),
            None,
            StatusCode::OK,
        )
        .await
    }

    pub async fn mutation(
        &self,
        method: Method,
        path: &str,
        token: &str,
        body: Option<serde_json::Value>,
    ) -> TestResult<super::model::MutationResponse> {
        self.request_json(method, path, Some(token), body, StatusCode::OK)
            .await
    }

    pub async fn expect_status(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<serde_json::Value>,
        expected_status: StatusCode,
    ) -> TestResult<serde_json::Value> {
        self.request_json(method, path, token, body, expected_status)
            .await
    }

    async fn request_json<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<serde_json::Value>,
        expected_status: StatusCode,
    ) -> TestResult<T> {
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
            return Err(format!(
                "request {} {} returned {} instead of {} with body {}",
                path,
                status.as_u16(),
                status,
                expected_status,
                body_text
            )
            .into());
        }

        if bytes.is_empty() {
            Ok(serde_json::from_value(serde_json::json!({}))?)
        } else {
            Ok(serde_json::from_slice(&bytes)?)
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temp_dir);
    }
}

fn configure_test_env() {
    TEST_ENV.call_once(|| unsafe {
        std::env::set_var("JWT_SECRET", "default-secret-change-in-production");
        std::env::remove_var("TURNSTILE_SECRET_KEY");
    });
}

fn create_temp_dir(label: &str) -> TestResult<PathBuf> {
    let temp_dir = std::env::temp_dir().join(format!(
        "sakkath-tournament-{label}-{}-{}",
        std::process::id(),
        random::<u64>()
    ));
    fs::create_dir_all(&temp_dir)?;
    Ok(temp_dir)
}

fn copy_database_snapshot(temp_dir: &Path) -> TestResult<PathBuf> {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("api crate should have a repo root parent")
        .join("sakkath.db");
    let destination = temp_dir.join("sakkath.db");

    copy_optional_file(&source, &destination)?;
    copy_optional_file(
        &with_suffix(&source, "-wal"),
        &with_suffix(&destination, "-wal"),
    )?;

    Ok(destination)
}

fn copy_optional_file(source: &Path, destination: &Path) -> TestResult<()> {
    if source.exists() {
        fs::copy(source, destination)?;
    }
    Ok(())
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{}", path.display(), suffix))
}

async fn load_credentials(db: &SqlitePool) -> TestResult<Credentials> {
    let super_email: (String,) =
        sqlx::query_as("SELECT email FROM users WHERE role = 0 AND deleted_at IS NULL LIMIT 1")
            .fetch_one(db)
            .await?;

    let admin_emails: Vec<String> = sqlx::query_as::<_, (String,)>(
        "SELECT email FROM users WHERE role = 1 AND deleted_at IS NULL ORDER BY email",
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|row| row.0)
    .collect();

    let poc_by_team: HashMap<i64, String> = sqlx::query_as::<_, (i64, String)>(
        "SELECT team_id, email FROM users WHERE role = 3 AND team_id IS NOT NULL AND deleted_at IS NULL ORDER BY team_id",
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .collect();

    Ok(Credentials {
        super_email: super_email.0,
        admin_emails,
        poc_by_team,
    })
}
