use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};
use md5::{Digest, Md5};
use tracing::{info, warn};

use crate::models::person::{Person, PersonAlias, PersonImage, PersonInput};
use crate::services::stashdb;
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

/// POST /api/admin/regather-stashdb — Forcefully regather StashDB data for every person
/// that has a stashdb_id set, purging old data (including photos) and downloading fresh.
pub async fn regather_stashdb(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if state.config.stashdb_api_key.is_none() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "StashDB API key not configured" })),
        ));
    }

    let persons: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, stashdb_id FROM persons WHERE stashdb_id IS NOT NULL",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "Failed to fetch persons with stashdb_id");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to fetch persons" })),
        )
    })?;

    let total = persons.len();

    if total == 0 {
        return Ok(Json(serde_json::json!({
            "status": "ok",
            "total": 0,
            "message": "No persons with StashDB data found"
        })));
    }

    let db = state.db.clone();
    let config = state.config.clone();

    tokio::spawn(async move {
        regather_all_stashdb_data(db, config, persons).await;
    });

    Ok(Json(serde_json::json!({
        "status": "started",
        "total": total,
        "message": format!("Regathering StashDB data for {} persons in background", total)
    })))
}

async fn regather_all_stashdb_data(
    pool: sqlx::SqlitePool,
    config: crate::config::Config,
    persons: Vec<(String, String)>,
) {
    let Some(ref api_key) = config.stashdb_api_key else {
        info!("StashDB API key not configured, aborting regather");
        return;
    };
    let http_client = &config.http_client;

    info!("Regathering StashDB data for {} persons", persons.len());

    let storage_dir = std::path::PathBuf::from(&config.storage_dir);
    let persons_dir = storage_dir.join("persons");
    let thumb_dir = storage_dir.join("thumbnails");
    tokio::fs::create_dir_all(&persons_dir).await.ok();
    tokio::fs::create_dir_all(&thumb_dir).await.ok();

    for (person_id, stashdb_id) in &persons {
        // 1. Purge old images from disk (if no other person references them) and DB
        let old_images = match PersonImage::get_for_person(&pool, person_id).await {
            Ok(imgs) => imgs,
            Err(e) => {
                warn!(error = %e, person_id = %person_id, "Failed to get old images");
                continue;
            }
        };

        for img in &old_images {
            let other_refs: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM person_images WHERE hash = ? AND person_id != ?",
            )
            .bind(&img.hash)
            .bind(person_id)
            .fetch_one(&pool)
            .await
            .unwrap_or((0,));

            if other_refs.0 == 0 {
                let file_path = persons_dir.join(format!("{}.{}", img.hash, img.extension));
                let _ = tokio::fs::remove_file(&file_path).await;
                let thumb_path = thumb_dir.join(format!("{}.jpg", img.hash));
                let _ = tokio::fs::remove_file(&thumb_path).await;
            }
        }

        if let Err(e) = sqlx::query("DELETE FROM person_images WHERE person_id = ?")
            .bind(person_id)
            .execute(&pool)
            .await
        {
            warn!(error = %e, person_id = %person_id, "Failed to delete old images");
            continue;
        }

        // 2. Fetch fresh performer data from StashDB
        let performer = match stashdb::get_performer(http_client, api_key, stashdb_id).await {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, person_id = %person_id, "Failed to fetch from StashDB");
                continue;
            }
        };

        // 3. Update person metadata and aliases
        let measurements_str = performer.measurements.as_ref().map(|m| {
            let mut parts = Vec::new();
            if let (Some(band), Some(ref cup)) = (m.band_size, &m.cup_size) {
                parts.push(format!("{}{}", band, cup));
            }
            if let Some(waist) = m.waist {
                parts.push(waist.to_string());
            }
            if let Some(hip) = m.hip {
                parts.push(hip.to_string());
            }
            parts.join("-")
        });

        let input = PersonInput {
            name: Some(performer.name.clone()),
            disambiguation: performer.disambiguation.clone(),
            gender: performer.gender.clone(),
            ethnicity: performer.ethnicity.clone(),
            country: performer.country.clone(),
            height: performer.height,
            hair_color: performer.hair_color.clone(),
            eye_color: performer.eye_color.clone(),
            measurements: measurements_str,
            breast_type: performer.breast_type.clone(),
            career_start_year: performer.career_start_year,
            career_end_year: performer.career_end_year,
            bio: None,
            extra_data: None,
            aliases: Some(performer.aliases.clone()),
        };

        if let Err(e) = Person::update(&pool, person_id, &input).await {
            warn!(error = %e, person_id = %person_id, "Failed to update person metadata");
            continue;
        }

        if let Err(e) = PersonAlias::set_aliases(&pool, person_id, &performer.aliases).await {
            warn!(error = %e, person_id = %person_id, "Failed to set aliases");
        }

        // 4. Download new images
        for (i, stash_img) in performer.images.iter().enumerate() {
            match stashdb::download_image(http_client, &stash_img.url).await {
                Ok(data) => {
                    let mut hasher = Md5::new();
                    hasher.update(&data);
                    let hash = format!("{:x}", hasher.finalize());
                    let extension = stash_img
                        .url
                        .rsplit('.')
                        .next()
                        .unwrap_or("jpg")
                        .split('?')
                        .next()
                        .unwrap_or("jpg")
                        .to_lowercase();
                    let extension =
                        if extension.len() > 4 { "jpg".to_string() } else { extension };

                    let file_path = persons_dir.join(format!("{}.{}", hash, extension));
                    if tokio::fs::write(&file_path, &data).await.is_ok() {
                        let (w, h) = if let Ok(img) = image::load_from_memory(&data) {
                            let thumb_path = thumb_dir.join(format!("{}.jpg", hash));
                            if !thumb_path.exists() {
                                let thumb = img.thumbnail(400, 400);
                                let _ = thumb.save(&thumb_path);
                            }
                            (Some(img.width() as i32), Some(img.height() as i32))
                        } else {
                            (stash_img.width, stash_img.height)
                        };

                        let _ = PersonImage::create(
                            &pool,
                            person_id,
                            &hash,
                            &extension,
                            w,
                            h,
                            i == 0,
                            Some(&stash_img.url),
                        )
                        .await;
                    }
                }
                Err(e) => {
                    warn!(url = %stash_img.url, error = %e, "Failed to download StashDB image");
                }
            }
        }

        info!(
            person_id = %person_id,
            stashdb_id = %stashdb_id,
            "Regathered StashDB data successfully"
        );
    }

    info!("StashDB regather complete for {} persons", persons.len());
}
