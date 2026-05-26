use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// How often (in seconds) the background checker re-probes down hosts.
const CHECK_INTERVAL_SECS: u64 = 300;

/// Per-host health information tracked in SiteHealth.
#[derive(Clone, Debug)]
pub struct HostEntry {
    pub is_down: bool,
    pub last_check_at: Option<SystemTime>,
    pub next_check_at: Option<SystemTime>,
}

/// Shared state tracking host health and check schedule.
#[derive(Clone, Debug)]
pub struct SiteHealth {
    inner: Arc<RwLock<HashMap<String, HostEntry>>>,
}

impl SiteHealth {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Mark a host as down (unreachable) and record check time.
    pub async fn mark_down(&self, host: &str) {
        let mut map = self.inner.write().await;
        let now = SystemTime::now();
        map.insert(host.to_string(), HostEntry {
            is_down: true,
            last_check_at: Some(now),
            next_check_at: Some(now + Duration::from_secs(CHECK_INTERVAL_SECS)),
        });
        warn!(host = %host, "Marked host as down");
    }

    /// Mark a host as up (reachable again) and record check time.
    pub async fn mark_up(&self, host: &str) {
        let mut map = self.inner.write().await;
        let now = SystemTime::now();
        map.insert(host.to_string(), HostEntry {
            is_down: false,
            last_check_at: Some(now),
            next_check_at: None,
        });
        info!(host = %host, "Marked host as up");
    }

    /// Check if a host is currently known to be down.
    pub async fn is_down(&self, host: &str) -> bool {
        let map = self.inner.read().await;
        map.get(host).map(|e| e.is_down).unwrap_or(false)
    }

    /// Check if we have any knowledge about this host (up or down).
    pub async fn is_known(&self, host: &str) -> bool {
        let map = self.inner.read().await;
        map.contains_key(host)
    }

    /// Return a snapshot of all down hosts (names only).
    pub async fn down_hosts(&self) -> Vec<String> {
        let map = self.inner.read().await;
        map.iter()
            .filter(|(_, e)| e.is_down)
            .map(|(h, _)| h.clone())
            .collect()
    }

    /// Return all known hosts with their full health information.
    pub async fn all_hosts(&self) -> Vec<(String, HostEntry)> {
        let map = self.inner.read().await;
        map.iter()
            .map(|(h, e)| (h.clone(), e.clone()))
            .collect()
    }

    /// Get health info for a single host.
    pub async fn get(&self, host: &str) -> Option<HostEntry> {
        let map = self.inner.read().await;
        map.get(host).cloned()
    }

    /// Probe a single host and update its status in place.
    /// Returns `true` if the host is down after the check.
    pub async fn check_host(&self, http_client: &reqwest::Client, host: &str) -> bool {
        let is_reachable = probe_host(http_client, host).await;
        let now = SystemTime::now();

        if is_reachable {
            let mut map = self.inner.write().await;
            map.insert(host.to_string(), HostEntry {
                is_down: false,
                last_check_at: Some(now),
                next_check_at: None,
            });
            info!(host = %host, "Host is up");
            false
        } else {
            let mut map = self.inner.write().await;
            map.insert(host.to_string(), HostEntry {
                is_down: true,
                last_check_at: Some(now),
                next_check_at: Some(now + Duration::from_secs(CHECK_INTERVAL_SECS)),
            });
            warn!(host = %host, "Host is down");
            true
        }
    }
}

impl Default for SiteHealth {
    fn default() -> Self {
        Self::new()
    }
}

/// Probe whether a host is reachable. Tries HTTPS first; if that fails
/// (connection error or timeout), falls back to HTTP. Returns `true` if
/// we receive *any* HTTP response (2xx/3xx/4xx/5xx) except 502 Bad Gateway —
/// the host is up even if it returns an error page or redirects.
pub async fn probe_host(http_client: &reqwest::Client, host: &str) -> bool {
    for scheme in &["https", "http"] {
        let url = format!("{}://{}", scheme, host);
        let probe = http_client.get(&url).send();
        match tokio::time::timeout(Duration::from_secs(5), probe).await {
            Ok(Ok(response)) => {
                if response.status() == reqwest::StatusCode::BAD_GATEWAY {
                    continue;
                }
                return true;
            }
            _ => continue,
        }
    }
    false
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
        Some(host.split(':').next().unwrap_or(&host).to_string())
    }
}

/// Check all currently-down hosts for recovery.
/// For any host that responds successfully, marks it up and sends its name
/// through `on_host_up` so pending jobs can be re-queued.
async fn check_down_hosts(
    site_health: &SiteHealth,
    http_client: &reqwest::Client,
    on_host_up: &mpsc::UnboundedSender<String>,
) {
    let hosts = site_health.down_hosts().await;
    if hosts.is_empty() {
        return;
    }
    info!(count = hosts.len(), "Checking down hosts for recovery");
    for host in &hosts {
        if probe_host(http_client, host).await {
            info!(host = %host, "Host appears to be back up, re-queueing pending jobs");
            site_health.mark_up(host).await;
            let _ = on_host_up.send(host.clone());
        } else {
            // Update the last checked time even when still down
            let now = SystemTime::now();
            {
                let mut map = site_health.inner.write().await;
                if let Some(entry) = map.get_mut(host) {
                    entry.last_check_at = Some(now);
                    entry.next_check_at = Some(now + Duration::from_secs(CHECK_INTERVAL_SECS));
                }
            }
            debug!(host = %host, "Host still unreachable");
        }
    }
}

/// Spawn a background task that runs an immediate recovery check for all
/// currently-down hosts, then periodically re-checks them every 300s.
/// When a recovered host is found, its name is sent through `on_host_up`
/// so the caller can re-queue pending jobs for that host.
pub fn spawn_periodic_checker(
    site_health: SiteHealth,
    http_client: reqwest::Client,
    on_host_up: mpsc::UnboundedSender<String>,
) {
    tokio::spawn(async move {
        check_down_hosts(&site_health, &http_client, &on_host_up).await;

        let mut interval = tokio::time::interval(Duration::from_secs(CHECK_INTERVAL_SECS));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            check_down_hosts(&site_health, &http_client, &on_host_up).await;
        }
    });
}
