use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware as axum_middleware;
use axum::response::IntoResponse;
use axum::routing::{get, get_service};
use std::net::SocketAddr;
use std::process;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

use api::{AppState, controllers, helpers, middleware, migration, routes, telemetry};
use axum::Router;
use sqlx::SqlitePool;

enum CliCommand {
    Serve,
    CreateDatabase,
    SeedDatabase(migration::SeedSource),
    MockDatabase(helpers::mock_seed::MockDatabaseRequest),
}

async fn serve_handbook_markdown(filename: &'static str) -> impl IntoResponse {
    match tokio::fs::read_to_string(format!("../ui/{filename}")).await {
        Ok(content) => (
            [
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/markdown; charset=utf-8"),
                ),
                (
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("no-store, no-cache, must-revalidate, max-age=0"),
                ),
                (header::PRAGMA, HeaderValue::from_static("no-cache")),
                (header::EXPIRES, HeaderValue::from_static("0")),
            ],
            content,
        )
            .into_response(),
        Err(error) => {
            tracing::warn!("Failed to serve handbook markdown {filename}: {error}");
            StatusCode::NOT_FOUND.into_response()
        }
    }
}

fn seed_database_csv_usage() -> &'static str {
    "Usage: sakkath-api seed-database csv [path] [--replace_password true|false]"
}

fn mock_database_usage() -> &'static str {
    "Usage: sakkath-api mock-database <round> [games]\n  round: 1..6, P, or F\n  games: optional integer between 1 and the stage total (P max 6, swiss/finals max 16)"
}

fn parse_bool_flag(value: &str, flag_name: &str) -> Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!(
            "Invalid value `{value}` for {flag_name}. {}",
            seed_database_csv_usage()
        )),
    }
}

fn parse_seed_database_csv_command(args: &[String]) -> Result<CliCommand, String> {
    let mut path = None;
    let mut replace_password = false;
    let mut index = 2;

    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--replace_password" | "replace_password" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    format!(
                        "Missing value for replace_password. {}",
                        seed_database_csv_usage()
                    )
                })?;
                replace_password = parse_bool_flag(value, "replace_password")?;
                index += 2;
            }
            _ if arg.starts_with("--replace_password=") => {
                let value = arg.trim_start_matches("--replace_password=");
                replace_password = parse_bool_flag(value, "replace_password")?;
                index += 1;
            }
            _ if path.is_none() => {
                path = Some(args[index].clone());
                index += 1;
            }
            _ => {
                return Err(format!(
                    "Unexpected argument `{arg}`. {}",
                    seed_database_csv_usage()
                ));
            }
        }
    }

    Ok(CliCommand::SeedDatabase(migration::SeedSource::TeamsCsv {
        path,
        replace_password,
    }))
}

fn parse_cli_command() -> Result<CliCommand, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("create-database") => Ok(CliCommand::CreateDatabase),
        Some("seed-database") => match args.get(1).map(String::as_str) {
            Some("csv") => parse_seed_database_csv_command(&args),
            _ => Ok(CliCommand::SeedDatabase(migration::SeedSource::MockData)),
        },
        Some("mock-database") => {
            if args.len() < 2 || args.len() > 3 {
                return Err(mock_database_usage().to_string());
            }

            let target = helpers::mock_seed::MockRoundTarget::parse(&args[1])?;
            let games = match args.get(2) {
                Some(raw_games) => Some(raw_games.parse::<usize>().map_err(|_| {
                    format!(
                        "Invalid games value `{raw_games}`. {}",
                        mock_database_usage()
                    )
                })?),
                None => None,
            };

            Ok(CliCommand::MockDatabase(
                helpers::mock_seed::MockDatabaseRequest::new(target, games)?,
            ))
        }
        _ => Ok(CliCommand::Serve),
    }
}

fn exit_with_error(message: &str) -> ! {
    tracing::error!("{}", message);
    eprintln!("{}", message);
    process::exit(1);
}

async fn connect_db(database_url: &str, create_if_missing: bool) -> SqlitePool {
    migration::init_db_pool(database_url, create_if_missing)
        .await
        .unwrap_or_else(|err| exit_with_error(&format!("Failed to connect to database: {err}")))
}

async fn run_database_setup(database_url: &str, seed_source: Option<migration::SeedSource>) {
    let db_pool = connect_db(database_url, true).await;

    migration::run_migrations(&db_pool)
        .await
        .unwrap_or_else(|err| exit_with_error(&format!("Failed to run migrations: {err}")));

    migration::verify_migrations(&db_pool)
        .await
        .unwrap_or_else(|err| exit_with_error(&format!("Failed to verify migrations: {err}")));

    if let Some(source) = seed_source {
        migration::seed_database(&db_pool, source)
            .await
            .unwrap_or_else(|err| exit_with_error(&format!("Failed to seed database: {err}")));
        tracing::info!("Database seeded successfully");
    } else {
        tracing::info!("Database created and migrated successfully");
    }
}

