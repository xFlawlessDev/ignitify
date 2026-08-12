use std::{collections::HashSet, sync::Arc};

use ignitify_db::{
    AuthorizedProjectVariables, AuthorizedService, NewProjectVariable, NewServiceVariable,
    ProjectActor, ProjectVariablesMutationOutcome, ProjectsRepository, ServiceActor,
    ServiceMutationOutcome, ServiceVariableRecord, ServicesRepository,
};
use ignitify_domain::{ServiceInput, ServiceVariableInput, validate_variable_inputs};
use zeroize::Zeroizing;

use crate::model::{
    AgeCipher, AutoDeploySecretRotation, AutoDeployWebhookTargetModel, Error,
    ProjectEnvironmentMutationModel, ProjectEnvironmentReadModel, ProjectEnvironmentVariableInput,
    ProjectEnvironmentVariableReadModel, Result, ServiceMutationOutcomeModel, ServiceReadModel,
    ServiceVariableReadModel, generate_auto_deploy_secret,
};

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

    pub(crate) fn encrypt_variables_preserving_secrets(
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
