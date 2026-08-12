use tokio::sync::broadcast;

use ignitify_db::DeploymentsRepository;

use crate::Result;

#[derive(Clone)]
pub struct StreamPublisher {
    sender: broadcast::Sender<StreamRecord>,
}

impl StreamPublisher {
    pub(crate) fn new(sender: broadcast::Sender<StreamRecord>) -> Self {
        Self { sender }
    }

    pub(crate) async fn publish_events(
        &self,
        deployments: &DeploymentsRepository,
        deployment_id: &str,
    ) {
        if let Ok(events) = deployments.events(deployment_id).await {
            for event in events {
                let _ = self.sender.send(StreamRecord::Event(event));
            }
        }
    }

    pub(crate) fn publish_logs(&self, logs: Vec<ignitify_db::DeploymentLogRecord>) {
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
    pub(crate) fn new(
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
