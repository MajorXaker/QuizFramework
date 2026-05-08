//! WebSocket connection handler.

use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db;
use crate::state::{
    answer_is_better, AnswerEntry, AppState, Connection, CurrentRound, ParticipantInfo,
    SessionRuntime,
};
use crate::ws::messages::{ClientMsg, ServerMsg, Winner};

#[derive(Debug, Deserialize)]
pub struct WsParams {
    pub session_id: Uuid,
    pub participant_id: Uuid,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| connection(socket, params, state))
}

async fn connection(socket: WebSocket, params: WsParams, state: AppState) {
    // Validate session + participant up front.
    let session = match db::fetch_session(&state.db, params.session_id).await {
        Ok(Some(s)) if s.closed_at.is_none() => s,
        _ => {
            close_with_error(socket, "session not found").await;
            return;
        }
    };
    let participant = match db::fetch_participant(&state.db, params.participant_id).await {
        Ok(Some(p)) if p.session_id == session.id && p.kicked_at.is_none() => p,
        _ => {
            close_with_error(socket, "participant not found").await;
            return;
        }
    };

    let (mut sink, mut stream) = socket.split();

    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    let runtime = state.runtime_for(session.id).await;

    // Register connection.
    {
        let mut rt = runtime.lock().await;
        rt.connections.insert(
            participant.id,
            Connection {
                is_host: participant.is_host,
                tx: tx.clone(),
            },
        );
        if participant.is_host {
            rt.host_last_seen = Some(Utc::now());
        }
    }

    // Send Welcome.
    let participants = match db::list_participants(&state.db, session.id).await {
        Ok(p) => p,
        Err(e) => {
            warn!(error=%e, "Failed to list participants");
            Vec::new()
        }
    };
    let participants_info = build_participant_info(&runtime, &participants).await;
    let welcome = ServerMsg::Welcome {
        session_id: session.id,
        participant_id: participant.id,
        is_host: participant.is_host,
        invite_code: session.invite_code.clone(),
        joins_enabled: session.joins_enabled,
        answers_enabled: session.answers_enabled,
        participants: participants_info.clone(),
        expires_at: session.expires_at,
    };
    let _ = tx.send(serde_json::to_string(&welcome).unwrap());

    // Notify others of new connection state.
    broadcast_participants(&runtime, &state, session.id).await;

    // Forward messages from `tx` to the WS sink.
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sink.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    // Read loop.
    let read_state = state.clone();
    let read_runtime = runtime.clone();
    let participant_id = participant.id;
    let participant_name = participant.name.clone();
    let is_host = participant.is_host;
    let session_id = session.id;
    let read_task = tokio::spawn(async move {
        while let Some(msg) = stream.next().await {
            let msg = match msg {
                Ok(m) => m,
                Err(_) => break,
            };
            match msg {
                Message::Text(text) => {
                    let parsed: Result<ClientMsg, _> = serde_json::from_str(&text);
                    match parsed {
                        Ok(ClientMsg::Ping) => {
                            send_to_self(&read_runtime, participant_id, &ServerMsg::Pong).await;
                        }
                        Ok(ClientMsg::Answer { elapsed_ms }) => {
                            handle_answer(
                                &read_state,
                                &read_runtime,
                                session_id,
                                participant_id,
                                &participant_name,
                                is_host,
                                elapsed_ms,
                            )
                            .await;
                        }
                        Err(e) => {
                            send_to_self(
                                &read_runtime,
                                participant_id,
                                &ServerMsg::Error {
                                    message: format!("invalid message: {}", e),
                                },
                            )
                            .await;
                        }
                    }
                }
                Message::Close(_) => break,
                Message::Ping(_) | Message::Pong(_) | Message::Binary(_) => {}
            }
        }
    });

    // Wait for either side to terminate.
    tokio::select! {
        _ = send_task => {},
        _ = read_task => {},
    }

    // Deregister.
    {
        let mut rt = runtime.lock().await;
        rt.connections.remove(&participant_id);
    }
    debug!(?participant_id, "WS disconnected");
    broadcast_participants(&runtime, &state, session.id).await;
}

async fn close_with_error(mut socket: WebSocket, message: &str) {
    let err = ServerMsg::Error {
        message: message.to_string(),
    };
    let _ = socket
        .send(Message::Text(serde_json::to_string(&err).unwrap()))
        .await;
    let _ = socket.close().await;
}

async fn build_participant_info(
    runtime: &Arc<Mutex<SessionRuntime>>,
    participants: &[db::ParticipantRow],
) -> Vec<ParticipantInfo> {
    let rt = runtime.lock().await;
    participants
        .iter()
        .map(|p| ParticipantInfo {
            id: p.id,
            name: p.name.clone(),
            is_host: p.is_host,
            connected: rt.connections.contains_key(&p.id),
        })
        .collect()
}

