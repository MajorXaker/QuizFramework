//! REST endpoint handlers. The REST surface is intentionally small — most
//! real-time work happens over WebSockets.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::db;
use crate::state::AppState;
use crate::ws::handler::{
    broadcast, broadcast_allow_answer, broadcast_session_state, broadcast_stop_answer,
};
use crate::ws::messages::{ServerMsg, Winner};

pub async fn heartbeat() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

pub async fn public_config(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "backend_url": state.cfg.public_backend_url,
        "session_ttl_seconds": state.cfg.session_ttl_seconds,
    }))
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionReq {
    pub host_name: String,
}

#[derive(Debug, Serialize)]
pub struct CreateSessionResp {
    pub session_id: Uuid,
    pub participant_id: Uuid,
    pub invite_code: String,
}

pub async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionReq>,
) -> Result<Json<CreateSessionResp>, ApiError> {
    let name = req.host_name.trim();
    if name.is_empty() || name.len() > 32 {
        return Err(ApiError::bad_request("host_name must be 1..=32 chars"));
    }
    let (session, host) =
        db::create_session(&state.db, name, state.cfg.session_ttl_seconds).await?;
    Ok(Json(CreateSessionResp {
        session_id: session.id,
        participant_id: host.id,
        invite_code: session.invite_code,
    }))
}

#[derive(Debug, Deserialize)]
pub struct JoinReq {
    pub invite_code: String,
    pub display_name: String,
}

#[derive(Debug, Serialize)]
pub struct JoinResp {
    pub session_id: Uuid,
    pub participant_id: Uuid,
}

pub async fn join_session(
    State(state): State<AppState>,
    Json(req): Json<JoinReq>,
) -> Result<Json<JoinResp>, ApiError> {
    let name = req.display_name.trim();
    if name.is_empty() || name.len() > 32 {
        return Err(ApiError::bad_request("display_name must be 1..=32 chars"));
    }
    let code = req.invite_code.trim().to_lowercase();
    let session = db::fetch_session_by_code(&state.db, &code)
        .await?
        .ok_or_else(|| ApiError::not_found("session not found"))?;
    if !session.joins_enabled {
        return Err(ApiError::forbidden("joins are disabled"));
    }
    let p = db::add_participant(&state.db, session.id, name).await?;

    // Notify already-connected clients.
    let runtime = state.runtime_for(session.id).await;
    crate::ws::handler::broadcast_participants(&runtime, &state, session.id).await;

    Ok(Json(JoinResp {
        session_id: session.id,
        participant_id: p.id,
    }))
}

#[derive(Debug, Deserialize)]
pub struct HostAuth {
    pub host_id: Uuid,
}

async fn assert_host(
    state: &AppState,
    session_id: Uuid,
    host_id: Uuid,
) -> Result<db::SessionRow, ApiError> {
    let session = db::fetch_session(&state.db, session_id)
        .await?
        .ok_or_else(|| ApiError::not_found("session not found"))?;
    if session.host_id != host_id {
        return Err(ApiError::forbidden("not the host"));
    }
    if session.closed_at.is_some() {
        return Err(ApiError::forbidden("session is closed"));
    }
    Ok(session)
}

pub async fn regenerate_code(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(auth): Json<HostAuth>,
) -> Result<Json<serde_json::Value>, ApiError> {
    assert_host(&state, session_id, auth.host_id).await?;
    let new_code = db::regenerate_invite_code(&state.db, session_id).await?;
    let runtime = state.runtime_for(session_id).await;
    broadcast_session_state(&runtime, &state, session_id).await;
    Ok(Json(json!({ "invite_code": new_code })))
}

#[derive(Debug, Deserialize)]
pub struct ToggleReq {
    pub host_id: Uuid,
    pub enabled: bool,
}

pub async fn set_joins(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ToggleReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    assert_host(&state, session_id, req.host_id).await?;
    db::set_joins_enabled(&state.db, session_id, req.enabled).await?;
    let runtime = state.runtime_for(session_id).await;
    broadcast_session_state(&runtime, &state, session_id).await;
    Ok(Json(json!({ "joins_enabled": req.enabled })))
}

