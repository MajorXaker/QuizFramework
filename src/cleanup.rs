//! Background cleanup task. Two responsibilities:
//!
//! 1. Delete sessions that are past their `expires_at` (hard 6-hour cap).
//! 2. Close sessions whose host has been disconnected for longer than the
//!    configured grace period.

use std::time::Duration;

use chrono::Utc;
use sqlx::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

use crate::db;
use crate::state::AppState;
use crate::ws::handler::broadcast;
use crate::ws::messages::ServerMsg;

const TICK: Duration = Duration::from_secs(15);

pub async fn run(state: AppState) {
    loop {
        if let Err(e) = tick(&state).await {
            warn!(error=%e, "cleanup tick failed");
        }
        tokio::time::sleep(TICK).await;
    }
}

async fn tick(state: &AppState) -> anyhow::Result<()> {
    // 1. TTL & explicitly closed sessions.
    let expired = db::list_expired_sessions(&state.db).await?;
    for sid in expired {
        end_session(state, sid, "expired or closed").await;
    }

    // 2. Host disconnect grace.
    let grace = state.cfg.host_disconnect_grace_seconds;
    let now = Utc::now();

    // Snapshot live runtimes to avoid holding the outer lock while we work.
    let live_ids: Vec<Uuid> = {
        let map = state.sessions.lock().await;
        map.keys().copied().collect()
    };

    for sid in live_ids {
        let runtime = state.runtime_for(sid).await;
        let (host_connected, last_seen) = {
            let rt = runtime.lock().await;
            let host_connected = rt.connections.values().any(|c| c.is_host);
            (host_connected, rt.host_last_seen)
        };

        if host_connected {
            let mut rt = runtime.lock().await;
            rt.host_last_seen = Some(now);
            continue;
        }

        let stale = match last_seen {
            Some(t) => (now - t).num_seconds() > grace,
            // Never seen: only count as stale once we've been tracking for a
            // bit. If we never recorded a host_last_seen, fall back to the
            // session's age in DB.
            None => true,
        };

        if stale {
            // Also covers cases where the host never connected after a long time.
            let still_recent = match db::fetch_session(&state.db, sid).await {
                Ok(Some(s)) => (now - s.created_at).num_seconds() <= grace,
                _ => false,
            };
            if still_recent {
                continue;
            }
            end_session(state, sid, "host disconnected").await;
        }
    }
    Ok(())
}

async fn end_session(state: &AppState, sid: Uuid, reason: &str) {
    info!(session_id=%sid, %reason, "ending session");
    let runtime = state.runtime_for(sid).await;
    let msg = ServerMsg::SessionClosed {
        reason: reason.to_string(),
    };
    broadcast(&runtime, &msg).await;
    if let Err(e) = close_and_delete(&state.db, sid).await {
        warn!(error=%e, "failed to delete session");
    }
    state.drop_runtime(sid).await;
}

async fn close_and_delete(pool: &PgPool, sid: Uuid) -> anyhow::Result<()> {
    db::close_session(pool, sid).await?;
    db::delete_session(pool, sid).await?;
    Ok(())
}