pub async fn broadcast_participants(
    runtime: &Arc<Mutex<SessionRuntime>>,
    state: &AppState,
    session_id: Uuid,
) {
    let participants = match db::list_participants(&state.db, session_id).await {
        Ok(p) => p,
        Err(_) => return,
    };
    let info = build_participant_info(runtime, &participants).await;
    let msg = ServerMsg::Participants { participants: info };
    broadcast(runtime, &msg).await;
}

pub async fn broadcast(runtime: &Arc<Mutex<SessionRuntime>>, msg: &ServerMsg) {
    let payload = serde_json::to_string(msg).unwrap();
    let rt = runtime.lock().await;
    for c in rt.connections.values() {
        let _ = c.tx.send(payload.clone());
    }
}

pub async fn broadcast_session_state(
    runtime: &Arc<Mutex<SessionRuntime>>,
    state: &AppState,
    session_id: Uuid,
) {
    if let Ok(Some(s)) = db::fetch_session(&state.db, session_id).await {
        let msg = ServerMsg::SessionState {
            invite_code: s.invite_code,
            joins_enabled: s.joins_enabled,
            answers_enabled: s.answers_enabled,
        };
        broadcast(runtime, &msg).await;
    }
}

pub async fn broadcast_allow_answer(runtime: &Arc<Mutex<SessionRuntime>>, round_id: Uuid) {
    {
        let mut rt = runtime.lock().await;
        rt.current_round = Some(CurrentRound {
            round_id,
            winner: None,
        });
    }
    let msg = ServerMsg::AllowAnswer { round_id };
    broadcast(runtime, &msg).await;
}

pub async fn broadcast_stop_answer(
    runtime: &Arc<Mutex<SessionRuntime>>,
    round_id: Uuid,
    winner: Option<Winner>,
) {
    {
        let mut rt = runtime.lock().await;
        if let Some(cur) = &rt.current_round {
            if cur.round_id == round_id {
                rt.current_round = None;
            }
        }
    }
    let msg = ServerMsg::StopAnswer { round_id, winner };
    broadcast(runtime, &msg).await;
}

async fn send_to_self(runtime: &Arc<Mutex<SessionRuntime>>, participant_id: Uuid, msg: &ServerMsg) {
    let payload = serde_json::to_string(msg).unwrap();
    let rt = runtime.lock().await;
    if let Some(c) = rt.connections.get(&participant_id) {
        let _ = c.tx.send(payload);
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_answer(
    state: &AppState,
    runtime: &Arc<Mutex<SessionRuntime>>,
    session_id: Uuid,
    participant_id: Uuid,
    participant_name: &str,
    is_host: bool,
    elapsed_ms: i32,
) {
    if is_host {
        send_to_self(
            runtime,
            participant_id,
            &ServerMsg::Error {
                message: "host cannot answer".to_string(),
            },
        )
        .await;
        return;
    }
    if elapsed_ms < 0 {
        send_to_self(
            runtime,
            participant_id,
            &ServerMsg::Error {
                message: "elapsed_ms must be non-negative".to_string(),
            },
        )
        .await;
        return;
    }

    // Pull current round id under lock; reject if no round is active.
    let round_id = {
        let rt = runtime.lock().await;
        match &rt.current_round {
            Some(cr) => cr.round_id,
            None => {
                drop(rt);
                send_to_self(
                    runtime,
                    participant_id,
                    &ServerMsg::Error {
                        message: "no active round".to_string(),
                    },
                )
                .await;
                return;
            }
        }
    };

    // Cross-check answers_enabled (defence in depth).
    if let Ok(Some(s)) = db::fetch_session(&state.db, session_id).await {
        if !s.answers_enabled {
            send_to_self(
                runtime,
                participant_id,
                &ServerMsg::Error {
                    message: "answers are disabled".to_string(),
                },
            )
            .await;
            return;
        }
    }

    // Persist the answer (unique per round+participant).
    let server_received =
        match db::record_answer(&state.db, round_id, participant_id, elapsed_ms).await {
            Ok(t) => t,
            Err(e) => {
                warn!(error=%e, "record_answer failed");
                return;
            }
        };

    let candidate = AnswerEntry {
        participant_id,
        participant_name: participant_name.to_string(),
        elapsed_ms,
        server_received,
    };

    // Atomically decide the winner.
    let winner = {
        let mut rt = runtime.lock().await;
        let current = match rt.current_round.as_mut() {
            Some(c) if c.round_id == round_id => c,
            _ => return,
        };
        match &current.winner {
            None => {
                current.winner = Some(candidate.clone());
                Some(candidate)
            }
            Some(existing) => {
                if answer_is_better(&candidate, existing) {
                    current.winner = Some(candidate.clone());
                    Some(candidate)
                } else {
                    None
                }
            }
        }
    };

    if let Some(w) = winner {
        let payload = ServerMsg::AnswerWinner {
            round_id,
            winner: Winner {
                participant_id: w.participant_id,
                participant_name: w.participant_name,
                elapsed_ms: w.elapsed_ms,
            },
        };
        broadcast(runtime, &payload).await;
        info!(?round_id, "winner decided");
    }
}
