//! Environment-based configuration.
//!
//! All settings are read from environment variables (or a `.env` file in dev).
//! See `.env.example` for the full list.

use anyhow::{Context, Result};
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_host: String,
    pub bind_port: u16,
    pub database_url: String,
    pub session_ttl_seconds: i64,
    pub host_disconnect_grace_seconds: i64,
    pub public_backend_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let _ = dotenvy::dotenv();

        let bind_host = env::var("QH_BIND_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let bind_port: u16 = env::var("QH_BIND_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .context("QH_BIND_PORT must be a u16")?;

        let database_url = env::var("DATABASE_URL").or_else(|_| {
            let user = env::var("QH_DB_USER").unwrap_or_else(|_| "quiz".to_string());
            let pass = env::var("QH_DB_PASSWORD").unwrap_or_else(|_| "quiz".to_string());
            let host = env::var("QH_DB_HOST").unwrap_or_else(|_| "localhost".to_string());
            let port = env::var("QH_DB_PORT").unwrap_or_else(|_| "5432".to_string());
            let db = env::var("QH_DB_NAME").unwrap_or_else(|_| "quiz".to_string());
            Ok::<_, anyhow::Error>(format!(
                "postgres://{}:{}@{}:{}/{}",
                user, pass, host, port, db
            ))
        })?;

        let session_ttl_seconds: i64 = env::var("QH_SESSION_TTL_SECONDS")
            .unwrap_or_else(|_| "21600".to_string())
            .parse()
            .context("QH_SESSION_TTL_SECONDS must be i64")?;

        let host_disconnect_grace_seconds: i64 = env::var("QH_HOST_DISCONNECT_GRACE_SECONDS")
            .unwrap_or_else(|_| "60".to_string())
            .parse()
            .context("QH_HOST_DISCONNECT_GRACE_SECONDS must be i64")?;

        // Used by the frontend to know how to reach the backend (e.g. through
        // a Cloudflare tunnel or external DNS). Empty string = use same origin.
        let public_backend_url =
            env::var("QH_PUBLIC_BACKEND_URL").unwrap_or_else(|_| "".to_string());

        Ok(Self {
            bind_host,
            bind_port,
            database_url,
            session_ttl_seconds,
            host_disconnect_grace_seconds,
            public_backend_url,
        })
    }

    pub fn redacted_db_url(&self) -> String {
        match url_split(&self.database_url) {
            Some((scheme, _user, _pass, rest)) => format!("{}://***:***@{}", scheme, rest),
            None => "***".to_string(),
        }
    }
}

fn url_split(s: &str) -> Option<(&str, &str, &str, &str)> {
    let (scheme, rest) = s.split_once("://")?;
    let (creds, host_part) = rest.split_once('@')?;
    let (user, pass) = creds.split_once(':').unwrap_or((creds, ""));
    Some((scheme, user, pass, host_part))
}
