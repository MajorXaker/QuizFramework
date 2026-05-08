//! WebSocket protocol & handler.
//!
//! Wire format: JSON objects with a `type` discriminator. See README for the
//! full protocol. The server is the source of truth; clients send signed
//! actions and receive broadcasts.

pub mod handler;
pub mod messages;

pub use handler::ws_handler;