pub async fn set_answers(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ToggleReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    assert_host(&state, session_id, req.host_id).await?;
    db::set_answers_enabled(&state.db, session_id, req.enabled).await?;
    let runtime = state.runtime_for(session_id).await;
    broadcast_session_state(&runtime, &state, session_id).await;
    Ok(Json(json!({ "answers_enabled": req.enabled })))
}

pub async fn start_round(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(auth): Json<HostAuth>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = assert_host(&state, session_id, auth.host_id).await?;
    if !session.answers_enabled {
        // Auto-enable answers when a round starts; matches typical host UX.
        db::set_answers_enabled(&state.db, session_id, true).await?;
    }
    let round_id = db::start_round(&state.db, session_id).await?;
    let runtime = state.runtime_for(session_id).await;
    broadcast_session_state(&runtime, &state, session_id).await;
    broadcast_allow_answer(&runtime, round_id).await;
    Ok(Json(json!({ "round_id": round_id })))
}

pub async fn stop_round(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(auth): Json<HostAuth>,
) -> Result<Json<serde_json::Value>, ApiError> {
    assert_host(&state, session_id, auth.host_id).await?;
    let runtime = state.runtime_for(session_id).await;
    let (round_id, winner) = {
        let rt = runtime.lock().await;
        match &rt.current_round {
            Some(cr) => (
                Some(cr.round_id),
                cr.winner.as_ref().map(|w| Winner {
                    participant_id: w.participant_id,
                    participant_name: w.participant_name.clone(),
                    elapsed_ms: w.elapsed_ms,
                }),
            ),
            None => (None, None),
        }
    };
    let Some(round_id) = round_id else {
        return Err(ApiError::bad_request("no active round"));
    };
    db::stop_round(
        &state.db,
        round_id,
        winner.as_ref().map(|w| w.participant_id),
    )
    .await?;
    broadcast_stop_answer(&runtime, round_id, winner).await;
    Ok(Json(json!({ "round_id": round_id })))
}

pub async fn kick(
    State(state): State<AppState>,
    Path((session_id, participant_id)): Path<(Uuid, Uuid)>,
    Json(auth): Json<HostAuth>,
) -> Result<Json<serde_json::Value>, ApiError> {
    assert_host(&state, session_id, auth.host_id).await?;
    let p = db::fetch_participant(&state.db, participant_id)
        .await?
        .ok_or_else(|| ApiError::not_found("participant not found"))?;
    if p.session_id != session_id {
        return Err(ApiError::bad_request("participant in different session"));
    }
    if p.is_host {
        return Err(ApiError::bad_request("cannot kick host"));
    }
    db::kick_participant(&state.db, participant_id).await?;

    let runtime = state.runtime_for(session_id).await;
    // Notify and disconnect the kicked participant.
    {
        let rt = runtime.lock().await;
        if let Some(c) = rt.connections.get(&participant_id) {
            let _ =
                c.tx.send(serde_json::to_string(&ServerMsg::Kicked).unwrap());
        }
    }
    // Drop their connection so they can't reuse it.
    {
        let mut rt = runtime.lock().await;
        rt.connections.remove(&participant_id);
    }
    crate::ws::handler::broadcast_participants(&runtime, &state, session_id).await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn close_session(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(auth): Json<HostAuth>,
) -> Result<Json<serde_json::Value>, ApiError> {
    assert_host(&state, session_id, auth.host_id).await?;
    db::close_session(&state.db, session_id).await?;
    let runtime = state.runtime_for(session_id).await;
    let msg = ServerMsg::SessionClosed {
        reason: "closed by host".to_string(),
    };
    broadcast(&runtime, &msg).await;
    state.drop_runtime(session_id).await;
    db::delete_session(&state.db, session_id).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Forbidden(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

impl ApiError {
    pub fn bad_request(s: impl Into<String>) -> Self {
        ApiError::BadRequest(s.into())
    }
    pub fn not_found(s: impl Into<String>) -> Self {
        ApiError::NotFound(s.into())
    }
    pub fn forbidden(s: impl Into<String>) -> Self {
        ApiError::Forbidden(s.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, m.clone()),
            ApiError::Forbidden(m) => (StatusCode::FORBIDDEN, m.clone()),
            ApiError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            ApiError::Sqlx(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
