//! WebSocket message types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::ParticipantInfo;

/// Messages from client → server.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Players send `Answer` with the elapsed milliseconds since they
    /// received the most recent `AllowAnswer` broadcast.
    Answer { elapsed_ms: i32 },
    /// Optional client→server keepalive.
    Ping,
}

/// Messages from server → client.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// Sent right after the WS handshake. Includes session settings and
    /// current participants.
    Welcome {
        session_id: Uuid,
        participant_id: Uuid,
        is_host: bool,
        invite_code: String,
        joins_enabled: bool,
        answers_enabled: bool,
        participants: Vec<ParticipantInfo>,
        expires_at: DateTime<Utc>,
    },
    /// Roster changed (join, leave, kick).
    Participants { participants: Vec<ParticipantInfo> },
    /// Session settings changed (joins_enabled / answers_enabled / invite_code).
    SessionState {
        invite_code: String,
        joins_enabled: bool,
        answers_enabled: bool,
    },
    /// Host started a question. Players should arm their answer button and
    /// start measuring elapsed milliseconds from receipt of this message.
    AllowAnswer { round_id: Uuid },
    /// Host stopped the question (no winner yet, or accepting no more).
    StopAnswer {
        round_id: Uuid,
        winner: Option<Winner>,
    },
    /// First valid answer recorded; everyone updates UI accordingly.
    AnswerWinner { round_id: Uuid, winner: Winner },
    /// You were kicked. The connection will be closed right after.
    Kicked,
    /// Session ended (TTL or host closed).
    SessionClosed { reason: String },
    /// Ping/pong helpers.
    Pong,
    /// Anything went wrong with a previous message.
    Error { message: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct Winner {
    pub participant_id: Uuid,
    pub participant_name: String,
    pub elapsed_ms: i32,
}
