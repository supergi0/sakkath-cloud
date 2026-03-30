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
    scheduling,
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
        .route("/poc/matches", get(matches::get_poc_matches))
        .route("/poc/matches/:id/spirit", put(matches::submit_spirit_score))
        .route("/poc/matches/:id/spirit-wfdf", put(matches::submit_wfdf_spirit))
        .route("/poc/matches/:id/confirm-score", post(matches::confirm_score))
        .route("/poc/matches/:id/opponent-players", get(matches::get_opponent_players))
}

// Volunteer/Admin routes (protected)
fn volunteer_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/matches", get(matches::get_upcoming_matches))
        .route("/admin/matches/:id/start", post(matches::start_match))
        .route("/admin/matches/:id/end", post(matches::end_match))
        .route("/admin/matches/:id/event", post(matches::record_event))
    .route("/admin/matches/:id/switch-possession", post(matches::switch_possession))
        .route("/admin/matches/:id/undo", post(matches::undo_event))
}

// Team routes (public)
fn team_routes() -> Router<AppState> {
    Router::new()
        .route("/teams", get(team::get_teams))
        .route("/teams/:id", get(team::get_team_detail))
        .route("/teams/:id/players", get(team::get_team_players))
        .route("/teams/:id/matches", get(matches::get_team_matches))
        .route("/standings", get(team::get_standings))
        .route("/player-stats", get(team::get_player_stats))
}

// Announcement routes (public)
fn announcement_routes() -> Router<AppState> {
    Router::new()
        .route("/announcements", get(announcements::get_announcements))
}

// Super admin routes (protected, SUPER only)
fn super_routes() -> Router<AppState> {
    Router::new()
        .route("/super/announcements", post(announcements::create_announcement))
        .route("/super/announcements/:id", put(announcements::update_announcement))
        .route("/super/announcements/:id", delete(announcements::delete_announcement))
}

// Match routes (public)
fn match_routes() -> Router<AppState> {
    Router::new()
        .route("/fields", get(matches::get_fields))
        .route("/matches/:id", get(matches::get_match_detail))
        .route("/matches/:id/events", get(matches::get_match_events))
        .route("/matches/:id/spirits", get(matches::get_match_spirits))
        .route("/matches/:id/score-confirmations", get(matches::get_score_confirmations))
}

// Schedule routes (public)
fn schedule_routes() -> Router<AppState> {
    Router::new()
        .route("/schedule/state", get(scheduling::read_tournament_state))
        .route("/schedule/matches", get(scheduling::get_schedule_matches))
        .route("/schedule/early-fixtures", get(scheduling::get_early_fixtures))
}

// Schedule admin routes (protected)
fn schedule_admin_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/schedule/:division/generate", post(scheduling::generate_next_round))
        .route("/admin/schedule/:division/check-gates", post(scheduling::check_and_populate_gates))
}

// Combine all routes
pub fn api_routes() -> Router<AppState> {
    let mut router = Router::new()
        .merge(routes_without_auth())
        .merge(team_routes())
        .merge(announcement_routes())
        .merge(match_routes())
        .merge(schedule_routes());

    router = router.merge(
        routes_with_auth()
            .merge(poc_routes())
            .merge(volunteer_routes())
            .merge(super_routes())
            .merge(schedule_admin_routes())
            .layer(middleware::from_fn(auth_middleware))
    );

    // Add logger middleware to all routes
    router.layer(middleware::from_fn(logger_middleware))
}
