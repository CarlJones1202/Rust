use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// Shared state tracking which hosts are known to be down.
#[derive(Clone, Debug)]
pub struct SiteHealth {
    inner: Arc<RwLock<HashMap<String, bool>>>,
}

impl SiteHealth {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Mark a host as down (unreachable).
    pub async fn mark_down(&self, host: &str) {
        let mut map = self.inner.write().await;
        map.insert(host.to_string(), true);
        warn!(host = %host, "Marked host as down");
    }

    /// Mark a host as up (reachable again).
    pub async fn mark_up(&self, host: &str) {
        let mut map = self.inner.write().await;
        map.remove(host);
        info!(host = %host, "Marked host as up");
    }

    /// Check if a host is currently known to be down.
    pub async fn is_down(&self, host: &str) -> bool {
        let map = self.inner.read().await;
        map.contains_key(host)
    }

    /// Return a snapshot of all down hosts.
    pub async fn down_hosts(&self) -> Vec<String> {
        let map = self.inner.read().await;
        map.keys().cloned().collect()
    }
}

impl Default for SiteHealth {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the hostname from a URL string.
/// Returns `None` if the URL is malformed or has no host.
pub fn extract_host(url: &str) -> Option<String> {
    let url = url.trim();
    let after_protocol = if let Some(pos) = url.find("://") {
        &url[pos + 3..]
    } else {
        url
    };
    let host = after_protocol
        .split('/')
        .next()
        .and_then(|s| s.split('?').next())
        .unwrap_or(after_protocol);
    if host.is_empty() {
        None
    } else {
        let host = host.trim().to_lowercase();
        // Strip port if present
        Some(host.split(':').next().unwrap_or(&host).to_string())
    }
}

/// Spawn a background task that periodically checks whether previously-down
/// hosts have recovered, and marks them up if they respond successfully.
/// Sends recovered host names through `on_host_up` so callers can re-queue pending jobs.
pub fn spawn_periodic_checker(
    site_health: SiteHealth,
    http_client: reqwest::Client,
    on_host_up: mpsc::UnboundedSender<String>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        // Tolerate drift — don't try to catch up if we're behind
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let hosts = site_health.down_hosts().await;
            if hosts.is_empty() {
                continue;
            }
            info!(count = hosts.len(), "Checking down hosts for recovery");
            for host in &hosts {
                let url = format!("https://{}", host);
                match http_client.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        info!(host = %host, "Host appears to be back up, re-queueing pending jobs");
                        site_health.mark_up(host).await;
                        let _ = on_host_up.send(host.clone());
                    }
                    Ok(resp) => {
                        debug!(host = %host, status = %resp.status(), "Host still down");
                    }
                    Err(e) => {
                        debug!(host = %host, error = %e, "Host still unreachable");
                    }
                }
            }
        }
    });
}
