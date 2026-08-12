use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePort {
    pub container_port: u16,
    pub host_ip: Option<String>,
    pub host_port: Option<u16>,
    pub protocol: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub health: Option<String>,
    pub ports: Vec<RuntimePort>,
    pub restart_count: i64,
    pub cpu_percentage: Option<f64>,
    pub memory_usage_bytes: Option<i64>,
    pub cpu_limit_nano_cpus: Option<i64>,
    pub memory_limit_bytes: Option<i64>,
    pub managed: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct HostRuntimeMetrics {
    pub containers: i64,
    pub containers_running: i64,
    pub images: i64,
    pub cpus: i64,
    pub memory_bytes: i64,
}

pub trait RuntimeHealth: Send + Sync {
    fn ready(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;

    fn host_metrics(
        &self,
    ) -> Pin<Box<dyn Future<Output = Option<HostRuntimeMetrics>> + Send + '_>> {
        Box::pin(std::future::ready(None))
    }

    fn container_inventory(
        &self,
    ) -> Pin<Box<dyn Future<Output = Option<Vec<RuntimeContainer>>> + Send + '_>> {
        Box::pin(std::future::ready(None))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemMetricsSnapshot {
    pub cpu_usage_percentage: f64,
    pub cpu_cores: u32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_total_bytes: u64,
    pub docker_disk_used_bytes: Option<u64>,
    pub docker_disk_total_bytes: Option<u64>,
    pub block_read_bytes_per_second: f64,
    pub block_write_bytes_per_second: f64,
    pub network_receive_bytes_per_second: f64,
    pub network_transmit_bytes_per_second: f64,
}

pub trait SystemMetricsProvider: Send + Sync {
    fn metrics(&self) -> Pin<Box<dyn Future<Output = Option<SystemMetricsSnapshot>> + Send + '_>>;
}

pub struct StaticSystemMetrics(pub Option<SystemMetricsSnapshot>);

impl SystemMetricsProvider for StaticSystemMetrics {
    fn metrics(&self) -> Pin<Box<dyn Future<Output = Option<SystemMetricsSnapshot>> + Send + '_>> {
        Box::pin(std::future::ready(self.0))
    }
}

pub struct StaticRuntimeHealth(pub bool);

impl RuntimeHealth for StaticRuntimeHealth {
    fn ready(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(std::future::ready(self.0))
    }
}

pub struct WorkerHealth(pub Arc<AtomicBool>);

impl RuntimeHealth for WorkerHealth {
    fn ready(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(std::future::ready(self.0.load(Ordering::Acquire)))
    }
}
