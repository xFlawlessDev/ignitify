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
    AuthorizedProjectVariables, AuthorizedService, BackupDestinationsRepository,
    BackupS3DestinationConnection, BackupS3DestinationRecord, CancelDeploymentOutcome,
    CreateDeploymentOutcome, DashboardActor, DashboardDeploymentRecord, DashboardProjectRecord,
    DashboardRecords, DashboardRepository, DashboardServiceRecord, DeploymentActor,
    DeploymentEventRecord, DeploymentLogRecord, DeploymentRecord, DeploymentVariableRecord,
    DeploymentsRepository, DomainActor, DomainMutationOutcome, DomainRecord,
    DomainVerificationRequestOutcome, DomainsRepository, EnvironmentsRepository,
    NewBackupS3Destination, NewDeployment, NewDeploymentLog, NewProjectVariable, NewProvider,
    NewRemoteBuilder, NewServerCertificate, NewServiceVariable, ProjectActor, ProjectRemoveOutcome,
    ProjectUpdateOutcome, ProjectVariableRecord, ProjectVariablesMutationOutcome,
    ProjectsRepository, ProviderAuthMode, ProviderKind, ProviderMutationOutcome, ProviderRecord,
    ProviderUpdate, ProvidersRepository, RefreshTokensRepository, RemoteBuilderConnection,
    RemoteBuilderRecord, RemoteBuilderUpdate, RemoteBuildersRepository, RetrySchedule,
    SequenceCursor, ServerCertificateRecord, ServerSettingsRecord, ServerSettingsRepository,
    ServerSettingsUpdate, ServiceActor, ServiceMutationOutcome, ServiceVariableRecord,
    ServicesRepository, UsersRepository,
};

#[cfg(test)]
mod tests;
