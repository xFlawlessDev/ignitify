//! Host terminal sessions backed by the local platform PTY.

mod error;
mod service;

pub use error::{Result, TerminalError};
pub use service::{TerminalEvent, TerminalService, TerminalSession};
