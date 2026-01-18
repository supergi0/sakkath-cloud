use axum::{
    Router,
    middleware,
    routing::{get, post, put, delete},
};

use crate::controllers::{
    user,
    team,
    announcements,
    matches,
    health,
};
use crate::middleware::auth::auth_middleware;
use crate::middleware::logger::logger_middleware;
use crate::AppState;

// Public routes without authentication
fn routes_without_auth() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health_check))
        .route("/auth/login", post(user::login))
        .route("/auth/verify", post(user::verify))
        .route("/stats", get(matches::get_stats))
}

// Protected routes requiring authentication
fn routes_with_auth() -> Router<AppState> {
    Router::new()
        .route("/auth/role", get(user::get_role))
}

// POC routes (protected)
fn poc_routes() -> Router<AppState> {
    Router::new()
        .route("/poc/team", get(team::get_poc_team))
        .route("/poc/team/logo", put(team::update_poc_team_logo))
        .route("/poc/players", get(team::get_poc_players))
        .route("/poc/players", post(team::add_poc_player))
        .route("/poc/players/:id", put(team::update_poc_player))
        .route("/poc/players/:id", delete(team::delete_poc_player))
}

// Team routes (public)
fn team_routes() -> Router<AppState> {
    Router::new()
        .route("/teams", get(team::get_teams))
        .route("/teams/:id", get(team::get_team_detail))
        .route("/teams/:id/players", get(team::get_team_players))
        .route("/standings", get(team::get_standings))
        .route("/player-stats", get(team::get_player_stats))
}

// Announcement routes (public)
fn announcement_routes() -> Router<AppState> {
    Router::new()
        .route("/announcements", get(announcements::get_announcements))
}

// Match routes (public)
fn match_routes() -> Router<AppState> {
    Router::new()
        .route("/fields", get(matches::get_fields))
}

// Combine all routes
pub fn api_routes() -> Router<AppState> {
    let mut router = Router::new()
        .merge(routes_without_auth())
        .merge(team_routes())
        .merge(announcement_routes())
        .merge(match_routes());

    router = router.merge(
        routes_with_auth()
            .merge(poc_routes())
            .layer(middleware::from_fn(auth_middleware))
    );

    // Add logger middleware to all routes
    router.layer(middleware::from_fn(logger_middleware))
}
