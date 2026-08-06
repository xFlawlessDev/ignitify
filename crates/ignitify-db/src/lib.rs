//! SQLite persistence for Ignitify.

mod database;
mod error;
mod models;
mod repositories;

pub use database::{Database, DatabaseConfig};
pub use error::{DatabaseError, Result};
pub use models::{RefreshTokenRecord, RotateRefreshTokenOutcome, UserRecord, UserRole};
pub use repositories::{
    ActivityActor, ActivityRecord, ActivityRepository, AuthorizedDeploymentService,
    AuthorizedService, CreateDeploymentOutcome, DashboardActor, DashboardDeploymentRecord,
    DashboardProjectRecord, DashboardRecords, DashboardRepository, DashboardServiceRecord,
    DeploymentActor, DeploymentEventRecord, DeploymentLogRecord, DeploymentRecord,
    DeploymentVariableRecord, DeploymentsRepository, DomainActor, DomainMutationOutcome,
    DomainRecord, DomainsRepository, EnvironmentsRepository, NewDeployment, NewDeploymentLog,
    NewServiceVariable, ProjectActor, ProjectUpdateOutcome, ProjectsRepository,
    RefreshTokensRepository, SequenceCursor, ServiceActor, ServiceMutationOutcome,
    ServiceVariableRecord, ServicesRepository, UsersRepository,
};

#[cfg(test)]
mod tests;
