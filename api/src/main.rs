use axum::Router;
use sqlx::SqlitePool;
use std::net::SocketAddr;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tower_http::cors::{CorsLayer, Any};
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder, key_extractor::PeerIpKeyExtractor};
use std::sync::Arc;

mod migration;
mod controllers;
mod middleware;
mod routes;

// Tournament configuration: number of swiss rounds per division
pub const OPEN_ROUNDS: i64 = 4;
pub const WOMEN_ROUNDS: i64 = 3;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
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
    
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "9000".to_string())
        .parse()
        .expect("PORT must be a number");

    tracing::info!("Starting server in {} mode on port {}", 
        if release_mode { "RELEASE" } else { "DEVELOPMENT" }, 
        port
    );

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| {
            let current_dir = std::env::current_dir().expect("Failed to get current directory");
            tracing::info!("Current directory: {}", current_dir.display());
            
            let db_path = current_dir.parent()
                .expect("Failed to get parent directory")
                .join("sakkath.db");
            
            tracing::info!("Database path: {}", db_path.display());
            format!("sqlite:{}", db_path.display())
        });
    
    tracing::info!("Using database URL: {}", database_url);
    
    let db_pool = migration::init_db_pool(&database_url)
        .await
        .expect("Failed to connect to database");
    
    migration::run_migrations(&db_pool)
        .await
        .expect("Failed to run migrations");
    
    migration::verify_migrations(&db_pool)
        .await
        .expect("Failed to verify migrations");

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
