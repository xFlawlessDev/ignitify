//! Durable deployment submission, encrypted snapshots, and worker orchestration.

mod domain_dns;
mod health;
mod runtime;

pub use domain_dns::{
    DnsVerificationResult, DnsVerifier, NoopDnsVerifier, reconcile_dns_verifications,
};
pub use health::{
    HostRuntimeMetrics, RuntimeContainer, RuntimeHealth, RuntimePort, StaticRuntimeHealth,
    StaticSystemMetrics, SystemMetricsProvider, SystemMetricsSnapshot, WorkerHealth,
};
pub use runtime::{
    ImageRuntime, Ingress, IngressRoute, NoRemoteRuntime, NoopSourceBuild, RuntimeDeployment,
    RuntimeLog, RuntimeObservation, RuntimeSelector, SourceBuild, SourceBuildOutput,
    WorkerDependencies,
};

use std::{
    collections::{BTreeMap, HashSet},
    io::Write,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use age::{Decryptor, Encryptor, x25519};
use chrono::{DateTime, Utc};
use ignitify_db::{
    AuthorizedDeploymentService, AuthorizedProjectVariables, AuthorizedService,
    CancelDeploymentOutcome, CreateDeploymentOutcome, DeploymentActor, DeploymentRecord,
    DeploymentsRepository, DomainsRepository, NewDeployment, NewProjectVariable,
    NewServiceVariable, ProjectActor, ProjectVariablesMutationOutcome, ProjectsRepository,
    RetrySchedule, ServiceActor, ServiceMutationOutcome, ServiceVariableRecord, ServicesRepository,
};
use ignitify_domain::{
    DeploymentState, ServiceInput, ServiceVariableInput, validate_variable_inputs,
};
use thiserror::Error;
use tokio::{
    sync::{broadcast, mpsc},
    task::JoinSet,
};
use uuid::Uuid;
use zeroize::Zeroizing;

const MAX_RUNTIME_START_ATTEMPTS: i64 = 3;
const HEALTH_GATE_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_CONCURRENT_DEPLOYMENT_JOBS: usize = 32;

type AutoDeploySecretUpdate = Option<Option<String>>;
type AutoDeploySecretUpdatePreparation = (AutoDeploySecretUpdate, Option<Zeroizing<String>>);

#[derive(Clone)]
pub struct ServiceControl {
    cipher: Arc<AgeCipher>,
    projects: ProjectsRepository,
    services: ServicesRepository,
}

impl ServiceControl {
    pub fn new(
        services: ServicesRepository,
        projects: ProjectsRepository,
        identity: impl AsRef<str>,
    ) -> Result<Self> {
        Ok(Self {
            cipher: Arc::new(AgeCipher::from_identity(identity)?),
            projects,
            services,
        })
    }

    pub async fn project_environment(
        &self,
        actor: ProjectActor<'_>,
        project_id: &str,
    ) -> Result<Option<ProjectEnvironmentReadModel>> {
        self.projects
            .variables(actor, project_id)
            .await?
            .map(|environment| self.read_project_environment(environment))
            .transpose()
    }

    pub async fn update_project_environment(
        &self,
        actor: ProjectActor<'_>,
        project_id: &str,
        variables: Vec<ProjectEnvironmentVariableInput>,
    ) -> Result<ProjectEnvironmentMutationModel> {
        let Some(current) = self.projects.variables(actor.clone(), project_id).await? else {
            return Ok(ProjectEnvironmentMutationModel::Missing);
        };
        if !actor.is_admin && !current.role.can_manage_services() {
            return Ok(ProjectEnvironmentMutationModel::Forbidden);
        }

        let existing = current
            .variables
            .iter()
            .map(|variable| (variable.key.as_str(), variable))
            .collect::<std::collections::HashMap<_, _>>();
        let mut validation_inputs = Vec::with_capacity(variables.len());
        let mut encrypted = Vec::with_capacity(variables.len());
        for variable in variables {
            let key = variable.key.clone();
            let value = match variable.value {
                Some(value) => {
                    validation_inputs.push(ServiceVariableInput {
                        key: key.clone(),
                        value: value.clone(),
                        is_secret: variable.is_secret,
                    });
                    Some(value)
                }
                None if variable.is_secret
                    && existing
                        .get(key.as_str())
                        .is_some_and(|stored| stored.is_secret) =>
                {
                    validation_inputs.push(ServiceVariableInput {
                        key: key.clone(),
                        value: String::new(),
                        is_secret: variable.is_secret,
                    });
                    None
                }
                None => return Err(ignitify_domain::InputError::InvalidVariableValue.into()),
            };
            encrypted.push((key, variable.is_secret, value));
        }
        validate_variable_inputs(&validation_inputs)?;
        let encrypted = encrypted
            .into_iter()
            .map(|(key, is_secret, value)| {
                let ciphertext = if let Some(value) = value {
                    self.cipher.encrypt(Zeroizing::new(value).as_bytes())?
                } else {
                    existing
                        .get(key.as_str())
                        .ok_or(Error::InvalidCiphertext)?
                        .ciphertext
                        .clone()
                };
                Ok(NewProjectVariable {
                    key,
                    is_secret,
                    ciphertext,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(
            match self
                .projects
                .replace_variables(actor, project_id, encrypted)
                .await?
            {
                ProjectVariablesMutationOutcome::Updated(environment) => {
                    ProjectEnvironmentMutationModel::Updated(
                        self.read_project_environment(environment)?,
                    )
                }
                ProjectVariablesMutationOutcome::Missing => {
                    ProjectEnvironmentMutationModel::Missing
                }
                ProjectVariablesMutationOutcome::Forbidden => {
                    ProjectEnvironmentMutationModel::Forbidden
                }
            },
        )
    }

    pub async fn list(
        &self,
        actor: ServiceActor<'_>,
        project_id: &str,
    ) -> Result<Option<Vec<ServiceReadModel>>> {
        let services = self.services.list(actor, project_id).await?;
        services
            .map(|services| {
                services
                    .into_iter()
                    .map(|service| self.read_model(service))
                    .collect()
            })
            .transpose()
    }

    pub async fn get(
        &self,
        actor: ServiceActor<'_>,
        service_id: &str,
    ) -> Result<Option<ServiceReadModel>> {
        self.services
            .get(actor, service_id)
            .await?
            .map(|service| self.read_model(service))
            .transpose()
    }

    pub async fn create(
        &self,
        actor: ServiceActor<'_>,
        project_id: &str,
        input: ServiceInput,
    ) -> Result<ServiceMutationOutcomeModel> {
        let (configuration, variables) = self.encrypt_variables(input)?;
        let (auto_deploy_secret_ciphertext, auto_deploy_webhook_secret) =
            self.auto_deploy_secret_for_create(&configuration)?;
        Ok(
            match self
                .services
                .create(
                    actor,
                    project_id,
                    configuration,
                    variables,
                    auto_deploy_secret_ciphertext,
                )
                .await?
            {
                ServiceMutationOutcome::Created(service) => ServiceMutationOutcomeModel::Created(
                    self.read_model_with_auto_deploy_secret(service, auto_deploy_webhook_secret)?,
                ),
                ServiceMutationOutcome::Updated(_) => {
                    unreachable!("service create cannot return update")
                }
                ServiceMutationOutcome::Removed(_) => {
                    unreachable!("service create cannot return remove")
                }
                ServiceMutationOutcome::Missing => ServiceMutationOutcomeModel::Missing,
                ServiceMutationOutcome::Forbidden => ServiceMutationOutcomeModel::Forbidden,
            },
        )
    }

    pub async fn update(
        &self,
        actor: ServiceActor<'_>,
        service_id: &str,
        input: ServiceInput,
    ) -> Result<ServiceMutationOutcomeModel> {
        self.update_preserving_secrets(actor, service_id, input, HashSet::new())
            .await
    }

    pub async fn update_preserving_secrets(
        &self,
        actor: ServiceActor<'_>,
        service_id: &str,
        input: ServiceInput,
        preserved_secret_keys: HashSet<String>,
    ) -> Result<ServiceMutationOutcomeModel> {
        let Some(current) = self.services.get(actor, service_id).await? else {
            return Ok(ServiceMutationOutcomeModel::Missing);
        };
        if !actor.is_admin && !current.role.can_manage_services() {
            return Ok(ServiceMutationOutcomeModel::Forbidden);
        }
        let (configuration, variables) = self.encrypt_variables_preserving_secrets(
            input,
            &current.variables,
            &preserved_secret_keys,
        )?;
        let (auto_deploy_secret_ciphertext, auto_deploy_webhook_secret) = self
            .auto_deploy_secret_for_update(
                &configuration,
                current.auto_deploy_secret_ciphertext.as_deref(),
            )?;
        Ok(
            match self
                .services
                .update(
                    actor,
                    service_id,
                    configuration,
                    variables,
                    auto_deploy_secret_ciphertext,
                )
                .await?
            {
                ServiceMutationOutcome::Created(_) => {
                    unreachable!("service update cannot return create")
                }
                ServiceMutationOutcome::Removed(_) => {
                    unreachable!("service update cannot return remove")
                }
                ServiceMutationOutcome::Updated(service) => ServiceMutationOutcomeModel::Updated(
                    self.read_model_with_auto_deploy_secret(service, auto_deploy_webhook_secret)?,
                ),
                ServiceMutationOutcome::Missing => ServiceMutationOutcomeModel::Missing,
                ServiceMutationOutcome::Forbidden => ServiceMutationOutcomeModel::Forbidden,
            },
        )
    }

    pub async fn remove(
        &self,
        actor: ServiceActor<'_>,
        service_id: &str,
        confirm_name: &str,
    ) -> Result<ServiceMutationOutcomeModel> {
        Ok(
            match self
                .services
                .remove(actor, service_id, confirm_name)
                .await?
            {
                ServiceMutationOutcome::Removed(_) => ServiceMutationOutcomeModel::Removed,
                ServiceMutationOutcome::Missing => ServiceMutationOutcomeModel::Missing,
                ServiceMutationOutcome::Forbidden => ServiceMutationOutcomeModel::Forbidden,
                ServiceMutationOutcome::Created(_) => {
                    unreachable!("service remove cannot return create")
                }
                ServiceMutationOutcome::Updated(_) => {
                    unreachable!("service remove cannot return update")
                }
            },
        )
    }

    pub async fn rotate_auto_deploy_webhook_secret(
        &self,
        actor: ServiceActor<'_>,
        service_id: &str,
    ) -> Result<AutoDeploySecretRotation> {
        let Some(service) = self.services.get(actor, service_id).await? else {
            return Ok(AutoDeploySecretRotation::Missing);
        };
        if !actor.is_admin && !service.role.can_manage_services() {
            return Ok(AutoDeploySecretRotation::Forbidden);
        }
        if !service
            .source_config
            .as_ref()
            .is_some_and(|source| source.auto_deploy)
        {
            return Ok(AutoDeploySecretRotation::Disabled);
        }
        let secret = generate_auto_deploy_secret();
        let ciphertext = self.cipher.encrypt(secret.as_bytes())?;
        match self
            .services
            .rotate_auto_deploy_secret(actor, service_id, ciphertext)
            .await?
        {
            ServiceMutationOutcome::Updated(_) => Ok(AutoDeploySecretRotation::Rotated(secret)),
            ServiceMutationOutcome::Missing => Ok(AutoDeploySecretRotation::Missing),
            ServiceMutationOutcome::Forbidden => Ok(AutoDeploySecretRotation::Forbidden),
            ServiceMutationOutcome::Created(_) | ServiceMutationOutcome::Removed(_) => {
                unreachable!("auto deploy secret rotation cannot create or remove a service")
            }
        }
    }

    pub async fn auto_deploy_webhook_target(
        &self,
        service_id: &str,
    ) -> Result<Option<AutoDeployWebhookTargetModel>> {
        let Some(target) = self.services.auto_deploy_webhook_target(service_id).await? else {
            return Ok(None);
        };
        let plaintext = self.cipher.decrypt(&target.secret_ciphertext)?;
        let secret =
            std::str::from_utf8(plaintext.as_slice()).map_err(|_| Error::InvalidCiphertext)?;
        if secret.is_empty() {
            return Err(Error::InvalidCiphertext);
        }
        Ok(Some(AutoDeployWebhookTargetModel {
            service_id: target.service_id,
            provider_id: target.provider_id,
            repository: target.repository,
            branch: target.branch,
            secret: Zeroizing::new(secret.to_owned()),
            project_owner_id: target.project_owner_id,
        }))
    }

    fn encrypt_variables(
        &self,
        input: ServiceInput,
    ) -> Result<(
        ignitify_domain::ServiceConfiguration,
        Vec<NewServiceVariable>,
    )> {
        let variables = input
            .variables
            .into_iter()
            .map(|variable| {
                let plaintext = Zeroizing::new(variable.value);
                Ok(NewServiceVariable {
                    key: variable.key,
                    is_secret: variable.is_secret,
                    ciphertext: self.cipher.encrypt(plaintext.as_bytes())?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((input.configuration, variables))
    }

    fn auto_deploy_secret_for_create(
        &self,
        configuration: &ignitify_domain::ServiceConfiguration,
    ) -> Result<(Option<String>, Option<Zeroizing<String>>)> {
        if !configuration
            .source_config
            .as_ref()
            .is_some_and(|source| source.auto_deploy)
        {
            return Ok((None, None));
        }
        let secret = generate_auto_deploy_secret();
        let ciphertext = self.cipher.encrypt(secret.as_bytes())?;
        Ok((Some(ciphertext), Some(secret)))
    }

    fn auto_deploy_secret_for_update(
        &self,
        configuration: &ignitify_domain::ServiceConfiguration,
        current_secret_ciphertext: Option<&str>,
    ) -> Result<AutoDeploySecretUpdatePreparation> {
        let auto_deploy_enabled = configuration
            .source_config
            .as_ref()
            .is_some_and(|source| source.auto_deploy);
        if !auto_deploy_enabled {
            return Ok((current_secret_ciphertext.is_some().then_some(None), None));
        }
        if current_secret_ciphertext.is_some() {
            return Ok((None, None));
        }
        let secret = generate_auto_deploy_secret();
        let ciphertext = self.cipher.encrypt(secret.as_bytes())?;
        Ok((Some(Some(ciphertext)), Some(secret)))
    }

    fn encrypt_variables_preserving_secrets(
        &self,
        input: ServiceInput,
        existing_variables: &[ServiceVariableRecord],
        preserved_secret_keys: &HashSet<String>,
    ) -> Result<(
        ignitify_domain::ServiceConfiguration,
        Vec<NewServiceVariable>,
    )> {
        let variables = input
            .variables
            .into_iter()
            .map(|variable| {
                if preserved_secret_keys.contains(&variable.key) {
                    if !variable.is_secret || !variable.value.is_empty() {
                        return Err(Error::Policy(
                            "a stored secret can only be preserved without a replacement value",
                        ));
                    }
                    let existing = existing_variables
                        .iter()
                        .find(|existing| existing.key == variable.key && existing.is_secret)
                        .ok_or(Error::Policy("the stored secret no longer exists"))?;
                    return Ok(NewServiceVariable {
                        key: variable.key,
                        is_secret: true,
                        ciphertext: existing.ciphertext.clone(),
                    });
                }
                let plaintext = Zeroizing::new(variable.value);
                Ok(NewServiceVariable {
                    key: variable.key,
                    is_secret: variable.is_secret,
                    ciphertext: self.cipher.encrypt(plaintext.as_bytes())?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((input.configuration, variables))
    }

    fn read_model(&self, service: AuthorizedService) -> Result<ServiceReadModel> {
        let can_read_values = service.role.can_manage_services();
        let variables = service
            .variables
            .into_iter()
            .map(|variable| {
                let value = if variable.is_secret || !can_read_values {
                    None
                } else {
                    let plaintext = self.cipher.decrypt(&variable.ciphertext)?;
                    let value = std::str::from_utf8(plaintext.as_slice())
                        .map_err(|_| Error::InvalidCiphertext)?;
                    Some(Zeroizing::new(value.to_owned()))
                };
                Ok(ServiceVariableReadModel {
                    key: variable.key,
                    is_secret: variable.is_secret,
                    is_set: true,
                    value,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(ServiceReadModel {
            id: service.id.to_string(),
            project_id: service.project_id.to_string(),
            environment_id: service.environment_id.to_string(),
            role: service.role.as_str().to_owned(),
            name: service.name,
            kind: service.kind.as_str().to_owned(),
            spec: service.spec,
            source_config: service.source_config,
            auto_deploy_webhook_secret: None,
            deployment_destination_id: service.deployment_destination_id,
            desired_generation: service.desired_generation,
            desired_state: service.desired_state,
            created_at: service.created_at,
            updated_at: service.updated_at,
            variables,
        })
    }

    fn read_model_with_auto_deploy_secret(
        &self,
        service: AuthorizedService,
        auto_deploy_webhook_secret: Option<Zeroizing<String>>,
    ) -> Result<ServiceReadModel> {
        let mut model = self.read_model(service)?;
        model.auto_deploy_webhook_secret = auto_deploy_webhook_secret;
        Ok(model)
    }

    fn read_project_environment(
        &self,
        environment: AuthorizedProjectVariables,
    ) -> Result<ProjectEnvironmentReadModel> {
        let can_read_values = environment.role.can_manage_services();
        let variables = environment
            .variables
            .into_iter()
            .map(|variable| {
                let value = if variable.is_secret || !can_read_values {
                    None
                } else {
                    let plaintext = self.cipher.decrypt(&variable.ciphertext)?;
                    let value = std::str::from_utf8(plaintext.as_slice())
                        .map_err(|_| Error::InvalidCiphertext)?;
                    Some(Zeroizing::new(value.to_owned()))
                };
                Ok(ProjectEnvironmentVariableReadModel {
                    key: variable.key,
                    is_secret: variable.is_secret,
                    is_set: true,
                    value,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(ProjectEnvironmentReadModel {
            variables,
            role: environment.role.as_str().to_owned(),
        })
    }
}

#[derive(Clone)]
pub struct ControlHandle {
    cipher: Arc<AgeCipher>,
    deployments: DeploymentsRepository,
    wake: mpsc::Sender<()>,
    publisher: StreamPublisher,
}

impl ControlHandle {
    pub fn new(
        deployments: DeploymentsRepository,
        identity: impl AsRef<str>,
    ) -> Result<(Self, mpsc::Receiver<()>)> {
        let (wake, receiver) = mpsc::channel(16);
        let (stream, _) = broadcast::channel(256);
        Ok((
            Self {
                cipher: Arc::new(AgeCipher::from_identity(identity)?),
                deployments,
                wake,
                publisher: StreamPublisher::new(stream),
            },
            receiver,
        ))
    }

    pub async fn submit_deploy(
        &self,
        actor: DeploymentActor<'_>,
        service_id: &str,
        idempotency_key: &str,
    ) -> Result<DeploymentSubmission> {
        self.submit_deploy_with_source_revision(actor, service_id, idempotency_key, None)
            .await
    }

    pub async fn submit_deploy_with_source_revision(
        &self,
        actor: DeploymentActor<'_>,
        service_id: &str,
        idempotency_key: &str,
        source_revision: Option<&str>,
    ) -> Result<DeploymentSubmission> {
        validate_idempotency_key(idempotency_key)?;
        if let Some(source_revision) = source_revision {
            validate_source_revision(source_revision)?;
        }
        let Some(service) = self
            .deployments
            .service_for_deployment(actor, service_id)
            .await?
        else {
            return Ok(DeploymentSubmission::Missing);
        };
        if !actor.is_admin && !service.role.can_manage_services() {
            return Ok(DeploymentSubmission::Forbidden);
        }
        let variables_ciphertext = self.snapshot_variables(&service)?;
        let spec = service.spec.clone();
        let source_config = service.source_config.clone();
        let outcome = self
            .deployments
            .create(
                actor,
                service_id,
                NewDeployment {
                    idempotency_key: idempotency_key.to_owned(),
                    requested_by_user_id: actor.id.to_owned(),
                    spec,
                    source_config,
                    deployment_destination_id: service.deployment_destination_id.clone(),
                    source_revision: source_revision.map(str::to_owned),
                    variables_ciphertext,
                },
            )
            .await?;
        if let CreateDeploymentOutcome::Created(record) = &outcome {
            self.publish_deployment_records(record.id.as_str()).await?;
            let _ = self.wake.try_send(());
        }
        Ok(match outcome {
            CreateDeploymentOutcome::Created(record) => DeploymentSubmission::Accepted(record),
            CreateDeploymentOutcome::Existing(record) => DeploymentSubmission::Existing(record),
            CreateDeploymentOutcome::Missing => DeploymentSubmission::Missing,
            CreateDeploymentOutcome::Forbidden => DeploymentSubmission::Forbidden,
            CreateDeploymentOutcome::ActiveConflict => DeploymentSubmission::ActiveConflict,
        })
    }

    pub async fn submit_rollback(
        &self,
        actor: DeploymentActor<'_>,
        deployment_id: &str,
        idempotency_key: &str,
    ) -> Result<DeploymentSubmission> {
        validate_idempotency_key(idempotency_key)?;
        let repository_key = format!("\u{1f}rollback:{deployment_id}:{idempotency_key}");
        let outcome = self
            .deployments
            .rollback(actor, deployment_id, &repository_key)
            .await?;
        if let CreateDeploymentOutcome::Created(record) = &outcome {
            self.publish_deployment_records(record.id.as_str()).await?;
            let _ = self.wake.try_send(());
        }
        Ok(match outcome {
            CreateDeploymentOutcome::Created(record) => DeploymentSubmission::Accepted(record),
            CreateDeploymentOutcome::Existing(record) => DeploymentSubmission::Existing(record),
            CreateDeploymentOutcome::Missing => DeploymentSubmission::Missing,
            CreateDeploymentOutcome::Forbidden => DeploymentSubmission::Forbidden,
            CreateDeploymentOutcome::ActiveConflict => DeploymentSubmission::ActiveConflict,
        })
    }

    pub async fn submit_cancel(
        &self,
        actor: DeploymentActor<'_>,
        deployment_id: &str,
    ) -> Result<DeploymentSubmission> {
        let outcome = self.deployments.cancel(actor, deployment_id).await?;
        if let CancelDeploymentOutcome::Cancelled(record) = &outcome {
            self.publish_deployment_records(record.id.as_str()).await?;
            let _ = self.wake.try_send(());
        }
        Ok(match outcome {
            CancelDeploymentOutcome::Cancelled(record) => DeploymentSubmission::Accepted(record),
            CancelDeploymentOutcome::Existing(record) => DeploymentSubmission::Existing(record),
            CancelDeploymentOutcome::Missing => DeploymentSubmission::Missing,
            CancelDeploymentOutcome::Forbidden => DeploymentSubmission::Forbidden,
        })
    }

    pub async fn list(
        &self,
        actor: DeploymentActor<'_>,
        service_id: &str,
        before_created_at: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Option<Vec<DeploymentRecord>>> {
        Ok(self
            .deployments
            .list(actor, service_id, before_created_at, limit)
            .await?)
    }

    pub async fn list_for_project(
        &self,
        actor: DeploymentActor<'_>,
        project_id: &str,
        before_created_at: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Option<Vec<DeploymentRecord>>> {
        Ok(self
            .deployments
            .list_for_project(actor, project_id, before_created_at, limit)
            .await?)
    }

    pub fn worker_cipher(&self) -> Arc<AgeCipher> {
        self.cipher.clone()
    }

    pub fn wake_worker(&self) -> Result<()> {
        self.wake.try_send(()).map_err(|_| Error::WorkerUnavailable)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<StreamRecord> {
        self.publisher.sender.subscribe()
    }

    pub fn worker_publisher(&self) -> StreamPublisher {
        self.publisher.clone()
    }

    pub async fn events(
        &self,
        actor: DeploymentActor<'_>,
        deployment_id: &str,
        after: i64,
        through: i64,
    ) -> Result<Option<Vec<ignitify_db::DeploymentEventRecord>>> {
        if self.get(actor, deployment_id).await?.is_none() {
            return Ok(None);
        }
        Ok(Some(
            self.deployments
                .events_after(deployment_id, after, through)
                .await?,
        ))
    }

    pub async fn logs(
        &self,
        actor: DeploymentActor<'_>,
        deployment_id: &str,
        after: i64,
        through: i64,
    ) -> Result<Option<Vec<ignitify_db::DeploymentLogRecord>>> {
        if self.get(actor, deployment_id).await?.is_none() {
            return Ok(None);
        }
        Ok(Some(
            self.deployments
                .logs_after(deployment_id, after, through)
                .await?,
        ))
    }

    pub async fn event_cursor(
        &self,
        actor: DeploymentActor<'_>,
        deployment_id: &str,
    ) -> Result<Option<ignitify_db::SequenceCursor>> {
        if self.get(actor, deployment_id).await?.is_none() {
            return Ok(None);
        }
        Ok(Some(self.deployments.event_cursor(deployment_id).await?))
    }

    pub async fn log_cursor(
        &self,
        actor: DeploymentActor<'_>,
        deployment_id: &str,
    ) -> Result<Option<ignitify_db::SequenceCursor>> {
        if self.get(actor, deployment_id).await?.is_none() {
            return Ok(None);
        }
        Ok(Some(self.deployments.log_cursor(deployment_id).await?))
    }

    pub fn decrypt_variables(&self, ciphertext: &str) -> Result<Vec<String>> {
        decrypt_deployment_environment(&self.cipher, ciphertext)
    }

    pub async fn get(
        &self,
        actor: DeploymentActor<'_>,
        deployment_id: &str,
    ) -> Result<Option<DeploymentRecord>> {
        Ok(self.deployments.get(actor, deployment_id).await?)
    }

    pub async fn submit_stop(
        &self,
        actor: DeploymentActor<'_>,
        service_id: &str,
    ) -> Result<DeploymentSubmission> {
        let Some(service) = self
            .deployments
            .service_for_deployment(actor, service_id)
            .await?
        else {
            return Ok(DeploymentSubmission::Missing);
        };
        if !actor.is_admin && !service.role.can_manage_services() {
            return Ok(DeploymentSubmission::Forbidden);
        }
        let Some(deployment) = self.deployments.active_for_stop(actor, service_id).await? else {
            let Some(deployment) = self
                .deployments
                .list(actor, service_id, None, Some(1))
                .await?
                .and_then(|deployments| deployments.into_iter().next())
            else {
                return Ok(DeploymentSubmission::Missing);
            };
            return Ok(if deployment.state == DeploymentState::Stopping {
                DeploymentSubmission::Existing(deployment)
            } else {
                DeploymentSubmission::ActiveConflict
            });
        };
        self.submit_cancel(actor, deployment.id.as_str()).await
    }

    async fn publish_deployment_records(&self, deployment_id: &str) -> Result<()> {
        self.publisher
            .publish_events(&self.deployments, deployment_id)
            .await;
        Ok(())
    }

    fn snapshot_variables(&self, service: &AuthorizedDeploymentService) -> Result<String> {
        let mut values = BTreeMap::new();
        for variable in &service.variables {
            let plaintext = self.cipher.decrypt(&variable.ciphertext)?;
            let value = Zeroizing::new(
                String::from_utf8(plaintext.to_vec()).map_err(|_| Error::InvalidCiphertext)?,
            );
            values.insert(variable.key.clone(), value);
        }
        let plaintext =
            Zeroizing::new(serde_json::to_vec(&values).map_err(|_| Error::InvalidCiphertext)?);
        self.cipher.encrypt(plaintext.as_slice())
    }
}

#[derive(Clone)]
pub struct StreamPublisher {
    sender: broadcast::Sender<StreamRecord>,
}

impl StreamPublisher {
    fn new(sender: broadcast::Sender<StreamRecord>) -> Self {
        Self { sender }
    }

    async fn publish_events(&self, deployments: &DeploymentsRepository, deployment_id: &str) {
        if let Ok(events) = deployments.events(deployment_id).await {
            for event in events {
                let _ = self.sender.send(StreamRecord::Event(event));
            }
        }
    }

    fn publish_logs(&self, logs: Vec<ignitify_db::DeploymentLogRecord>) {
        for log in logs {
            let _ = self.sender.send(StreamRecord::Log(log));
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<StreamRecord> {
        self.sender.subscribe()
    }
}

#[derive(Clone)]
pub struct DeploymentLogSink {
    deployments: DeploymentsRepository,
    publisher: StreamPublisher,
    deployment_id: String,
}

impl DeploymentLogSink {
    fn new(
        deployments: DeploymentsRepository,
        publisher: StreamPublisher,
        deployment_id: impl Into<String>,
    ) -> Self {
        Self {
            deployments,
            publisher,
            deployment_id: deployment_id.into(),
        }
    }

    pub async fn system(&self, line: impl Into<String>) -> Result<()> {
        self.append("system", line).await
    }

    pub async fn append(&self, stream: &str, line: impl Into<String>) -> Result<()> {
        let inserted = self
            .deployments
            .append_logs(
                &self.deployment_id,
                &[ignitify_db::NewDeploymentLog {
                    stream: stream.to_owned(),
                    line: line.into(),
                }],
            )
            .await?;
        self.publisher.publish_logs(inserted);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum StreamRecord {
    Event(ignitify_db::DeploymentEventRecord),
    Log(ignitify_db::DeploymentLogRecord),
}

pub enum DeploymentSubmission {
    Accepted(DeploymentRecord),
    Existing(DeploymentRecord),
    Missing,
    Forbidden,
    ActiveConflict,
}

struct ReconciliationContext<'a, R, I, S, V> {
    runtime: &'a R,
    ingress: &'a I,
    source_build: &'a S,
    dns_verifier: &'a V,
}

pub fn spawn_worker<R, I>(
    deployments: DeploymentsRepository,
    domains: DomainsRepository,
    cipher: Arc<AgeCipher>,
    runtime: R,
    ingress: I,
    publisher: StreamPublisher,
    wake: mpsc::Receiver<()>,
) -> (tokio::task::JoinHandle<()>, Arc<AtomicBool>)
where
    R: ImageRuntime,
    I: Ingress,
{
    spawn_worker_with_source(
        deployments,
        domains,
        cipher,
        WorkerDependencies::new(runtime, ingress, NoopSourceBuild),
        publisher,
        wake,
    )
}

pub fn spawn_worker_with_source<R, I, S>(
    deployments: DeploymentsRepository,
    domains: DomainsRepository,
    cipher: Arc<AgeCipher>,
    dependencies: WorkerDependencies<R, I, S>,
    publisher: StreamPublisher,
    wake: mpsc::Receiver<()>,
) -> (tokio::task::JoinHandle<()>, Arc<AtomicBool>)
where
    R: ImageRuntime,
    I: Ingress,
    S: SourceBuild,
{
    spawn_worker_with_source_and_dns(deployments, domains, cipher, dependencies, publisher, wake)
}

pub fn spawn_worker_with_source_and_dns<R, I, S, V>(
    deployments: DeploymentsRepository,
    domains: DomainsRepository,
    cipher: Arc<AgeCipher>,
    dependencies: WorkerDependencies<R, I, S, V>,
    publisher: StreamPublisher,
    mut wake: mpsc::Receiver<()>,
) -> (tokio::task::JoinHandle<()>, Arc<AtomicBool>)
where
    R: ImageRuntime,
    I: Ingress,
    S: SourceBuild,
    V: DnsVerifier,
{
    let ready = Arc::new(AtomicBool::new(false));
    let worker_ready = ready.clone();
    let handle = tokio::spawn(async move {
        struct Liveness(Arc<AtomicBool>);

        impl Drop for Liveness {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }

        let _liveness = Liveness(worker_ready.clone());
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        let mut jobs = JoinSet::new();
        let mut active_jobs = HashSet::new();
        loop {
            let reconciliation = ReconciliationContext {
                runtime: dependencies.runtime.as_ref(),
                ingress: dependencies.ingress.as_ref(),
                source_build: dependencies.source_build.as_ref(),
                dns_verifier: dependencies.dns_verifier.as_ref(),
            };
            match reconcile_runtime_state(
                &deployments,
                &domains,
                &cipher,
                &reconciliation,
                &publisher,
                &active_jobs,
                false,
            )
            .await
            {
                Ok(()) => worker_ready.store(true, Ordering::Release),
                Err(error) => {
                    worker_ready.store(false, Ordering::Release);
                    tracing::error!(error = %error, "deployment worker reconciliation failed");
                }
            }
            if let Err(error) = deployments.prune_retention().await {
                tracing::error!(error = %error, "deployment retention pruning failed");
            }
            while jobs.len() < MAX_CONCURRENT_DEPLOYMENT_JOBS {
                let Some(deployment) = (match deployments.claim_next().await {
                    Ok(deployment) => deployment,
                    Err(error) => {
                        tracing::error!(error = %error, "deployment worker could not claim deployment");
                        break;
                    }
                }) else {
                    break;
                };
                publisher
                    .publish_events(&deployments, deployment.id.as_str())
                    .await;
                let deployment_id = deployment.id.to_string();
                active_jobs.insert(deployment_id.clone());
                let task_deployments = deployments.clone();
                let task_domains = domains.clone();
                let task_cipher = cipher.clone();
                let task_runtime = dependencies.runtime.clone();
                let task_ingress = dependencies.ingress.clone();
                let task_source_build = dependencies.source_build.clone();
                let task_publisher = publisher.clone();
                jobs.spawn(async move {
                    let result = process_claimed_deployment(
                        &task_deployments,
                        &task_domains,
                        &task_cipher,
                        task_runtime.as_ref(),
                        task_ingress.as_ref(),
                        task_source_build.as_ref(),
                        &task_publisher,
                        deployment,
                    )
                    .await;
                    (deployment_id, result)
                });
            }
            tokio::select! {
                _ = interval.tick() => {}
                value = wake.recv() => {
                    if value.is_none() {
                        return;
                    }
                }
                Some(result) = jobs.join_next(), if !jobs.is_empty() => {
                    match result {
                        Ok((deployment_id, Ok(()))) => {
                            active_jobs.remove(&deployment_id);
                        }
                        Ok((deployment_id, Err(error))) => {
                            active_jobs.remove(&deployment_id);
                            tracing::error!(deployment_id = %deployment_id, error = %error, "deployment job failed");
                        }
                        Err(error) => {
                            tracing::error!(error = %error, "deployment job task failed");
                        }
                    }
                }
            }
        }
    });
    (handle, ready)
}

pub async fn reconcile_once<R, I>(
    deployments: &DeploymentsRepository,
    domains: &DomainsRepository,
    cipher: &AgeCipher,
    runtime: &R,
    ingress: &I,
    publisher: &StreamPublisher,
) -> Result<()>
where
    R: ImageRuntime,
    I: Ingress,
{
    reconcile_once_with_source(
        deployments,
        domains,
        cipher,
        runtime,
        ingress,
        &NoopSourceBuild,
        publisher,
    )
    .await
}

pub async fn reconcile_once_with_source<R, I, S>(
    deployments: &DeploymentsRepository,
    domains: &DomainsRepository,
    cipher: &AgeCipher,
    runtime: &R,
    ingress: &I,
    source_build: &S,
    publisher: &StreamPublisher,
) -> Result<()>
where
    R: ImageRuntime,
    I: Ingress,
    S: SourceBuild,
{
    reconcile_once_with_context(
        deployments,
        domains,
        cipher,
        &ReconciliationContext {
            runtime,
            ingress,
            source_build,
            dns_verifier: &NoopDnsVerifier,
        },
        publisher,
    )
    .await
}

async fn reconcile_once_with_context<R, I, S, V>(
    deployments: &DeploymentsRepository,
    domains: &DomainsRepository,
    cipher: &AgeCipher,
    context: &ReconciliationContext<'_, R, I, S, V>,
    publisher: &StreamPublisher,
) -> Result<()>
where
    R: ImageRuntime,
    I: Ingress,
    S: SourceBuild,
    V: DnsVerifier,
{
    reconcile_runtime_state(
        deployments,
        domains,
        cipher,
        context,
        publisher,
        &HashSet::new(),
        true,
    )
    .await?;
    process_next_deployment(
        deployments,
        domains,
        cipher,
        context.runtime,
        context.ingress,
        context.source_build,
        publisher,
    )
    .await
}

async fn reconcile_runtime_state<R, I, S, V>(
    deployments: &DeploymentsRepository,
    domains: &DomainsRepository,
    cipher: &AgeCipher,
    context: &ReconciliationContext<'_, R, I, S, V>,
    publisher: &StreamPublisher,
    active_jobs: &HashSet<String>,
    claim_deployment: bool,
) -> Result<()>
where
    R: ImageRuntime,
    I: Ingress,
    S: SourceBuild,
    V: DnsVerifier,
{
    let runtime = context.runtime;
    let ingress = context.ingress;
    let source_build = context.source_build;
    let dns_verifier = context.dns_verifier;
    ingress.reconcile().await?;
    reconcile_dns_verifications(domains, dns_verifier).await?;
    for deployment in deployments.nonterminal().await? {
        match deployment.state {
            DeploymentState::Queued => {}
            DeploymentState::Preparing | DeploymentState::Running => {
                if let Some(runtime_ref) = deployment.runtime_ref.as_deref() {
                    let runtime_deployment = RuntimeDeployment::from(&deployment);
                    let observation = runtime.inspect(&runtime_deployment, runtime_ref).await?;
                    if observation.owned {
                        persist_logs(
                            deployments,
                            cipher,
                            runtime,
                            &deployment,
                            runtime_ref,
                            publisher,
                        )
                        .await?;
                    }
                    let became_healthy = advance_observed_deployment(
                        deployments,
                        runtime,
                        &deployment,
                        runtime_ref,
                        observation,
                        publisher,
                    )
                    .await?;
                    if became_healthy {
                        cleanup_prior_deployments(deployments, runtime, &deployment, publisher)
                            .await?;
                    }
                } else if deployment.state == DeploymentState::Preparing
                    && !active_jobs.contains(deployment.id.as_str())
                {
                    deployments
                        .reset_preparing_without_runtime(deployment.id.as_str())
                        .await?;
                }
            }
            DeploymentState::Stopping => {
                if let Some(runtime_ref) = deployment.runtime_ref.as_deref() {
                    let stopped = runtime
                        .stop(
                            runtime_ref,
                            deployment.service_id.as_str(),
                            deployment.generation,
                        )
                        .await?;
                    let (next, failure_reason) = if stopped {
                        (DeploymentState::Stopped, None)
                    } else {
                        (DeploymentState::Failed, Some("runtime identity mismatch"))
                    };
                    deployments
                        .transition(
                            deployment.id.as_str(),
                            next,
                            Some(runtime_ref),
                            failure_reason,
                        )
                        .await?;
                    publisher
                        .publish_events(deployments, deployment.id.as_str())
                        .await;
                } else {
                    deployments
                        .transition(deployment.id.as_str(), DeploymentState::Stopped, None, None)
                        .await?;
                    publisher
                        .publish_events(deployments, deployment.id.as_str())
                        .await;
                }
            }
            DeploymentState::Healthy
            | DeploymentState::Failed
            | DeploymentState::Stopped
            | DeploymentState::Superseded => {}
        }
    }
    for deployment in deployments.routable().await? {
        let runtime_deployment = RuntimeDeployment::from(&deployment);
        if deployment.state == DeploymentState::Healthy
            && let Some(runtime_ref) = deployment.runtime_ref.as_deref()
        {
            persist_logs(
                deployments,
                cipher,
                runtime,
                &deployment,
                runtime_ref,
                publisher,
            )
            .await?;
        }
        reconcile_routes(
            domains,
            cipher,
            runtime,
            ingress,
            &deployment,
            &runtime_deployment,
        )
        .await?;
    }
    if !claim_deployment {
        return Ok(());
    }
    let Some(deployment) = deployments.claim_next().await? else {
        return Ok(());
    };
    publisher
        .publish_events(deployments, deployment.id.as_str())
        .await;
    process_claimed_deployment(
        deployments,
        domains,
        cipher,
        runtime,
        ingress,
        source_build,
        publisher,
        deployment,
    )
    .await
}

async fn process_next_deployment<R, I, S>(
    deployments: &DeploymentsRepository,
    domains: &DomainsRepository,
    cipher: &AgeCipher,
    runtime: &R,
    ingress: &I,
    source_build: &S,
    publisher: &StreamPublisher,
) -> Result<()>
where
    R: ImageRuntime,
    I: Ingress,
    S: SourceBuild,
{
    let Some(deployment) = deployments.claim_next().await? else {
        return Ok(());
    };
    publisher
        .publish_events(deployments, deployment.id.as_str())
        .await;
    process_claimed_deployment(
        deployments,
        domains,
        cipher,
        runtime,
        ingress,
        source_build,
        publisher,
        deployment,
    )
    .await
}

#[expect(
    clippy::too_many_arguments,
    reason = "a claimed deployment is processed with explicit adapter boundaries"
)]
async fn process_claimed_deployment<R, I, S>(
    deployments: &DeploymentsRepository,
    domains: &DomainsRepository,
    cipher: &AgeCipher,
    runtime: &R,
    ingress: &I,
    source_build: &S,
    publisher: &StreamPublisher,
    deployment: DeploymentRecord,
) -> Result<()>
where
    R: ImageRuntime,
    I: Ingress,
    S: SourceBuild,
{
    if deployments.cancel_requested(deployment.id.as_str()).await? {
        return Ok(());
    }
    let source_logs = DeploymentLogSink::new(
        deployments.clone(),
        publisher.clone(),
        deployment.id.as_str(),
    );
    source_logs.system("Source build started").await?;
    let runtime_deployment = match source_build.build(&deployment, &source_logs).await {
        Ok(Some(output)) => {
            source_logs.system("Source build completed").await?;
            if deployments.cancel_requested(deployment.id.as_str()).await? {
                return Ok(());
            }
            deployments
                .record_source_resolution(
                    deployment.id.as_str(),
                    &output.source_revision,
                    output.local_image_id.as_deref(),
                    output.runtime_spec.as_ref(),
                )
                .await?;
            let mut runtime_deployment = RuntimeDeployment::from(&deployment);
            runtime_deployment.local_image_id = output.local_image_id;
            if let Some(spec) = output.runtime_spec {
                runtime_deployment.spec = spec;
            }
            runtime_deployment
        }
        Ok(None) => RuntimeDeployment::from(&deployment),
        Err(error) => {
            tracing::warn!(deployment_id = %deployment.id, error = %error, "deployment source build failed");
            if deployments.cancel_requested(deployment.id.as_str()).await? {
                return Ok(());
            }
            let failure_reason = match error {
                Error::SourceBuild(reason) => format!("source build failed: {reason}"),
                Error::Policy(reason) => format!("source build policy rejected: {reason}"),
                error => format!("source build failed: {error}"),
            };
            let _ = source_logs
                .system(format!("Source build failed: {failure_reason}"))
                .await;
            deployments
                .transition(
                    deployment.id.as_str(),
                    DeploymentState::Failed,
                    None,
                    Some(&failure_reason),
                )
                .await?;
            publisher
                .publish_events(deployments, deployment.id.as_str())
                .await;
            return Ok(());
        }
    };
    if deployments.cancel_requested(deployment.id.as_str()).await? {
        return Ok(());
    }
    let predicted_runtime_ref = runtime.runtime_ref(&runtime_deployment);
    deployments
        .record_runtime_ref(deployment.id.as_str(), &predicted_runtime_ref)
        .await?;
    let environment = decrypt_deployment_environment(cipher, &deployment.variables_ciphertext)?;
    let runtime_ref = match runtime.start(&runtime_deployment, environment).await {
        Ok(runtime_ref) => {
            deployments
                .replace_runtime_ref(deployment.id.as_str(), &runtime_ref)
                .await?;
            runtime_ref
        }
        Err(Error::Policy(reason)) => {
            let failure_reason = format!("runtime policy rejected input: {reason}");
            deployments
                .transition(
                    deployment.id.as_str(),
                    DeploymentState::Failed,
                    Some(&predicted_runtime_ref),
                    Some(&failure_reason),
                )
                .await?;
            publisher
                .publish_events(deployments, deployment.id.as_str())
                .await;
            return Ok(());
        }
        Err(error) => {
            tracing::warn!(deployment_id = %deployment.id, error = %error, "deployment runtime start uncertain");
            match runtime
                .inspect(&runtime_deployment, &predicted_runtime_ref)
                .await
            {
                Ok(observation) if observation.owned && observation.running => {
                    predicted_runtime_ref
                }
                Ok(_) => {
                    schedule_runtime_retry(deployments, &deployment, publisher).await?;
                    return Ok(());
                }
                Err(inspect_error) => return Err(inspect_error),
            }
        }
    };
    let observation = match runtime.inspect(&runtime_deployment, &runtime_ref).await {
        Ok(observation) => observation,
        Err(error) => {
            tracing::warn!(deployment_id = %deployment.id, error = %error, "deployment runtime inspection uncertain");
            return Ok(());
        }
    };
    if observation.owned {
        persist_logs(
            deployments,
            cipher,
            runtime,
            &deployment,
            &runtime_ref,
            publisher,
        )
        .await?;
    }
    let became_healthy = advance_observed_deployment(
        deployments,
        runtime,
        &deployment,
        &runtime_ref,
        observation,
        publisher,
    )
    .await?;
    if observation.owned {
        reconcile_routes(
            domains,
            cipher,
            runtime,
            ingress,
            &deployment,
            &runtime_deployment,
        )
        .await?;
    }
    if became_healthy {
        cleanup_prior_deployments(deployments, runtime, &deployment, publisher).await?;
    }
    Ok(())
}

async fn persist_logs<R>(
    deployments: &DeploymentsRepository,
    cipher: &AgeCipher,
    runtime: &R,
    deployment: &DeploymentRecord,
    runtime_ref: &str,
    publisher: &StreamPublisher,
) -> Result<()>
where
    R: ImageRuntime,
{
    let values = deployment_secret_values(cipher, &deployment.variables_ciphertext)?;
    // ponytail: Docker's `since` cursor is second-granularity, so replay its boundary rather than lose same-second lines. Add source timestamps plus a unique cursor when exact-once logs matter.
    let since = deployments
        .latest_log_since(deployment.id.as_str())
        .await?
        .unwrap_or(0);
    let logs = redact_logs(
        runtime
            .logs(runtime_ref, since)
            .await?
            .into_iter()
            .map(ignitify_db::NewDeploymentLog::from)
            .collect(),
        &values,
    );
    let inserted = deployments
        .append_logs(deployment.id.as_str(), &logs)
        .await?;
    publisher.publish_logs(inserted);
    Ok(())
}

async fn reconcile_routes<R, I>(
    domains: &DomainsRepository,
    cipher: &AgeCipher,
    runtime: &R,
    ingress: &I,
    deployment: &DeploymentRecord,
    runtime_deployment: &RuntimeDeployment,
) -> Result<()>
where
    R: ImageRuntime,
    I: Ingress,
{
    let Some(port) = runtime_deployment.spec.internal_port() else {
        return Ok(());
    };
    let domain_records = domains
        .active_for_service(deployment.service_id.as_str())
        .await?;
    if domain_records.is_empty() {
        return Ok(());
    }
    // A remote release attaches directly to the ingress on its destination. Its
    // runtime verifies that destination network during reconciliation, so local
    // ingress availability must not gate a remote service.
    if deployment.deployment_destination_id.is_none() && !ingress.ensure_ready().await? {
        set_domain_statuses(
            domains,
            &domain_records,
            ignitify_domain::DomainStatus::Failed,
            Some("ingress is unavailable"),
        )
        .await?;
        return Ok(());
    }
    let mut routes = Vec::with_capacity(domain_records.len());
    for domain in &domain_records {
        routes.push(ingress.route(&deployment.service_id, &domain.id, &domain.hostname, port)?);
    }
    let environment = decrypt_deployment_environment(cipher, &deployment.variables_ciphertext)?;
    let Some(runtime_ref) = deployment.runtime_ref.as_deref() else {
        return Ok(());
    };
    match runtime
        .reconcile_routes(runtime_deployment, runtime_ref, environment, routes)
        .await
    {
        Ok(true) => {
            set_domain_statuses(
                domains,
                &domain_records,
                ignitify_domain::DomainStatus::Active,
                None,
            )
            .await
        }
        Ok(false) => {
            set_domain_statuses(
                domains,
                &domain_records,
                ignitify_domain::DomainStatus::Failed,
                Some("route reconciliation failed"),
            )
            .await
        }
        Err(error) => {
            set_domain_statuses(
                domains,
                &domain_records,
                ignitify_domain::DomainStatus::Failed,
                Some("route reconciliation failed"),
            )
            .await?;
            Err(error)
        }
    }
}

async fn set_domain_statuses(
    domains: &DomainsRepository,
    domain_records: &[ignitify_db::DomainRecord],
    status: ignitify_domain::DomainStatus,
    last_error: Option<&str>,
) -> Result<()> {
    for domain in domain_records {
        domains
            .set_status(domain.id.as_str(), status, last_error)
            .await?;
    }
    Ok(())
}

async fn cleanup_prior_deployments<R>(
    deployments: &DeploymentsRepository,
    runtime: &R,
    deployment: &DeploymentRecord,
    publisher: &StreamPublisher,
) -> Result<()>
where
    R: ImageRuntime,
{
    let prior_deployments = deployments
        .healthy_prior_deployments(deployment.service_id.as_str(), deployment.id.as_str())
        .await?;
    for prior in &prior_deployments {
        if let Some(runtime_ref) = prior.runtime_ref.as_deref()
            && !runtime
                .stop(
                    runtime_ref,
                    deployment.service_id.as_str(),
                    prior.generation,
                )
                .await?
        {
            return Ok(());
        }
    }
    deployments
        .supersede_prior_healthy(deployment.service_id.as_str(), deployment.id.as_str())
        .await?;
    publisher
        .publish_events(deployments, deployment.id.as_str())
        .await;
    for prior in prior_deployments {
        publisher
            .publish_events(deployments, prior.id.as_str())
            .await;
    }
    Ok(())
}

fn decrypt_deployment_environment(cipher: &AgeCipher, ciphertext: &str) -> Result<Vec<String>> {
    Ok(decrypt_deployment_values(cipher, ciphertext)?
        .into_iter()
        .map(|(key, value)| format!("{key}={}", value.as_str()))
        .collect())
}

fn deployment_secret_values(
    cipher: &AgeCipher,
    ciphertext: &str,
) -> Result<Vec<Zeroizing<String>>> {
    Ok(decrypt_deployment_values(cipher, ciphertext)?
        .into_values()
        .filter(|value| !value.is_empty())
        .collect())
}

fn redact_logs(
    logs: Vec<ignitify_db::NewDeploymentLog>,
    values: &[Zeroizing<String>],
) -> Vec<ignitify_db::NewDeploymentLog> {
    if values.is_empty() {
        return logs;
    }
    logs.into_iter()
        .map(|mut log| {
            log.line = "[REDACTED]".to_owned();
            log
        })
        .collect()
}

fn decrypt_deployment_values(
    cipher: &AgeCipher,
    ciphertext: &str,
) -> Result<BTreeMap<String, Zeroizing<String>>> {
    let plaintext = cipher.decrypt(ciphertext)?;
    serde_json::from_slice(plaintext.as_slice()).map_err(|_| Error::InvalidCiphertext)
}

async fn schedule_runtime_retry(
    deployments: &DeploymentsRepository,
    deployment: &DeploymentRecord,
    publisher: &StreamPublisher,
) -> Result<()> {
    match deployments
        .schedule_retry(
            deployment.id.as_str(),
            "runtime did not start",
            MAX_RUNTIME_START_ATTEMPTS,
        )
        .await?
    {
        RetrySchedule::Scheduled { retry_after } => {
            tracing::warn!(
                deployment_id = %deployment.id,
                attempt_count = deployment.attempt_count,
                retry_after = %retry_after,
                "deployment runtime start retry scheduled"
            );
        }
        RetrySchedule::Exhausted => {
            tracing::warn!(deployment_id = %deployment.id, "deployment runtime retries exhausted");
        }
        RetrySchedule::Cancelled | RetrySchedule::Unchanged => {}
    }
    publisher
        .publish_events(deployments, deployment.id.as_str())
        .await;
    Ok(())
}

async fn advance_observed_deployment<R>(
    deployments: &DeploymentsRepository,
    runtime: &R,
    deployment: &DeploymentRecord,
    runtime_ref: &str,
    observation: RuntimeObservation,
    publisher: &StreamPublisher,
) -> Result<bool>
where
    R: ImageRuntime,
{
    if !observation.owned {
        deployments
            .transition(
                deployment.id.as_str(),
                DeploymentState::Failed,
                Some(runtime_ref),
                Some("runtime identity mismatch"),
            )
            .await?;
        publisher
            .publish_events(deployments, deployment.id.as_str())
            .await;
        return Ok(false);
    }
    if !observation.running {
        if deployment.state == DeploymentState::Preparing {
            if observation.owned {
                runtime
                    .stop(
                        runtime_ref,
                        deployment.service_id.as_str(),
                        deployment.generation,
                    )
                    .await?;
            }
            schedule_runtime_retry(deployments, deployment, publisher).await?;
            return Ok(false);
        }
        if deployments
            .transition(
                deployment.id.as_str(),
                DeploymentState::Failed,
                Some(runtime_ref),
                Some("runtime is not running"),
            )
            .await?
        {
            runtime
                .stop(
                    runtime_ref,
                    deployment.service_id.as_str(),
                    deployment.generation,
                )
                .await?;
        }
        publisher
            .publish_events(deployments, deployment.id.as_str())
            .await;
        return Ok(false);
    }
    if observation.health_failing {
        if deployments
            .transition(
                deployment.id.as_str(),
                DeploymentState::Failed,
                Some(runtime_ref),
                Some("runtime healthcheck failed"),
            )
            .await?
        {
            runtime
                .stop(
                    runtime_ref,
                    deployment.service_id.as_str(),
                    deployment.generation,
                )
                .await?;
        }
        publisher
            .publish_events(deployments, deployment.id.as_str())
            .await;
        return Ok(false);
    }
    if deployment.state == DeploymentState::Preparing {
        deployments
            .transition(
                deployment.id.as_str(),
                DeploymentState::Running,
                Some(runtime_ref),
                None,
            )
            .await?;
    }
    let has_healthcheck = matches!(
        deployment.spec,
        ignitify_domain::ServiceSpec::Image {
            healthcheck: Some(_),
            ..
        }
    );
    if has_healthcheck
        && observation.healthy != Some(true)
        && health_gate_expired(deployment.started_at.as_deref())
    {
        if deployments
            .transition(
                deployment.id.as_str(),
                DeploymentState::Failed,
                Some(runtime_ref),
                Some("runtime healthcheck did not become healthy within 5 minutes"),
            )
            .await?
        {
            runtime
                .stop(
                    runtime_ref,
                    deployment.service_id.as_str(),
                    deployment.generation,
                )
                .await?;
        }
        publisher
            .publish_events(deployments, deployment.id.as_str())
            .await;
        return Ok(false);
    }
    let became_healthy = (!has_healthcheck
        && !matches!(
            deployment.spec,
            ignitify_domain::ServiceSpec::Compose { .. }
        )
        || observation.healthy == Some(true)
        || matches!(
            deployment.spec,
            ignitify_domain::ServiceSpec::Compose { .. }
        ) && observation.healthy.is_none())
        && deployments
            .transition(
                deployment.id.as_str(),
                DeploymentState::Healthy,
                Some(runtime_ref),
                None,
            )
            .await?;
    publisher
        .publish_events(deployments, deployment.id.as_str())
        .await;
    Ok(became_healthy)
}

fn health_gate_expired(started_at: Option<&str>) -> bool {
    started_at
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .is_some_and(|started_at| {
            Utc::now().signed_duration_since(started_at.with_timezone(&Utc))
                >= chrono::Duration::from_std(HEALTH_GATE_TIMEOUT).unwrap_or_default()
        })
}

pub struct AgeCipher {
    identity: x25519::Identity,
    recipient: x25519::Recipient,
}

impl AgeCipher {
    pub fn from_identity(identity: impl AsRef<str>) -> Result<Self> {
        let identity =
            x25519::Identity::from_str(identity.as_ref()).map_err(|_| Error::InvalidIdentity)?;
        let recipient = identity.to_public();
        Ok(Self {
            identity,
            recipient,
        })
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<String> {
        let encryptor =
            Encryptor::with_recipients(std::iter::once(&self.recipient as &dyn age::Recipient))
                .map_err(|_| Error::Encryption)?;
        let mut output = Vec::new();
        {
            let armor =
                age::armor::ArmoredWriter::wrap_output(&mut output, age::armor::Format::AsciiArmor)
                    .map_err(|_| Error::Encryption)?;
            let mut writer = encryptor
                .wrap_output(armor)
                .map_err(|_| Error::Encryption)?;
            writer.write_all(plaintext).map_err(|_| Error::Encryption)?;
            writer
                .finish()
                .map_err(|_| Error::Encryption)?
                .finish()
                .map_err(|_| Error::Encryption)?;
        }
        String::from_utf8(output).map_err(|_| Error::Encryption)
    }

    pub fn decrypt(&self, ciphertext: &str) -> Result<Zeroizing<Vec<u8>>> {
        let decryptor = Decryptor::new(age::armor::ArmoredReader::new(ciphertext.as_bytes()))
            .map_err(|_| Error::InvalidCiphertext)?;
        let mut reader = decryptor
            .decrypt(std::iter::once(&self.identity as &dyn age::Identity))
            .map_err(|_| Error::InvalidCiphertext)?;
        let mut plaintext = Zeroizing::new(Vec::new());
        std::io::Read::read_to_end(&mut reader, &mut plaintext)
            .map_err(|_| Error::InvalidCiphertext)?;
        Ok(plaintext)
    }
}

#[derive(Debug, Clone)]
pub struct ServiceReadModel {
    pub id: String,
    pub project_id: String,
    pub environment_id: String,
    pub role: String,
    pub name: String,
    pub kind: String,
    pub spec: ignitify_domain::ServiceSpec,
    pub source_config: Option<ignitify_domain::ServiceSourceConfig>,
    pub auto_deploy_webhook_secret: Option<Zeroizing<String>>,
    pub deployment_destination_id: Option<String>,
    pub desired_generation: i64,
    pub desired_state: String,
    pub created_at: String,
    pub updated_at: String,
    pub variables: Vec<ServiceVariableReadModel>,
}

#[derive(Debug)]
pub struct AutoDeployWebhookTargetModel {
    pub service_id: String,
    pub provider_id: String,
    pub repository: String,
    pub branch: String,
    pub secret: Zeroizing<String>,
    pub project_owner_id: String,
}

#[derive(Debug)]
pub enum AutoDeploySecretRotation {
    Rotated(Zeroizing<String>),
    Missing,
    Forbidden,
    Disabled,
}

#[derive(Debug, Clone)]
pub struct ProjectEnvironmentVariableInput {
    pub key: String,
    pub value: Option<String>,
    pub is_secret: bool,
}

#[derive(Debug, Clone)]
pub struct ProjectEnvironmentReadModel {
    pub role: String,
    pub variables: Vec<ProjectEnvironmentVariableReadModel>,
}

#[derive(Debug, Clone)]
pub struct ProjectEnvironmentVariableReadModel {
    pub key: String,
    pub is_secret: bool,
    pub is_set: bool,
    pub value: Option<Zeroizing<String>>,
}

#[derive(Debug, Clone)]
pub enum ProjectEnvironmentMutationModel {
    Updated(ProjectEnvironmentReadModel),
    Missing,
    Forbidden,
}

#[derive(Debug, Clone)]
pub struct ServiceVariableReadModel {
    pub key: String,
    pub is_secret: bool,
    pub is_set: bool,
    pub value: Option<Zeroizing<String>>,
}

#[derive(Debug, Clone)]
pub enum ServiceMutationOutcomeModel {
    Created(ServiceReadModel),
    Updated(ServiceReadModel),
    Removed,
    Missing,
    Forbidden,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid age identity")]
    InvalidIdentity,
    #[error("could not encrypt service variable")]
    Encryption,
    #[error("invalid encrypted service variable")]
    InvalidCiphertext,
    #[error("idempotency key must use visible ASCII and be 1 to 128 bytes")]
    InvalidIdempotencyKey,
    #[error("source revision must be a 40 to 64 character lowercase hexadecimal commit id")]
    InvalidSourceRevision,
    #[error("image runtime failed")]
    Runtime,
    #[error("source build failed: {0}")]
    SourceBuild(String),
    #[error("runtime policy rejected input: {0}")]
    Policy(&'static str),
    #[error("worker is unavailable")]
    WorkerUnavailable,
    #[error(transparent)]
    Domain(#[from] ignitify_domain::InputError),
    #[error(transparent)]
    Database(#[from] ignitify_db::DatabaseError),
}

pub type Result<T> = std::result::Result<T, Error>;

fn validate_idempotency_key(value: &str) -> Result<()> {
    if !(1..=128).contains(&value.len()) || value.bytes().any(|byte| !(0x21..=0x7e).contains(&byte))
    {
        return Err(Error::InvalidIdempotencyKey);
    }
    Ok(())
}

fn validate_source_revision(value: &str) -> Result<()> {
    if !(40..=64).contains(&value.len())
        || value.bytes().any(|byte| !byte.is_ascii_hexdigit())
        || value.bytes().all(|byte| byte == b'0')
    {
        return Err(Error::InvalidSourceRevision);
    }
    Ok(())
}

fn generate_auto_deploy_secret() -> Zeroizing<String> {
    Zeroizing::new(format!(
        "{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };

    use age::secrecy::ExposeSecret;
    use ignitify_db::{
        Database, DatabaseConfig, DeploymentActor, NewServiceVariable, ProjectActor, ServiceActor,
        ServiceVariableRecord, UserRole as DatabaseUserRole,
    };
    use ignitify_domain::{
        DnsRecord, DnsRecordType, DnsVerificationStatus, DomainName, DomainStatus, ProjectInput,
        ServiceInput, ServiceVariableInput,
    };

    use super::{
        AgeCipher, ControlHandle, ImageRuntime, Ingress, IngressRoute,
        ProjectEnvironmentVariableInput, ServiceControl, reconcile_once,
    };

    #[test]
    fn deployment_logs_redact_snapshot_values() {
        let logs = super::redact_logs(
            vec![ignitify_db::NewDeploymentLog {
                stream: "stdout".to_owned(),
                line: "TOKEN=plain-secret".to_owned(),
            }],
            &[zeroize::Zeroizing::new("plain-secret".to_owned())],
        );

        assert_eq!(logs[0].line, "[REDACTED]");
    }

    #[test]
    fn deployment_logs_redact_when_any_variable_is_present() {
        let logs = super::redact_logs(
            vec![ignitify_db::NewDeploymentLog {
                stream: "stdout".to_owned(),
                line: "safe output".to_owned(),
            }],
            &[zeroize::Zeroizing::new("plain-secret".to_owned())],
        );

        assert_eq!(logs[0].line, "[REDACTED]");
    }

    #[test]
    fn deployment_logs_redact_multiline_snapshot_values() {
        let logs = super::redact_logs(
            vec![
                ignitify_db::NewDeploymentLog {
                    stream: "stdout".to_owned(),
                    line: "TOKEN=first".to_owned(),
                },
                ignitify_db::NewDeploymentLog {
                    stream: "stdout".to_owned(),
                    line: "second".to_owned(),
                },
            ],
            &[zeroize::Zeroizing::new("first\nsecond".to_owned())],
        );

        assert_eq!(
            logs.into_iter().map(|log| log.line).collect::<Vec<_>>(),
            ["[REDACTED]", "[REDACTED]"]
        );
    }

    #[test]
    fn ciphertext_excludes_plaintext() {
        let identity = age::x25519::Identity::generate();
        let identity = identity.to_string();
        let cipher = AgeCipher::from_identity(identity.expose_secret()).unwrap();
        let ciphertext = cipher.encrypt(b"not-in-ciphertext").unwrap();

        assert!(!ciphertext.contains("not-in-ciphertext"));
        assert_eq!(
            cipher.decrypt(&ciphertext).unwrap().as_slice(),
            b"not-in-ciphertext"
        );
    }

    #[tokio::test]
    async fn service_update_preserves_existing_secret_ciphertext() {
        let database = Database::connect(&DatabaseConfig {
            url: "sqlite::memory:".to_owned(),
        })
        .await
        .unwrap();
        let identity = age::x25519::Identity::generate().to_string();
        let control = ServiceControl::new(
            database.services(),
            database.projects(),
            identity.expose_secret(),
        )
        .unwrap();
        let input = ServiceInput::image(
            "web",
            "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some(80),
            None,
            vec![ServiceVariableInput {
                key: "API_TOKEN".to_owned(),
                value: String::new(),
                is_secret: true,
            }],
        )
        .unwrap();

        let (_, variables) = control
            .encrypt_variables_preserving_secrets(
                input,
                &[ServiceVariableRecord {
                    key: "API_TOKEN".to_owned(),
                    is_secret: true,
                    ciphertext: "existing-ciphertext".to_owned(),
                }],
                &HashSet::from(["API_TOKEN".to_owned()]),
            )
            .unwrap();

        assert_eq!(variables[0].ciphertext, "existing-ciphertext");
    }

    struct FakeIngress;

    impl Ingress for FakeIngress {
        fn route(
            &self,
            _service_id: &ignitify_domain::ServiceId,
            _domain_id: &ignitify_domain::DomainId,
            _hostname: &ignitify_domain::DomainName,
            _port: u32,
        ) -> super::Result<IngressRoute> {
            Ok(IngressRoute {
                labels: std::collections::BTreeMap::new(),
                network: "none".to_owned(),
            })
        }
    }

    struct SyncingIngress(Arc<AtomicBool>);

    impl Ingress for SyncingIngress {
        fn route(
            &self,
            _service_id: &ignitify_domain::ServiceId,
            _domain_id: &ignitify_domain::DomainId,
            _hostname: &ignitify_domain::DomainName,
            _port: u32,
        ) -> super::Result<IngressRoute> {
            Ok(IngressRoute {
                labels: std::collections::BTreeMap::new(),
                network: "none".to_owned(),
            })
        }

        async fn reconcile(&self) -> super::Result<()> {
            self.0.store(true, Ordering::Release);
            Ok(())
        }
    }

    struct ValidDnsVerifier;

    impl super::DnsVerifier for ValidDnsVerifier {
        async fn verify(
            &self,
            _domain: &ignitify_db::DomainRecord,
        ) -> super::DnsVerificationResult {
            super::DnsVerificationResult {
                status: DnsVerificationStatus::Valid,
                error: None,
            }
        }
    }

    struct FakeRuntime {
        calls: Arc<Mutex<Vec<String>>>,
        routed_local_images: Arc<Mutex<Vec<Option<String>>>>,
        logs: Arc<Mutex<Vec<Vec<super::RuntimeLog>>>>,
        routes_fail: bool,
    }

    #[tokio::test]
    async fn worker_syncs_ingress_before_claiming_deployments() {
        let database = Database::connect(&DatabaseConfig {
            url: "sqlite::memory:".to_owned(),
        })
        .await
        .unwrap();
        let identity = age::x25519::Identity::generate().to_string();
        let cipher = AgeCipher::from_identity(identity.expose_secret()).unwrap();
        let synchronized = Arc::new(AtomicBool::new(false));
        let ingress = SyncingIngress(synchronized.clone());
        let runtime = FakeRuntime {
            calls: Arc::new(Mutex::new(vec![])),
            routed_local_images: Arc::new(Mutex::new(vec![])),
            logs: Arc::new(Mutex::new(vec![])),
            routes_fail: false,
        };
        let (publisher, _) = tokio::sync::broadcast::channel(16);

        reconcile_once(
            &database.deployments(),
            &database.domains(),
            &cipher,
            &runtime,
            &ingress,
            &super::StreamPublisher::new(publisher),
        )
        .await
        .unwrap();

        assert!(synchronized.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn worker_completes_requested_dns_verification() {
        let database = Database::connect(&DatabaseConfig {
            url: "sqlite::memory:".to_owned(),
        })
        .await
        .unwrap();
        let actor_id = database
            .users()
            .create("owner", "hash", DatabaseUserRole::User)
            .await
            .unwrap()
            .id;
        let project = database
            .projects()
            .create(&actor_id, ProjectInput::new("Platform").unwrap())
            .await
            .unwrap();
        let service = database
            .services()
            .create(
                ServiceActor {
                    id: &actor_id,
                    is_admin: false,
                },
                project.id.as_str(),
                ServiceInput::image(
                    "web",
                    "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    Some(8080),
                    None,
                    vec![],
                )
                .unwrap()
                .configuration,
                Vec::<NewServiceVariable>::new(),
                None,
            )
            .await
            .unwrap();
        let ignitify_db::ServiceMutationOutcome::Created(service) = service else {
            panic!("service must exist");
        };
        let actor = ignitify_db::DomainActor {
            id: &actor_id,
            is_admin: false,
        };
        let domain = database
            .domains()
            .create(
                actor,
                service.id.as_str(),
                DomainName::new("app.example.com").unwrap(),
                DnsRecord::new(DnsRecordType::A, "203.0.113.10").unwrap(),
            )
            .await
            .unwrap();
        let ignitify_db::DomainMutationOutcome::Created(domain) = domain else {
            panic!("domain must exist");
        };
        database
            .domains()
            .request_dns_verification(actor, domain.id.as_str())
            .await
            .unwrap();

        let identity = age::x25519::Identity::generate().to_string();
        let cipher = AgeCipher::from_identity(identity.expose_secret()).unwrap();
        let runtime = FakeRuntime {
            calls: Arc::new(Mutex::new(vec![])),
            routed_local_images: Arc::new(Mutex::new(vec![])),
            logs: Arc::new(Mutex::new(vec![])),
            routes_fail: false,
        };
        let ingress = SyncingIngress(Arc::new(AtomicBool::new(false)));
        let (publisher, _) = tokio::sync::broadcast::channel(16);

        super::reconcile_once_with_context(
            &database.deployments(),
            &database.domains(),
            &cipher,
            &super::ReconciliationContext {
                runtime: &runtime,
                ingress: &ingress,
                source_build: &super::NoopSourceBuild,
                dns_verifier: &ValidDnsVerifier,
            },
            &super::StreamPublisher::new(publisher),
        )
        .await
        .unwrap();

        let domains = database
            .domains()
            .list(actor, service.id.as_str())
            .await
            .unwrap();
        let domain = domains.unwrap().into_iter().next().unwrap();
        assert_eq!(domain.dns_status, DnsVerificationStatus::Valid);
        assert!(domain.dns_checked_at.is_some());
    }

    impl ImageRuntime for FakeRuntime {
        fn runtime_ref(&self, deployment: &super::RuntimeDeployment) -> String {
            format!("runtime-{}", deployment.id)
        }

        async fn start(
            &self,
            deployment: &super::RuntimeDeployment,
            _environment: Vec<String>,
        ) -> super::Result<String> {
            self.calls.lock().unwrap().push(deployment.id.to_string());
            Ok(format!("runtime-{}", deployment.id))
        }

        async fn inspect(
            &self,
            _deployment: &super::RuntimeDeployment,
            _runtime_ref: &str,
        ) -> super::Result<super::RuntimeObservation> {
            Ok(super::RuntimeObservation {
                owned: true,
                running: true,
                healthy: Some(false),
                health_failing: false,
            })
        }

        async fn stop(
            &self,
            _runtime_ref: &str,
            _service_id: &str,
            _generation: i64,
        ) -> super::Result<bool> {
            Ok(true)
        }

        async fn logs(
            &self,
            _runtime_ref: &str,
            _since: i64,
        ) -> super::Result<Vec<super::RuntimeLog>> {
            let mut logs = self.logs.lock().unwrap();
            Ok(if logs.is_empty() {
                vec![]
            } else {
                logs.remove(0)
            })
        }

        async fn reconcile_routes(
            &self,
            deployment: &super::RuntimeDeployment,
            _runtime_ref: &str,
            _environment: Vec<String>,
            _routes: Vec<IngressRoute>,
        ) -> super::Result<bool> {
            self.routed_local_images
                .lock()
                .unwrap()
                .push(deployment.local_image_id.clone());
            if self.routes_fail {
                return Err(super::Error::Runtime);
            }
            Ok(true)
        }
    }

    #[tokio::test]
    async fn worker_restart_scan_recovers_preparing_deployment_without_restarting_runtime() {
        let database = Database::connect(&DatabaseConfig {
            url: "sqlite::memory:".to_owned(),
        })
        .await
        .unwrap();
        let actor_id = database
            .users()
            .create("owner", "hash", DatabaseUserRole::User)
            .await
            .unwrap()
            .id;
        let project = database
            .projects()
            .create(&actor_id, ProjectInput::new("Platform").unwrap())
            .await
            .unwrap();
        let input = ServiceInput::image(
            "web",
            "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some(8080),
            None,
            vec![],
        )
        .unwrap();
        let service = database
            .services()
            .create(
                ServiceActor {
                    id: &actor_id,
                    is_admin: false,
                },
                project.id.as_str(),
                input.configuration,
                Vec::<NewServiceVariable>::new(),
                None,
            )
            .await
            .unwrap();
        let ignitify_db::ServiceMutationOutcome::Created(service) = service else {
            panic!("service must exist");
        };
        database
            .domains()
            .create(
                ignitify_db::DomainActor {
                    id: &actor_id,
                    is_admin: false,
                },
                service.id.as_str(),
                DomainName::new("app.example.com").unwrap(),
                ignitify_domain::DnsRecord::new(ignitify_domain::DnsRecordType::A, "203.0.113.10")
                    .unwrap(),
            )
            .await
            .unwrap();
        let identity = age::x25519::Identity::generate().to_string();
        let cipher = AgeCipher::from_identity(identity.expose_secret()).unwrap();
        let variables_ciphertext = cipher.encrypt(b"{}").unwrap();
        let deployment = database
            .deployments()
            .create(
                DeploymentActor {
                    id: &actor_id,
                    is_admin: false,
                },
                service.id.as_str(),
                ignitify_db::NewDeployment {
                    idempotency_key: "deploy-1".to_owned(),
                    requested_by_user_id: actor_id.clone(),
                    spec: service.spec,
                    source_config: None,
                    deployment_destination_id: None,
                    source_revision: None,
                    variables_ciphertext,
                },
            )
            .await
            .unwrap();
        let ignitify_db::CreateDeploymentOutcome::Created(deployment) = deployment else {
            panic!("deployment must be created");
        };
        let claimed = database.deployments().claim_next().await.unwrap().unwrap();
        database
            .deployments()
            .record_source_resolution(
                claimed.id.as_str(),
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                Some("sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
                None,
            )
            .await
            .unwrap();
        database
            .deployments()
            .record_runtime_ref(claimed.id.as_str(), &format!("runtime-{}", claimed.id))
            .await
            .unwrap();
        let runtime = FakeRuntime {
            calls: Arc::new(Mutex::new(vec![])),
            routed_local_images: Arc::new(Mutex::new(vec![])),
            logs: Arc::new(Mutex::new(vec![
                vec![super::RuntimeLog {
                    stream: "stdout".to_owned(),
                    line: "starting application".to_owned(),
                }],
                vec![super::RuntimeLog {
                    stream: "stdout".to_owned(),
                    line: "application is ready".to_owned(),
                }],
            ])),
            routes_fail: false,
        };
        let (publisher, _) = tokio::sync::broadcast::channel(16);
        reconcile_once(
            &database.deployments(),
            &database.domains(),
            &cipher,
            &runtime,
            &FakeIngress,
            &super::StreamPublisher::new(publisher),
        )
        .await
        .unwrap();

        let events = database
            .deployments()
            .events(deployment.id.as_str())
            .await
            .unwrap();
        assert_eq!(
            events
                .into_iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            [
                "deployment.queued",
                "deployment.preparing",
                "deployment.running",
                "deployment.healthy"
            ]
        );
        assert!(runtime.calls.lock().unwrap().is_empty());
        let logs = database
            .deployments()
            .logs_after(deployment.id.as_str(), 0, 10)
            .await
            .unwrap();
        assert_eq!(
            logs.into_iter().map(|log| log.line).collect::<Vec<_>>(),
            ["starting application", "application is ready"]
        );
        assert_eq!(
            runtime.routed_local_images.lock().unwrap().as_slice(),
            [Some(
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_owned()
            )]
        );
    }

    #[tokio::test]
    async fn reconcile_marks_domains_failed_when_route_application_fails() {
        let database = Database::connect(&DatabaseConfig {
            url: "sqlite::memory:".to_owned(),
        })
        .await
        .unwrap();
        let actor_id = database
            .users()
            .create("owner", "hash", DatabaseUserRole::User)
            .await
            .unwrap()
            .id;
        let project = database
            .projects()
            .create(&actor_id, ProjectInput::new("Platform").unwrap())
            .await
            .unwrap();
        let service = database
            .services()
            .create(
                ServiceActor {
                    id: &actor_id,
                    is_admin: false,
                },
                project.id.as_str(),
                ServiceInput::image(
                    "web",
                    "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    Some(8080),
                    None,
                    vec![],
                )
                .unwrap()
                .configuration,
                Vec::<NewServiceVariable>::new(),
                None,
            )
            .await
            .unwrap();
        let ignitify_db::ServiceMutationOutcome::Created(service) = service else {
            panic!("service must exist");
        };
        database
            .domains()
            .create(
                ignitify_db::DomainActor {
                    id: &actor_id,
                    is_admin: false,
                },
                service.id.as_str(),
                DomainName::new("app.example.com").unwrap(),
                ignitify_domain::DnsRecord::new(ignitify_domain::DnsRecordType::A, "203.0.113.10")
                    .unwrap(),
            )
            .await
            .unwrap();
        let identity = age::x25519::Identity::generate().to_string();
        let cipher = AgeCipher::from_identity(identity.expose_secret()).unwrap();
        let deployment = database
            .deployments()
            .create(
                DeploymentActor {
                    id: &actor_id,
                    is_admin: false,
                },
                service.id.as_str(),
                ignitify_db::NewDeployment {
                    idempotency_key: "deploy-1".to_owned(),
                    requested_by_user_id: actor_id.clone(),
                    spec: service.spec,
                    source_config: None,
                    deployment_destination_id: None,
                    source_revision: None,
                    variables_ciphertext: cipher.encrypt(b"{}").unwrap(),
                },
            )
            .await
            .unwrap();
        let ignitify_db::CreateDeploymentOutcome::Created(_) = deployment else {
            panic!("deployment must be created");
        };
        let claimed = database.deployments().claim_next().await.unwrap().unwrap();
        database
            .deployments()
            .record_runtime_ref(claimed.id.as_str(), &format!("runtime-{}", claimed.id))
            .await
            .unwrap();
        let runtime = FakeRuntime {
            calls: Arc::new(Mutex::new(vec![])),
            routed_local_images: Arc::new(Mutex::new(vec![])),
            logs: Arc::new(Mutex::new(vec![])),
            routes_fail: true,
        };
        let (publisher, _) = tokio::sync::broadcast::channel(16);

        assert!(
            reconcile_once(
                &database.deployments(),
                &database.domains(),
                &cipher,
                &runtime,
                &FakeIngress,
                &super::StreamPublisher::new(publisher),
            )
            .await
            .is_err()
        );

        let domains = database
            .domains()
            .list(
                ignitify_db::DomainActor {
                    id: &actor_id,
                    is_admin: false,
                },
                service.id.as_str(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(domains[0].status, DomainStatus::Failed);
        assert_eq!(
            domains[0].last_error.as_deref(),
            Some("route reconciliation failed")
        );
    }

    #[tokio::test]
    async fn deployment_snapshot_merges_project_variables_before_service_overrides() {
        let database = Database::connect(&DatabaseConfig {
            url: "sqlite::memory:".to_owned(),
        })
        .await
        .unwrap();
        let actor_id = database
            .users()
            .create("owner", "hash", DatabaseUserRole::User)
            .await
            .unwrap()
            .id;
        let project = database
            .projects()
            .create(&actor_id, ProjectInput::new("Platform").unwrap())
            .await
            .unwrap();
        let identity = age::x25519::Identity::generate().to_string();
        let service_control = ServiceControl::new(
            database.services(),
            database.projects(),
            identity.expose_secret(),
        )
        .unwrap();
        let service = service_control
            .create(
                ServiceActor {
                    id: &actor_id,
                    is_admin: false,
                },
                project.id.as_str(),
                ServiceInput::image(
                    "web",
                    "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    None,
                    None,
                    vec![ServiceVariableInput {
                        key: "SHARED".to_owned(),
                        value: "service".to_owned(),
                        is_secret: false,
                    }],
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let super::ServiceMutationOutcomeModel::Created(service) = service else {
            panic!("service must exist");
        };
        service_control
            .update_project_environment(
                ProjectActor {
                    id: &actor_id,
                    is_admin: false,
                },
                project.id.as_str(),
                vec![
                    ProjectEnvironmentVariableInput {
                        key: "SHARED".to_owned(),
                        value: Some("project".to_owned()),
                        is_secret: false,
                    },
                    ProjectEnvironmentVariableInput {
                        key: "PROJECT_ONLY".to_owned(),
                        value: Some("available".to_owned()),
                        is_secret: false,
                    },
                ],
            )
            .await
            .unwrap();
        let (control, _wake) =
            ControlHandle::new(database.deployments(), identity.expose_secret()).unwrap();
        let submission = control
            .submit_deploy(
                DeploymentActor {
                    id: &actor_id,
                    is_admin: false,
                },
                &service.id,
                "deploy-1",
            )
            .await
            .unwrap();
        let deployment = match submission {
            super::DeploymentSubmission::Accepted(deployment) => deployment,
            _ => panic!("deployment must be accepted"),
        };
        let cipher = AgeCipher::from_identity(identity.expose_secret()).unwrap();
        let values =
            super::decrypt_deployment_values(&cipher, &deployment.variables_ciphertext).unwrap();
        assert_eq!(values["SHARED"].as_str(), "service");
        assert_eq!(values["PROJECT_ONLY"].as_str(), "available");
    }
}
