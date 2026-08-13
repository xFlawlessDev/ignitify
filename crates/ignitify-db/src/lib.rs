//! SQLite persistence for Ignitify.

mod database;
mod error;
mod models;
mod repositories;

pub use database::{Database, DatabaseConfig};
pub use error::{DatabaseError, Result};
pub use models::{RefreshTokenRecord, RotateRefreshTokenOutcome, UserRecord, UserRole};
pub use repositories::{
    ActivityActor, ActivityRecord, ActivityRepository, AiSettingsConnection, AiSettingsRecord,
    AiSettingsRepository, AuditContext, AuditOutcome, AuthorizedDeploymentService,
    AuthorizedProjectVariables, AuthorizedService, AutoDeployWebhookTarget,
    BackupDestinationsRepository, BackupOperationsSummary, BackupRunSummary,
    BackupS3DestinationConnection, BackupS3DestinationRecord, BackupS3RunRecord,
    CancelDeploymentOutcome, CertificateOperationsSummary, CreateDeploymentOutcome, DashboardActor,
    DashboardDeploymentRecord, DashboardProjectRecord, DashboardRecords, DashboardRepository,
    DashboardServiceRecord, DeploymentActor, DeploymentEventRecord, DeploymentLogRecord,
    DeploymentOperationsSummary, DeploymentRecord, DeploymentVariableRecord, DeploymentsRepository,
    DomainActor, DomainMutationOutcome, DomainOperationsSummary, DomainRecord,
    DomainVerificationRequestOutcome, DomainsRepository, EnvironmentsRepository, NewAiSettings,
    NewBackupS3Destination, NewDeployment, NewDeploymentLog, NewNotificationChannel,
    NewProjectVariable, NewProvider, NewRemoteBuilder, NewRemoteServer, NewServerCertificate,
    NewServiceVariable, NewUptimeMonitor, NotificationChannelConnection, NotificationChannelRecord,
    NotificationChannelsRepository, NotificationDeliveryRecord, OperationalAlertEvent,
    OperationalAlertTransition, OperationsRepository, OperationsSummary, ProjectActor,
    ProjectRemoveOutcome, ProjectUpdateOutcome, ProjectVariableRecord,
    ProjectVariablesMutationOutcome, ProjectsRepository, ProviderAuthMode, ProviderKind,
    ProviderMutationOutcome, ProviderRecord, ProviderUpdate, ProvidersRepository,
    RefreshTokensRepository, RemoteAgentOperationsSummary, RemoteBuilderConnection,
    RemoteBuilderRecord, RemoteBuilderUpdate, RemoteBuildersRepository,
    RemoteNotificationEventRecord, RemoteServerAgentHeartbeat, RemoteServerAgentRecord,
    RemoteServerAgentsRepository, RemoteServerConnection, RemoteServerRecord, RemoteServerUpdate,
    RemoteServersRepository, RetrySchedule, SequenceCursor, ServerCertificateRecord,
    ServerSettingsRecord, ServerSettingsRepository, ServerSettingsUpdate, ServiceActor,
    ServiceMutationOutcome, ServiceVariableRecord, ServicesRepository, UptimeCheckUpdate,
    UptimeMonitorRecord, UptimeMonitorUpdate, UptimeMonitorsRepository, UsersRepository,
};

#[cfg(test)]
mod tests;
