// Docker container stats via bollard

mod stats;

pub use stats::process_statistics;

use crate::models::ContainerStats;
use bollard::Docker;
use bollard::query_parameters::{ListContainersOptions, StatsOptions};
use futures_util::StreamExt;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::instrument;

pub struct DockerRepo {
    docker: Docker,
    live_stats: Arc<RwLock<HashMap<String, ContainerStats>>>,
    active_streams: Arc<RwLock<HashMap<String, tokio::task::JoinHandle<()>>>>,
}

impl DockerRepo {
    pub fn connect() -> anyhow::Result<Self> {
        let docker = Docker::connect_with_unix_defaults()?;
        Ok(Self {
            docker,
            live_stats: Arc::new(RwLock::new(HashMap::new())),
            active_streams: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn list_running_and_refresh_stats(&self) -> Vec<ContainerStats> {
        let mut filters = HashMap::new();
        filters.insert("status".to_string(), vec!["running".to_string()]);

        let filter = ListContainersOptions {
            all: false,
            filters: Some(filters),
            ..Default::default()
        };

        let containers = match self.docker.list_containers(Some(filter)).await {
            Ok(c) => {
                tracing::debug!(
                    operation = "list_containers",
                    containers_count = c.len(),
                    "Listed running containers"
                );
                c
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    operation = "list_containers",
                    "Docker list_containers failed"
                );
                return self.get_cached_stats().await;
            }
        };

        let mut running_ids = Vec::with_capacity(containers.len());
        let mut id_to_name = HashMap::with_capacity(containers.len());
        for c in &containers {
            let id = c.id.as_ref().cloned().unwrap_or_default();
            let name = c
                .names
                .as_ref()
                .and_then(|n| n.first())
                .cloned()
                .unwrap_or_else(|| id.clone());
            let name = name.trim_start_matches('/').to_string();
            running_ids.push(id.clone());
            id_to_name.insert(id.clone(), name);
        }
        let running_set: HashSet<String> = running_ids.iter().cloned().collect();

        let current_keys: Vec<String> = {
            let r = self.active_streams.read().await;
            r.keys().cloned().collect()
        };

        let to_add: Vec<(String, String)> = running_ids
            .into_iter()
            .filter(|id| !current_keys.contains(id))
            .map(|id| {
                let name = id_to_name.get(&id).cloned().unwrap_or_else(|| id.clone());
                (id, name)
            })
            .collect();
        let to_remove: Vec<String> = current_keys
            .into_iter()
            .filter(|id| !running_set.contains(id))
            .collect();

        if !to_add.is_empty() {
            tracing::info!(
                operation = "start_monitoring",
                containers_count = to_add.len(),
                container_names = ?to_add.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>(),
                "Starting monitoring for new containers"
            );
        }
        if !to_remove.is_empty() {
            tracing::info!(
                operation = "stop_monitoring",
                containers_count = to_remove.len(),
                "Stopping monitoring for removed containers"
            );
        }

        let new_handles: Vec<(String, tokio::task::JoinHandle<()>)> = {
            let mut out = Vec::with_capacity(to_add.len());
            for (id, name) in to_add {
                let handle = self.start_monitoring(id.clone(), name);
                out.push((id, handle));
            }
            out
        };

        {
            let mut streams = self.active_streams.write().await;
            for (id, handle) in new_handles {
                streams.insert(id, handle);
            }
            for id in &to_remove {
                if let Some(handle) = streams.remove(id) {
                    handle.abort();
                }
            }
        }
        // Keep the cache aligned with this list. A stats task that already exited
        // has removed itself from active_streams, so it is not in `to_remove`,
        // and its last sample would otherwise be stored on every later snapshot.
        // Retaining under the write lock also drops a sample inserted after abort.
        let stats = {
            let mut live = self.live_stats.write().await;
            stats_for_running_ids(&mut live, &running_set)
        };
        tracing::debug!(
            operation = "get_cached_stats",
            containers_count = stats.len(),
            "Retrieved cached container stats"
        );
        stats
    }

    #[instrument(skip(self), fields(container_id = %id, container_name = %name))]
    fn start_monitoring(&self, id: String, name: String) -> tokio::task::JoinHandle<()> {
        let docker = self.docker.clone();
        let live_stats = self.live_stats.clone();
        let active_streams = self.active_streams.clone();

        tracing::info!(
            operation = "start_monitoring",
            "Starting Docker stats stream for container"
        );

        tokio::spawn(async move {
            let span = tracing::span!(
                tracing::Level::DEBUG,
                "docker_stats_stream",
                container_id = %id,
                container_name = %name
            );
            let _guard = span.enter();

            let options = StatsOptions {
                stream: true,
                ..Default::default()
            };
            let mut stream = docker.stats(&id, Some(options));

            let mut stats_count = 0u64;
            while let Some(result) = stream.next().await {
                match result {
                    Ok(s) => {
                        if let Some(stats) = stats::process_statistics(&s, &id, &name) {
                            stats_count += 1;
                            // Log key metrics periodically (every 10th stat update) at debug level
                            if stats_count.is_multiple_of(10) {
                                let memory_percent = if stats.memory_limit_bytes > 0 {
                                    (stats.memory_usage_bytes as f64
                                        / stats.memory_limit_bytes as f64)
                                        * 100.0
                                } else {
                                    0.0
                                };
                                tracing::debug!(
                                    container_id = %id,
                                    container_name = %name,
                                    cpu_percent = stats.cpu_percent,
                                    memory_usage_mb = stats.memory_usage_bytes / 1024 / 1024,
                                    memory_limit_mb = stats.memory_limit_bytes / 1024 / 1024,
                                    memory_percent = memory_percent,
                                    network_rx_mb = stats.network_rx_bytes / 1024 / 1024,
                                    network_tx_mb = stats.network_tx_bytes / 1024 / 1024,
                                    block_read_mb = stats.block_read_bytes / 1024 / 1024,
                                    block_write_mb = stats.block_write_bytes / 1024 / 1024,
                                    pids = stats.pids,
                                    cpu_throttled = stats.cpu_throttled,
                                    "Container stats update"
                                );
                            }
                            live_stats.write().await.insert(id.clone(), stats);
                        } else {
                            tracing::debug!(
                                container_id = %id,
                                container_name = %name,
                                "Failed to process container stats (missing data)"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            container_id = %id,
                            container_name = %name,
                            operation = "docker_stats_stream",
                            "Stats stream error for container"
                        );
                        break;
                    }
                }
            }
            tracing::info!(
                container_id = %id,
                container_name = %name,
                operation = "docker_stats_stream",
                stats_updates_count = stats_count,
                "Stats stream ended for container"
            );
            // Stream EOF/error means this container is gone. If we only drop the
            // task handle, list_running no longer knows to delete the last sample.
            live_stats.write().await.remove(&id);
            active_streams.write().await.remove(&id);
        })
    }

    async fn get_cached_stats(&self) -> Vec<ContainerStats> {
        let live = self.live_stats.read().await;
        let stats: Vec<ContainerStats> = live.values().cloned().collect();
        tracing::debug!(
            operation = "get_cached_stats",
            containers_count = stats.len(),
            "Retrieved cached container stats"
        );
        stats
    }
}

/// Drop cached samples for containers that are not in the latest running set.
fn stats_for_running_ids(
    live: &mut HashMap<String, ContainerStats>,
    running: &HashSet<String>,
) -> Vec<ContainerStats> {
    live.retain(|id, _| running.contains(id));
    live.values().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ContainerState;

    fn sample(id: &str, memory: u64) -> ContainerStats {
        ContainerStats {
            id: id.to_string(),
            name: id.to_string(),
            cpu_percent: 1.0,
            memory_usage_bytes: memory,
            memory_limit_bytes: 100,
            state: ContainerState::Running,
            network_rx_bytes: 0,
            network_tx_bytes: 0,
            network_rx_packets: 0,
            network_tx_packets: 0,
            network_rx_errors: 0,
            network_tx_errors: 0,
            network_rx_dropped: 0,
            network_tx_dropped: 0,
            block_read_bytes: 0,
            block_write_bytes: 0,
            block_read_ops: 0,
            block_write_ops: 0,
            pids: 1,
            pids_limit: 0,
            cpu_throttled: false,
            cpu_throttled_periods: 0,
            cpu_throttled_time_ns: 0,
            cpu_kernel_percent: 0.0,
            cpu_user_percent: 0.0,
            online_cpus: 1,
            memory_max_usage_bytes: 0,
        }
    }

    #[test]
    fn stats_for_running_ids_drops_containers_whose_stream_already_ended() {
        let mut live = HashMap::from([
            ("alive".to_string(), sample("alive", 10)),
            ("replaced".to_string(), sample("replaced", 99)),
        ]);
        let running = HashSet::from(["alive".to_string()]);
        let reported = stats_for_running_ids(&mut live, &running);
        assert_eq!(reported.len(), 1);
        assert_eq!(reported[0].id, "alive");
        assert_eq!(reported[0].memory_usage_bytes, 10);
        assert!(!live.contains_key("replaced"));
    }
}
