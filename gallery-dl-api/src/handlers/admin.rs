use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::AppState;

#[derive(Serialize)]
pub struct HostStatus {
    pub host: String,
    pub is_down: bool,
    pub last_check_at: Option<u64>,
    pub next_check_at: Option<u64>,
}

#[derive(Serialize)]
pub struct ListHostsResponse {
    pub hosts: Vec<HostStatus>,
}

fn system_time_to_epoch_secs(t: SystemTime) -> Option<u64> {
    t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// GET /api/admin/hosts — List all known hosts with their health status.
pub async fn list_hosts(
    State(state): State<AppState>,
) -> Json<ListHostsResponse> {
    let hosts = state.site_health.all_hosts().await;
    let hosts = hosts
        .into_iter()
        .map(|(host, entry)| HostStatus {
            host,
            is_down: entry.is_down,
            last_check_at: entry.last_check_at.and_then(system_time_to_epoch_secs),
            next_check_at: entry.next_check_at.and_then(system_time_to_epoch_secs),
        })
        .collect();
    // Sort by host name for consistent display
    let mut resp = ListHostsResponse { hosts };
    resp.hosts.sort_by(|a, b| a.host.cmp(&b.host));
    Json(resp)
}

/// POST /api/admin/hosts/{host}/check — Manually trigger a health check for a host.
pub async fn check_host(
    State(state): State<AppState>,
    Path(host): Path<String>,
) -> Result<Json<HostStatus>, (StatusCode, Json<serde_json::Value>)> {
    if host.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "host is required" })),
        ));
    }

    state.site_health.check_host(&state.config.http_client, &host).await;

    let entry = state.site_health.get(&host).await;
    match entry {
        Some(e) => Ok(Json(HostStatus {
            host,
            is_down: e.is_down,
            last_check_at: e.last_check_at.and_then(system_time_to_epoch_secs),
            next_check_at: e.next_check_at.and_then(system_time_to_epoch_secs),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "host not found" })),
        )),
    }
}

/// POST /api/admin/hosts/{host}/mark-down — Manually mark a host as down without probing.
pub async fn mark_host_down(
    State(state): State<AppState>,
    Path(host): Path<String>,
) -> Result<Json<HostStatus>, (StatusCode, Json<serde_json::Value>)> {
    if host.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "host is required" })),
        ));
    }

    state.site_health.mark_down(&host).await;

    let entry = state.site_health.get(&host).await;
    match entry {
        Some(e) => Ok(Json(HostStatus {
            host,
            is_down: e.is_down,
            last_check_at: e.last_check_at.and_then(system_time_to_epoch_secs),
            next_check_at: e.next_check_at.and_then(system_time_to_epoch_secs),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "host not found" })),
        )),
    }
}
