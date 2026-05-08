//! Database helpers: migrations and session/participant CRUD.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

pub fn generate_invite_code() -> String {
    // 8 chars, lowercase letters + digits.
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..8)
        .map(|_| {
            let i = rng.gen_range(0..CHARSET.len());
            CHARSET[i] as char
        })
        .collect()
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct SessionRow {
    pub id: Uuid,
    pub invite_code: String,
    pub host_id: Uuid,
    pub joins_enabled: bool,
    pub answers_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ParticipantRow {
    pub id: Uuid,
    pub session_id: Uuid,
    pub name: String,
    pub is_host: bool,
    pub joined_at: DateTime<Utc>,
    pub kicked_at: Option<DateTime<Utc>>,
}

pub async fn create_session(
    pool: &PgPool,
    host_name: &str,
    ttl_seconds: i64,
) -> Result<(SessionRow, ParticipantRow)> {
    // Try a few times in the unlikely event of an invite_code collision.
    for _ in 0..5 {
        let code = generate_invite_code();
        let session_id = Uuid::new_v4();
        let host_id = Uuid::new_v4();
        let expires_at = Utc::now() + Duration::seconds(ttl_seconds);

        let mut tx = pool.begin().await?;

        let session_res = sqlx::query_as::<_, SessionRow>(
            r#"INSERT INTO sessions (id, invite_code, host_id, expires_at)
               VALUES ($1, $2, $3, $4)
               RETURNING *"#,
        )
        .bind(session_id)
        .bind(&code)
        .bind(host_id)
        .bind(expires_at)
        .fetch_one(&mut *tx)
        .await;

        let session = match session_res {
            Ok(s) => s,
            Err(sqlx::Error::Database(e)) if e.constraint() == Some("sessions_invite_code_key") => {
                tx.rollback().await.ok();
                continue;
            }
            Err(e) => return Err(e.into()),
        };

        let host = sqlx::query_as::<_, ParticipantRow>(
            r#"INSERT INTO participants (id, session_id, name, is_host)
               VALUES ($1, $2, $3, TRUE)
               RETURNING *"#,
        )
        .bind(host_id)
        .bind(session_id)
        .bind(host_name)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        return Ok((session, host));
    }
    anyhow::bail!("Could not generate a unique invite code after several attempts")
}

pub async fn regenerate_invite_code(pool: &PgPool, session_id: Uuid) -> Result<String> {
    for _ in 0..5 {
        let code = generate_invite_code();
        let res = sqlx::query("UPDATE sessions SET invite_code = $1 WHERE id = $2")
            .bind(&code)
            .bind(session_id)
            .execute(pool)
            .await;
        match res {
            Ok(r) if r.rows_affected() == 1 => return Ok(code),
            Ok(_) => anyhow::bail!("session not found"),
            Err(sqlx::Error::Database(e)) if e.constraint() == Some("sessions_invite_code_key") => {
                continue
            }
            Err(e) => return Err(e.into()),
        }
    }
    anyhow::bail!("Could not regenerate invite code")
}

pub async fn fetch_session_by_code(pool: &PgPool, code: &str) -> Result<Option<SessionRow>> {
    let row = sqlx::query_as::<_, SessionRow>(
        "SELECT * FROM sessions WHERE invite_code = $1 AND closed_at IS NULL",
    )
    .bind(code)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn fetch_session(pool: &PgPool, id: Uuid) -> Result<Option<SessionRow>> {
    let row = sqlx::query_as::<_, SessionRow>("SELECT * FROM sessions WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn add_participant(
    pool: &PgPool,
    session_id: Uuid,
    name: &str,
) -> Result<ParticipantRow> {
    let id = Uuid::new_v4();
    let row = sqlx::query_as::<_, ParticipantRow>(
        r#"INSERT INTO participants (id, session_id, name, is_host)
           VALUES ($1, $2, $3, FALSE)
           RETURNING *"#,
    )
    .bind(id)
    .bind(session_id)
    .bind(name)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn list_participants(pool: &PgPool, session_id: Uuid) -> Result<Vec<ParticipantRow>> {
    let rows = sqlx::query_as::<_, ParticipantRow>(
        "SELECT * FROM participants WHERE session_id = $1 AND kicked_at IS NULL ORDER BY joined_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn fetch_participant(pool: &PgPool, id: Uuid) -> Result<Option<ParticipantRow>> {
    let row = sqlx::query_as::<_, ParticipantRow>("SELECT * FROM participants WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn kick_participant(pool: &PgPool, participant_id: Uuid) -> Result<()> {
    sqlx::query("UPDATE participants SET kicked_at = NOW() WHERE id = $1 AND is_host = FALSE")
        .bind(participant_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_joins_enabled(pool: &PgPool, session_id: Uuid, enabled: bool) -> Result<()> {
    sqlx::query("UPDATE sessions SET joins_enabled = $1 WHERE id = $2")
        .bind(enabled)
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_answers_enabled(pool: &PgPool, session_id: Uuid, enabled: bool) -> Result<()> {
    sqlx::query("UPDATE sessions SET answers_enabled = $1 WHERE id = $2")
        .bind(enabled)
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn start_round(pool: &PgPool, session_id: Uuid) -> Result<Uuid> {
    // Stop any ongoing round first to keep history clean.
    sqlx::query(
        "UPDATE rounds SET stopped_at = NOW() WHERE session_id = $1 AND stopped_at IS NULL",
    )
    .bind(session_id)
    .execute(pool)
    .await?;

    let round_id = Uuid::new_v4();
    sqlx::query("INSERT INTO rounds (id, session_id) VALUES ($1, $2)")
        .bind(round_id)
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(round_id)
}

pub async fn stop_round(pool: &PgPool, round_id: Uuid, winner: Option<Uuid>) -> Result<()> {
    sqlx::query(
        "UPDATE rounds SET stopped_at = NOW(), winner_id = $2 WHERE id = $1 AND stopped_at IS NULL",
    )
    .bind(round_id)
    .bind(winner)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn record_answer(
    pool: &PgPool,
    round_id: Uuid,
    participant_id: Uuid,
    elapsed_ms: i32,
) -> Result<DateTime<Utc>> {
    let id = Uuid::new_v4();
    let row: (DateTime<Utc>,) = sqlx::query_as(
        r#"INSERT INTO answers (id, round_id, participant_id, elapsed_ms)
           VALUES ($1, $2, $3, $4)
           ON CONFLICT (round_id, participant_id) DO UPDATE
             SET elapsed_ms = LEAST(answers.elapsed_ms, EXCLUDED.elapsed_ms)
           RETURNING server_received"#,
    )
    .bind(id)
    .bind(round_id)
    .bind(participant_id)
    .bind(elapsed_ms)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn close_session(pool: &PgPool, session_id: Uuid) -> Result<()> {
    sqlx::query("UPDATE sessions SET closed_at = NOW() WHERE id = $1 AND closed_at IS NULL")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_session(pool: &PgPool, session_id: Uuid) -> Result<()> {
    // ON DELETE CASCADE on participants/rounds/answers cleans the rest.
    sqlx::query("DELETE FROM sessions WHERE id = $1")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_expired_sessions(pool: &PgPool) -> Result<Vec<Uuid>> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM sessions WHERE expires_at <= NOW() OR closed_at IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}
