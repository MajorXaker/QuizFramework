//! In-memory hub: per-session connection registry plus the round/answer
//! evaluation logic. The DB is the persistent record; this module is the live
//! pub/sub for active WebSocket clients.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::config::Config;

/// A message broadcast to a single WebSocket client.
pub type WsMsg = String;

#[derive(Debug)]
pub struct Connection {
    pub is_host: bool,
    pub tx: mpsc::UnboundedSender<WsMsg>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParticipantInfo {
    pub id: Uuid,
    pub name: String,
    pub is_host: bool,
    pub connected: bool,
}

#[derive(Debug, Default)]
pub struct CurrentRound {
    pub round_id: Uuid,
    /// First valid answer wins. We store it once, atomically.
    pub winner: Option<AnswerEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnswerEntry {
    pub participant_id: Uuid,
    pub participant_name: String,
    pub elapsed_ms: i32,
    pub server_received: DateTime<Utc>,
}

#[derive(Debug, Default)]
pub struct SessionRuntime {
    pub connections: HashMap<Uuid, Connection>, // participant_id -> conn
    pub current_round: Option<CurrentRound>,
    /// last time the host's WS was seen connected; used by cleanup loop
    pub host_last_seen: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub cfg: Arc<Config>,
    pub sessions: Arc<Mutex<HashMap<Uuid, Arc<Mutex<SessionRuntime>>>>>,
}

impl AppState {
    pub fn new(db: PgPool, cfg: Config) -> Self {
        Self {
            db,
            cfg: Arc::new(cfg),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn runtime_for(&self, session_id: Uuid) -> Arc<Mutex<SessionRuntime>> {
        let mut map = self.sessions.lock().await;
        map.entry(session_id)
            .or_insert_with(|| Arc::new(Mutex::new(SessionRuntime::default())))
            .clone()
    }

    pub async fn drop_runtime(&self, session_id: Uuid) {
        let mut map = self.sessions.lock().await;
        map.remove(&session_id);
    }
}

/// Compare answers deterministically. Lower elapsed wins; tie-break by
/// server-received time, then participant id.
pub fn answer_is_better(a: &AnswerEntry, b: &AnswerEntry) -> bool {
    if a.elapsed_ms != b.elapsed_ms {
        return a.elapsed_ms < b.elapsed_ms;
    }
    if a.server_received != b.server_received {
        return a.server_received < b.server_received;
    }
    a.participant_id < b.participant_id
}