async fn run_mock_database(database_url: &str, request: helpers::mock_seed::MockDatabaseRequest) {
    let db_pool = connect_db(database_url, false).await;

    migration::run_migrations(&db_pool)
        .await
        .unwrap_or_else(|err| exit_with_error(&format!("Failed to run migrations: {err}")));

    migration::verify_migrations(&db_pool)
        .await
        .unwrap_or_else(|err| exit_with_error(&format!("Failed to verify migrations: {err}")));

    // Let mock-database invalidate the same shared Redis cache that serve mode uses.
    helpers::cache::init_redis().await;

    let summary = helpers::mock_seed::mock_existing_database(&db_pool, request)
        .await
        .unwrap_or_else(|err| exit_with_error(&format!("Failed to mock database: {err}")));

    tracing::info!(
        target = %summary.target_label,
        newly_completed_matches = summary.newly_completed_matches,
        post_match_updates = summary.post_match_updates,
        target_stage_completed = summary.target_stage_completed,
        target_stage_total = summary.target_stage_total,
        "Database mock reporting completed successfully"
    );
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .with_max_level(tracing::Level::INFO)
        .init();

    let release_mode = std::env::var("release")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";
    let telemetry_enabled = std::env::var("emit_telemetry")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";

    let cli_command = parse_cli_command().unwrap_or_else(|err| exit_with_error(&err));

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "9000".to_string())
        .parse()
        .expect("PORT must be a number");

    tracing::info!(
        "Starting server in {} mode on port {}",
        if release_mode {
            "RELEASE"
        } else {
            "DEVELOPMENT"
        },
        port
    );

    let database_url = migration::resolve_database_url()
        .unwrap_or_else(|err| exit_with_error(&format!("Failed to resolve database path: {err}")));

    tracing::info!("Using database URL: {}", database_url);

    if let Some(db_path) = migration::sqlite_path_from_url(&database_url) {
        tracing::info!("Database path: {}", db_path.display());
    }

    match cli_command {
        CliCommand::CreateDatabase => {
            run_database_setup(&database_url, None).await;
            return;
        }
        CliCommand::SeedDatabase(source) => {
            if !migration::is_sqlite_file_present(&database_url) {
                exit_with_error(
                    "Database file is missing. Run `sakkath-api create-database` first.",
                );
            }

            run_database_setup(&database_url, Some(source)).await;
            return;
        }
        CliCommand::MockDatabase(request) => {
            if !migration::is_sqlite_file_present(&database_url) {
                exit_with_error(
                    "Database file is missing. Run `sakkath-api create-database` first, then seed it before using `sakkath-api mock-database`.",
                );
            }

            run_mock_database(&database_url, request).await;
            return;
        }
        CliCommand::Serve => {}
    }

    helpers::auth::ensure_jwt_secret_configured()
        .unwrap_or_else(|message| exit_with_error(&message));

    if !migration::is_sqlite_file_present(&database_url) {
        exit_with_error(
            "Database file is missing. Run `sakkath-api create-database` first, then optionally `sakkath-api seed-database`.",
        );
    }

    let db_pool = connect_db(&database_url, false).await;

    migration::run_migrations(&db_pool)
        .await
        .unwrap_or_else(|err| exit_with_error(&format!("Failed to run migrations: {err}")));

    migration::verify_migrations(&db_pool)
        .await
        .unwrap_or_else(|err| exit_with_error(&format!("Failed to verify migrations: {err}")));

    helpers::sorting::initialize_persistent_coin_toss_seed(&db_pool)
        .await
        .unwrap_or_else(|err| {
            exit_with_error(&format!("Failed to load persistent tiebreak seed: {err}"))
        });

    // Init redis cache (non-blocking, works without redis)
    helpers::cache::init_redis().await;
    telemetry::init(db_pool.clone(), telemetry_enabled).await;

    // Auto-generate R1 for divisions with no matches
    controllers::scheduling::auto_generate_initial_rounds(&db_pool).await;

    let app_state = AppState {
        db: db_pool,
        telemetry_enabled,
        live_updates: helpers::live_updates::LiveUpdates::new(),
    };

    let telemetry_routes = Router::new().route(
        "/telemetry",
        get(controllers::telemetry::get_telemetry)
            .route_layer(middleware::rate_limit::telemetry_rate_limit_layer()),
    );

    // Use routes from routes.rs
    let api_routes = routes::api_routes()
        .layer(CompressionLayer::new())
        .layer(middleware::rate_limit::api_rate_limit_layer());

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = if release_mode {
        tracing::info!("Serving static UI files from ../ui");

        Router::new()
            .merge(telemetry_routes)
            .nest("/v1", api_routes)
            .route(
                "/rules.md",
                get(|| async { serve_handbook_markdown("rules.md").await }),
            )
            .route(
                "/format.md",
                get(|| async { serve_handbook_markdown("format.md").await }),
            )
            .route(
                "/reporting.md",
                get(|| async { serve_handbook_markdown("reporting.md").await }),
            )
            .route(
                "/edit-team.md",
                get(|| async { serve_handbook_markdown("edit-team.md").await }),
            )
            .fallback_service(
                get_service(ServeDir::new("../ui"))
                    .layer(middleware::rate_limit::ui_rate_limit_layer()),
            )
            .with_state(app_state)
            .layer(axum_middleware::from_fn(
                middleware::telemetry::telemetry_middleware,
            ))
            .layer(cors)
    } else {
        tracing::info!("Development mode: serving API only at /v1");

        Router::new()
            .merge(telemetry_routes)
            .nest("/v1", api_routes)
            .with_state(app_state)
            .layer(axum_middleware::from_fn(
                middleware::telemetry::telemetry_middleware,
            ))
            .layer(cors)
    };

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Server failed");
}
