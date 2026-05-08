//! quiz-helper — backend entrypoint.

mod api;
mod cleanup;
mod config;
mod db;
mod state;
mod ws;

use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cfg = config::Config::from_env().unwrap_or_else(|e| {
        error!(error=%e, "Failed to load config from env");
        std::process::exit(1);
    });

    info!(db = %cfg.redacted_db_url(), "Connecting to PostgreSQL");
    let pool = sqlx::PgPool::connect(&cfg.database_url)
        .await
        .unwrap_or_else(|e| {
            error!(error=%e, "Failed to connect to PostgreSQL — aborting");
            std::process::exit(1);
        });

    db::run_migrations(&pool).await.unwrap_or_else(|e| {
        error!(error=%e, "Failed to run migrations — aborting");
        std::process::exit(1);
    });

    let app_state = state::AppState::new(pool, cfg.clone());

    {
        let s = app_state.clone();
        tokio::spawn(async move { cleanup::run(s).await });
    }

    let bind = format!("{}:{}", cfg.bind_host, cfg.bind_port);
    let router = api::create_router(app_state);

    info!(address = %bind, "HTTP server listening");
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .unwrap_or_else(|e| {
            error!(error=%e, address=%bind, "Failed to bind TCP listener — aborting");
            std::process::exit(1);
        });

    axum::serve(listener, router).await.unwrap_or_else(|e| {
        error!(error=%e, "HTTP server error");
        std::process::exit(1);
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::state::{answer_is_better, AnswerEntry};
    use chrono::TimeZone;
    use uuid::Uuid;

    fn entry(elapsed: i32, ts_secs: i64, id_byte: u8) -> AnswerEntry {
        AnswerEntry {
            participant_id: Uuid::from_bytes([id_byte; 16]),
            participant_name: "p".into(),
            elapsed_ms: elapsed,
            server_received: chrono::Utc.timestamp_opt(ts_secs, 0).unwrap(),
        }
    }

    #[test]
    fn lower_elapsed_wins() {
        let a = entry(100, 0, 1);
        let b = entry(200, 0, 2);
        assert!(answer_is_better(&a, &b));
        assert!(!answer_is_better(&b, &a));
    }

    #[test]
    fn tie_breaks_by_server_time_then_id() {
        let a = entry(100, 1, 2);
        let b = entry(100, 2, 1);
        assert!(answer_is_better(&a, &b)); // earlier server time wins

        let c = entry(100, 5, 3);
        let d = entry(100, 5, 9);
        assert!(answer_is_better(&c, &d)); // smaller id wins on full tie
    }

    #[test]
    fn invite_code_format() {
        let c = super::db::generate_invite_code();
        assert_eq!(c.len(), 8);
        assert!(c
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit()));
    }
}
