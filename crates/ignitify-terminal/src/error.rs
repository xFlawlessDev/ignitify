use thiserror::Error;

#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("host terminal is unavailable")]
    Unavailable,
    #[error("terminal session is closed")]
    Closed,
}

pub type Result<T> = std::result::Result<T, TerminalError>;
