use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::Instant,
};

use ignitify_control_plane::{SystemMetricsProvider, SystemMetricsSnapshot};
use ignitify_runtime_docker::DockerRuntime;
use sysinfo::{Disks, Networks, System};

struct CollectorState {
    system: System,
    disks: Disks,
    networks: Networks,
    last_refresh: Instant,
    has_previous_sample: bool,
}

struct HostSnapshot {
    metrics: SystemMetricsSnapshot,
    disks: Vec<DiskCapacity>,
}

struct DiskCapacity {
    mount_point: PathBuf,
    used_bytes: u64,
    total_bytes: u64,
}

pub(crate) struct SystemMetricsCollector {
    state: Mutex<CollectorState>,
    docker: Option<DockerRuntime>,
}

impl SystemMetricsCollector {
    pub(crate) fn new(docker: Option<DockerRuntime>) -> Self {
        let mut system = System::new_all();
        system.refresh_cpu_usage();
        system.refresh_memory();
        Self {
            state: Mutex::new(CollectorState {
                system,
                disks: Disks::new_with_refreshed_list(),
                networks: Networks::new_with_refreshed_list(),
                last_refresh: Instant::now(),
                has_previous_sample: false,
            }),
            docker,
        }
    }
}

impl SystemMetricsProvider for SystemMetricsCollector {
    fn metrics(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Option<SystemMetricsSnapshot>> + Send + '_>,
    > {
        Box::pin(async move {
            let host = {
                let Ok(mut state) = self.state.lock() else {
                    return None;
                };
                collect_host_snapshot(&mut state)
            };

            let Some(docker) = self.docker.as_ref() else {
                return Some(host.metrics);
            };
            let Ok(docker_usage) = docker.disk_usage().await else {
                return Some(host.metrics);
            };

            let docker_disk_total_bytes = docker_usage
                .root_dir
                .as_deref()
                .and_then(|root_dir| disk_capacity_for_path(&host.disks, root_dir));
            Some(SystemMetricsSnapshot {
                docker_disk_used_bytes: Some(docker_usage.used_bytes),
                docker_disk_total_bytes,
                ..host.metrics
            })
        })
    }
}

fn collect_host_snapshot(state: &mut CollectorState) -> HostSnapshot {
    let now = Instant::now();
    let elapsed_seconds = now
        .duration_since(state.last_refresh)
        .as_secs_f64()
        .max(f64::EPSILON);
    state.system.refresh_cpu_usage();
    state.system.refresh_memory();
    state.disks.refresh(false);
    state.networks.refresh(true);

    let disks = state
        .disks
        .list()
        .iter()
        .map(|disk| DiskCapacity {
            mount_point: disk.mount_point().to_path_buf(),
            used_bytes: disk.total_space().saturating_sub(disk.available_space()),
            total_bytes: disk.total_space(),
        })
        .collect::<Vec<_>>();
    let primary_disk = primary_disk(&disks);
    let (block_read_bytes_per_second, block_write_bytes_per_second) = if state.has_previous_sample {
        state
            .disks
            .list()
            .iter()
            .map(|disk| {
                let usage = disk.usage();
                (
                    usage.read_bytes as f64 / elapsed_seconds,
                    usage.written_bytes as f64 / elapsed_seconds,
                )
            })
            .fold((0.0, 0.0), |(read, written), (next_read, next_written)| {
                (read + next_read, written + next_written)
            })
    } else {
        (0.0, 0.0)
    };
    let (network_receive_bytes_per_second, network_transmit_bytes_per_second) =
        if state.has_previous_sample {
            state
                .networks
                .list()
                .values()
                .map(|network| {
                    (
                        network.received() as f64 / elapsed_seconds,
                        network.transmitted() as f64 / elapsed_seconds,
                    )
                })
                .fold(
                    (0.0, 0.0),
                    |(received, transmitted), (next_received, next_transmitted)| {
                        (received + next_received, transmitted + next_transmitted)
                    },
                )
        } else {
            (0.0, 0.0)
        };

    state.last_refresh = now;
    state.has_previous_sample = true;

    HostSnapshot {
        metrics: SystemMetricsSnapshot {
            cpu_usage_percentage: f64::from(state.system.global_cpu_usage()),
            cpu_cores: u32::try_from(state.system.cpus().len()).unwrap_or(u32::MAX),
            memory_used_bytes: state.system.used_memory(),
            memory_total_bytes: state.system.total_memory(),
            disk_used_bytes: primary_disk.map_or(0, |disk| disk.used_bytes),
            disk_total_bytes: primary_disk.map_or(0, |disk| disk.total_bytes),
            docker_disk_used_bytes: None,
            docker_disk_total_bytes: None,
            block_read_bytes_per_second,
            block_write_bytes_per_second,
            network_receive_bytes_per_second,
            network_transmit_bytes_per_second,
        },
        disks,
    }
}

fn primary_disk(disks: &[DiskCapacity]) -> Option<&DiskCapacity> {
    disks
        .iter()
        .find(|disk| disk.mount_point == Path::new("/"))
        .or_else(|| disks.iter().max_by_key(|disk| disk.total_bytes))
}

fn disk_capacity_for_path(disks: &[DiskCapacity], path: &Path) -> Option<u64> {
    disks
        .iter()
        .filter(|disk| path.starts_with(&disk.mount_point))
        .max_by_key(|disk| disk.mount_point.components().count())
        .map(|disk| disk.total_bytes)
}

#[cfg(test)]
mod tests {
    use super::{DiskCapacity, disk_capacity_for_path};
    use std::path::{Path, PathBuf};

    #[test]
    fn docker_root_uses_the_most_specific_mount_capacity() {
        let disks = vec![
            DiskCapacity {
                mount_point: PathBuf::from("/"),
                used_bytes: 1,
                total_bytes: 100,
            },
            DiskCapacity {
                mount_point: PathBuf::from("/var/lib"),
                used_bytes: 2,
                total_bytes: 200,
            },
        ];

        assert_eq!(
            disk_capacity_for_path(&disks, Path::new("/var/lib/docker")),
            Some(200)
        );
    }
}
