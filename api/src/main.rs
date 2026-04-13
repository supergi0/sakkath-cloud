use axum::Router;
use sqlx::SqlitePool;
use std::net::SocketAddr;
use std::process;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tower_http::cors::{CorsLayer, Any};
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder, key_extractor::PeerIpKeyExtractor};
use std::sync::Arc;

mod migration;
mod seeder;
mod controllers;
mod middleware;
mod routes;
pub mod helpers;

// Tournament configuration: number of swiss rounds per division
pub const OPEN_ROUNDS: i64 = 6;
pub const WOMEN_ROUNDS: i64 = 6;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
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

    // Auto-generate R1 for divisions with no matches
    controllers::scheduling::auto_generate_initial_rounds(&db_pool).await;

    let app_state = AppState {
        db: db_pool,
    };

    // Use routes from routes.rs
    let api_routes = routes::api_routes()
        .with_state(app_state);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Rate limiter: 100 requests per minute per IP (using PeerIpKeyExtractor for direct connections)
    let rate_limit_config = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(PeerIpKeyExtractor)
            .per_second(2)
            .burst_size(100)
            .finish()
            .unwrap()
    );
    let rate_limit_layer = GovernorLayer { config: rate_limit_config };

    let app = if release_mode {
        tracing::info!("Serving static UI files from ../ui");
        
        Router::new()
            .nest("/v1", api_routes)
            .fallback_service(ServeDir::new("../ui"))
            .layer(rate_limit_layer)
            .layer(cors)
            .layer(TraceLayer::new_for_http())
    } else {
        tracing::info!("Development mode: serving API only at /v1");
        
        Router::new()
            .nest("/v1", api_routes)
            .layer(rate_limit_layer)
            .layer(cors)
            .layer(TraceLayer::new_for_http())
    };

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("Listening on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");
    
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .expect("Server failed");
}
