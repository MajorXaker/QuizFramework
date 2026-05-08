# Quiz Helper

A real-life quiz buzzer. The host creates a game on a laptop; players join from
their phones with an 8-character invite code. When the host starts a question,
all players see a big BUZZ button — the backend decides who answered first
based on the elapsed milliseconds reported by each client since they received
the `AllowAnswer` WebSocket signal.

The project is a single Rust service (Axum + Tokio + sqlx + PostgreSQL) that
also serves the static frontend, so deploying through a Cloudflare tunnel or
a single Portainer stack is straightforward.

## Architecture

```
┌────────────────────┐        ┌──────────────────────────────┐
│   Mobile browsers  │  WS    │  quiz-helper (Rust / Axum)   │
│  (players + host)  ├───────►│  REST + WS + static FE       │
└────────────────────┘        │                              │
                              │  in-memory hub per session   │
                              │  ── mpsc to each WS client   │
                              │                              │
                              │  PostgreSQL (sessions,       │
                              │  participants, rounds,       │
                              │  answers)                    │
                              └──────────────────────────────┘
```

* **Sessions** are short-lived: hard cap of 6 hours and torn down ~60s after
  the host disconnects (configurable). A background task runs every 15s to
  delete expired data.
* **Answer evaluation** is deterministic: lower client `elapsed_ms` wins;
  ties are broken by server-received timestamp, then by participant UUID.
* **State**: PostgreSQL is the persistent record for sessions, participants,
  rounds, and answers. An in-memory hub (`AppState.sessions`) holds the live
  WebSocket senders and the current round's atomic winner slot.

## Project layout

```
quiz-helper/
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml
├── .env.example
├── migrations/
│   ├── 0001_init.up.sql
│   └── 0001_init.down.sql
├── src/
│   ├── main.rs             # entrypoint + unit tests
│   ├── config.rs           # env-based config
│   ├── db.rs               # sqlx queries + invite-code generator
│   ├── state.rs            # in-memory hub + tie-break logic
│   ├── cleanup.rs          # 6h TTL + host-disconnect cleanup loop
│   ├── api/
│   │   ├── mod.rs          # router + static file serving
│   │   └── routes.rs       # REST endpoints
│   └── ws/
│       ├── mod.rs
│       ├── messages.rs     # client/server WS message types
│       └── handler.rs      # connection handler + broadcast helpers
└── static/
    ├── index.html          # mobile-first single-page UI
    ├── style.css
    └── app.js
```

## REST API

All bodies are JSON.

| Method | Path | Body | Notes |
|---|---|---|---|
| GET  | `/__heartbeat__` | — | Liveness probe. |
| GET  | `/api/config` | — | Returns `{backend_url, session_ttl_seconds}` for the FE. |
| POST | `/api/sessions` | `{host_name}` | Create a session. Caller becomes host. |
| POST | `/api/sessions/join` | `{invite_code, display_name}` | Join an existing session. |
| POST | `/api/sessions/:sid/regenerate_code` | `{host_id}` | Issue a new invite code. |
| POST | `/api/sessions/:sid/joins_enabled` | `{host_id, enabled}` | Toggle joins. |
| POST | `/api/sessions/:sid/answers_enabled` | `{host_id, enabled}` | Toggle answer mode. |
| POST | `/api/sessions/:sid/start_round` | `{host_id}` | Start a question; broadcasts `allow_answer`. |
| POST | `/api/sessions/:sid/stop_round` | `{host_id}` | Stop the current question; broadcasts `stop_answer`. |
| POST | `/api/sessions/:sid/participants/:pid/kick` | `{host_id}` | Remove a player. |
| POST | `/api/sessions/:sid/close` | `{host_id}` | End and delete the session immediately. |

The `host_id` field is the host's `participant_id` returned at creation time
and acts as a bearer credential for host-only actions. Anyone with the
session UUID *and* the host id can act as host — keep the host id private.

## WebSocket protocol

Connect to:

