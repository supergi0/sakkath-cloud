use axum::Router;
use axum::middleware as axum_middleware;
use axum::routing::{get, get_service};
use sqlx::SqlitePool;
use std::net::SocketAddr;
use std::process;
use tower_http::services::ServeDir;
use tower_http::cors::{CorsLayer, Any};

mod migration;
mod seeder;
mod controllers;
mod middleware;
mod routes;
mod telemetry;
pub mod helpers;

// Tournament configuration: number of swiss rounds per division
pub const OPEN_ROUNDS: i64 = 6;
pub const WOMEN_ROUNDS: i64 = 6;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub telemetry_enabled: bool,
}

enum CliCommand {
    Serve,
    CreateDatabase,
    SeedDatabase(migration::SeedSource),
}

fn parse_cli_command() -> CliCommand {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("create-database") => CliCommand::CreateDatabase,
        Some("seed-database") => match args.get(1).map(String::as_str) {
            Some("csv") => CliCommand::SeedDatabase(migration::SeedSource::TeamsCsv { path: args.get(2).cloned() }),
            _ => CliCommand::SeedDatabase(migration::SeedSource::MockData),
        },
        _ => CliCommand::Serve,
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
        .to_lowercase() == "true";
    let telemetry_enabled = std::env::var("emit_telemetry")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase() == "true";

    let cli_command = parse_cli_command();
    
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "9000".to_string())
        .parse()
        .expect("PORT must be a number");

    tracing::info!("Starting server in {} mode on port {}", 
        if release_mode { "RELEASE" } else { "DEVELOPMENT" }, 
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
                exit_with_error("Database file is missing. Run `sakkath-api create-database` first.");
            }

            run_database_setup(&database_url, Some(source)).await;
            return;
        }
        CliCommand::Serve => {}
    }

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

    // Init redis cache (non-blocking, works without redis)
    helpers::cache::init_redis().await;
    telemetry::init(db_pool.clone(), telemetry_enabled).await;

    // Auto-generate R1 for divisions with no matches
    controllers::scheduling::auto_generate_initial_rounds(&db_pool).await;

    let app_state = AppState {
        db: db_pool,
        telemetry_enabled,
    };

    let telemetry_routes = Router::new().route(
        "/telemetry",
        get(controllers::telemetry::get_telemetry)
            .route_layer(middleware::rate_limit::telemetry_rate_limit_layer()),
    );

    // Use routes from routes.rs
    let api_routes = routes::api_routes().layer(middleware::rate_limit::api_rate_limit_layer());

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = if release_mode {
        tracing::info!("Serving static UI files from ../ui");
        
        Router::new()
            .merge(telemetry_routes)
            .nest("/v1", api_routes)
            .fallback_service(
                get_service(ServeDir::new("../ui"))
                    .layer(middleware::rate_limit::ui_rate_limit_layer()),
            )
            .with_state(app_state)
            .layer(axum_middleware::from_fn(middleware::telemetry::telemetry_middleware))
            .layer(cors)
    } else {
        tracing::info!("Development mode: serving API only at /v1");
        
        Router::new()
            .merge(telemetry_routes)
            .nest("/v1", api_routes)
            .with_state(app_state)
            .layer(axum_middleware::from_fn(middleware::telemetry::telemetry_middleware))
            .layer(cors)
    };

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Listening on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");
    
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .expect("Server failed");
}
