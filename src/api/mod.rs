//! REST API: session lifecycle and host controls.

pub mod routes;

use axum::{
    routing::{get, post},
    Router,
};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get_service;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

use crate::state::AppState;
use crate::ws::ws_handler;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/__heartbeat__", get(routes::heartbeat))
        .route("/api/config", get(routes::public_config))
        .route("/api/sessions", post(routes::create_session))
        .route("/api/sessions/join", post(routes::join_session))
        .route(
            "/api/sessions/:session_id/regenerate_code",
            post(routes::regenerate_code),
        )
        .route(
            "/api/sessions/:session_id/joins_enabled",
            post(routes::set_joins),
        )
        .route(
            "/api/sessions/:session_id/answers_enabled",
            post(routes::set_answers),
        )
        .route(
            "/api/sessions/:session_id/start_round",
            post(routes::start_round),
        )
        .route(
            "/api/sessions/:session_id/stop_round",
            post(routes::stop_round),
        )
        .route(
            "/api/sessions/:session_id/participants/:participant_id/kick",
            post(routes::kick),
        )
        .route(
            "/api/sessions/:session_id/close",
            post(routes::close_session),
        )
        .route("/ws", get(ws_handler))
        .route("/", get_service(ServeFile::new("static/index.html")))
        .nest_service(
            "/static",
            ServeDir::new("static")
                // default is true, but explicit is nice for readability
                .append_index_html_on_directories(true),
        )
        // router-level fallback for everything else
        .fallback(not_found)
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "404 Not Found")
}