```
wss://<backend>/ws?session_id=<UUID>&participant_id=<UUID>
```

Messages are JSON objects with a `type` discriminator.

### Client → server

```json
{"type":"answer","elapsed_ms":42}
{"type":"ping"}
```

`elapsed_ms` is the time the player waited between receiving the most recent
`allow_answer` and pressing the buzzer, measured with
`performance.now()`.

### Server → client

```json
{"type":"welcome", "session_id":"…", "participant_id":"…", "is_host":false,
 "invite_code":"abcd1234", "joins_enabled":true, "answers_enabled":false,
 "participants":[{"id":"…","name":"Alice","is_host":true,"connected":true}],
 "expires_at":"2026-05-08T15:00:00Z"}

{"type":"session_state", "invite_code":"…","joins_enabled":true,"answers_enabled":true}

{"type":"participants", "participants":[ … ]}

{"type":"allow_answer", "round_id":"…"}

{"type":"answer_winner", "round_id":"…",
 "winner":{"participant_id":"…","participant_name":"Alice","elapsed_ms":42}}

{"type":"stop_answer", "round_id":"…",
 "winner":{ … } }     // winner may be null if no one answered

{"type":"kicked"}
{"type":"session_closed", "reason":"…"}
{"type":"pong"}
{"type":"error", "message":"…"}
```

### Tie-breaking

The first answer to arrive for a round always wins its slot; subsequent
answers can only displace it if `(elapsed_ms, server_received, participant_id)`
is lexicographically smaller. In practice the first to arrive almost always
has the lowest elapsed time, so the in-memory check is fast and deterministic.

## Running locally (no Docker)

```bash
# 1. Bring up Postgres however you like, e.g.:
docker run --rm -d --name quiz-pg -p 5432:5432 \
  -e POSTGRES_USER=quiz -e POSTGRES_PASSWORD=quiz -e POSTGRES_DB=quiz postgres:16-alpine

# 2. Configure the app.
cp .env.example .env
# edit .env if needed

# 3. Build & run.
cargo run --release
# → HTTP server listening on 0.0.0.0:8080

# 4. Open http://localhost:8080 on your laptop (host) and on a phone
# connected to the same network (player). Or use a tunnel.
```

## Running with Docker / Portainer

```bash
cp .env.example .env  # set QH_DB_PASSWORD and QH_PUBLIC_BACKEND_URL
docker compose up -d --build
# → http://<host>:8080
```

The `docker-compose.yml` is Portainer-friendly: deploy as a Stack, paste the
file, and provide environment variables in the Portainer UI. The compose file
exposes only ports/volumes that are needed; the `db` service is reachable only
from the `app` service.

To put the service behind a Cloudflare tunnel, point the tunnel at
`http://app:8080` (or `host:${QH_PUBLISH_PORT}`) and set
`QH_PUBLIC_BACKEND_URL=https://your.public.host` so the frontend connects WS
back to the same origin.

## Configuration reference

| Env var | Default | Purpose |
|---|---|---|
| `QH_BIND_HOST` | `0.0.0.0` | TCP bind address |
| `QH_BIND_PORT` | `8080` | TCP port |
| `DATABASE_URL` | (built from QH_DB_*) | Full Postgres URL; takes precedence |
| `QH_DB_HOST/PORT/USER/PASSWORD/NAME` | — | Used if `DATABASE_URL` not set |
| `QH_SESSION_TTL_SECONDS` | `21600` | Hard 6h cap on session lifetime |
| `QH_HOST_DISCONNECT_GRACE_SECONDS` | `60` | Tear down sessions whose host has been gone this long |
| `QH_PUBLIC_BACKEND_URL` | (empty) | Tells the FE where to connect (empty = same origin) |
| `RUST_LOG` | `info` | Tracing filter |

## Development

```bash
cargo fmt --check
cargo check
cargo test          # unit tests for tie-break + invite-code generator
```

The full integration with Postgres requires a running database and the
schema migrated; the unit tests included don't need it.
