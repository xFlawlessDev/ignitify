//! SQLite persistence for Ignitify.

mod database;
mod error;
mod models;
mod repositories;

pub use database::{Database, DatabaseConfig};
pub use error::{DatabaseError, Result};
pub use models::{RefreshTokenRecord, RotateRefreshTokenOutcome, UserRecord, UserRole};
pub use repositories::{
    EnvironmentsRepository, ProjectActor, ProjectUpdateOutcome, ProjectsRepository,
    RefreshTokensRepository, UsersRepository,
};

#[cfg(test)]
mod tests;
