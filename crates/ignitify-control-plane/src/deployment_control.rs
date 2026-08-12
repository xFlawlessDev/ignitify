use std::{collections::BTreeMap, sync::Arc};

use ignitify_db::{
    AuthorizedDeploymentService, CancelDeploymentOutcome, CreateDeploymentOutcome, DeploymentActor,
    DeploymentRecord, DeploymentsRepository, NewDeployment,
};
use ignitify_domain::DeploymentState;
use tokio::sync::{broadcast, mpsc};
use zeroize::Zeroizing;

use crate::deployment_values::decrypt_deployment_environment;
use crate::model::{AgeCipher, Error, Result, validate_idempotency_key, validate_source_revision};
use crate::{StreamPublisher, StreamRecord};

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
        self.publisher.subscribe()
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

pub enum DeploymentSubmission {
    Accepted(DeploymentRecord),
    Existing(DeploymentRecord),
    Missing,
    Forbidden,
    ActiveConflict,
}
