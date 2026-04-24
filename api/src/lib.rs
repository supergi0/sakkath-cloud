use axum::Router;
use sqlx::SqlitePool;

pub mod controllers;
pub mod helpers;
pub mod middleware;
pub mod migration;
pub mod routes;
pub mod seeder;
pub mod telemetry;

pub const OPEN_ROUNDS: i64 = 6;
pub const WOMEN_ROUNDS: i64 = 6;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub telemetry_enabled: bool,
}

pub fn build_api_only_app(app_state: AppState) -> Router {
    Router::new()
        .nest("/v1", routes::api_routes())
        .with_state(app_state)
}
