// Docker container stats via bollard

mod stats;

use crate::models::ContainerStats;
use bollard::Docker;
use bollard::query_parameters::{ListContainersOptions, StatsOptions};
use futures_util::StreamExt;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::warn;

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
            Ok(c) => c,
            Err(e) => {
                warn!("Docker list_containers failed: {}", e);
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

        let new_handles: Vec<(String, tokio::task::JoinHandle<()>)> = {
            let mut out = Vec::with_capacity(to_add.len());
            for (id, name) in to_add {
                let handle = self.start_monitoring(id.clone(), name).await;
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
        if !to_remove.is_empty() {
            let mut live = self.live_stats.write().await;
            for id in &to_remove {
                live.remove(id);
            }
        }

        self.get_cached_stats().await
    }

    async fn start_monitoring(&self, id: String, name: String) -> tokio::task::JoinHandle<()> {
        let docker = self.docker.clone();
        let live_stats = self.live_stats.clone();
        let active_streams = self.active_streams.clone();

        tokio::spawn(async move {
            let options = StatsOptions {
                stream: true,
                ..Default::default()
            };
            let mut stream = docker.stats(&id, Some(options));

            while let Some(result) = stream.next().await {
                match result {
                    Ok(s) => {
                        if let Some(stats) = stats::process_statistics(&s, &id, &name) {
                            live_stats.write().await.insert(id.clone(), stats);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Stats stream error for container {}: {}", name, e);
                        break;
                    }
                }
            }
            tracing::info!("Stats stream ended for container {}", name);
            active_streams.write().await.remove(&id);
        })
    }

    async fn get_cached_stats(&self) -> Vec<ContainerStats> {
        let live = self.live_stats.read().await;
        live.values().cloned().collect()
    }
}
