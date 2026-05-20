use crate::config::Config;
use crate::models::person::PersonImage;
use crate::models::request::DownloadRequest;
use crate::services::stashdb;
use md5::{Digest, Md5};
use sqlx::SqlitePool;
use std::path::PathBuf;
use tracing::{info, warn};

pub async fn run_requeue_failed(pool: &SqlitePool) {
    info!("Requeue-failed flag detected: resetting all failed requests to pending");

    let failed = match sqlx::query_as::<_, DownloadRequest>(
        "SELECT r.*,
         (SELECT COUNT(*) FROM images i JOIN galleries g ON i.gallery_id = g.id WHERE g.request_id = r.id) as image_count,
         (SELECT COUNT(*) FROM videos v WHERE v.request_id = r.id) as video_count
         FROM requests r WHERE r.status = 'failed'"
    )
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            warn!(error = %e, "Failed to fetch failed requests for requeue");
            return;
        }
    };

    let count = failed.len();
    for request in &failed {
        if let Err(e) = DownloadRequest::update_status(pool, &request.id, "pending", None).await {
            warn!(error = %e, request_id = %request.id, "Failed to requeue request");
        } else {
            info!(request_id = %request.id, url = %request.url, "Requeued failed request to pending");
        }
    }

    info!(count = count, "Requeue-failed complete — failed requests reset to pending and will be picked up by recovery");
}

pub async fn run_requeue_all(pool: &SqlitePool, config: &Config) -> u64 {
    info!("Requeue-all flag detected: purging media and resetting all requests to pending");

    let storage_dir = PathBuf::from(&config.storage_dir);

    // 1. Purge media files from disk (images, videos, thumbnails, trickplay, temp)
    // We intentionally leave the 'persons' directory — stashdb will repopulate it below.
    let subdirs = ["images", "videos", "thumbnails", "trickplay", "temp"];
    for subdir in subdirs.iter() {
        let path = storage_dir.join(subdir);
        if path.exists() {
            if let Ok(mut entries) = tokio::fs::read_dir(&path).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let entry_path = entry.path();
                    if entry_path.is_file() {
                        let _ = tokio::fs::remove_file(entry_path).await;
                    } else if entry_path.is_dir() {
                        let _ = tokio::fs::remove_dir_all(entry_path).await;
                    }
                }
            }
        }
    }
    info!("Media files purged from disk.");

    // 2. Purge gallery, video, and person_image records from DB
    if let Err(e) = sqlx::query("DELETE FROM galleries").execute(pool).await {
        warn!(error = %e, "Failed to delete from galleries");
    }
    if let Err(e) = sqlx::query("DELETE FROM videos").execute(pool).await {
        warn!(error = %e, "Failed to delete from videos");
    }
    if let Err(e) = sqlx::query("DELETE FROM person_images").execute(pool).await {
        warn!(error = %e, "Failed to delete from person_images");
    }
    info!("DB records purged (galleries, videos, person_images).");

    // 3. Reset all requests to pending and get the count
    let count = match sqlx::query("UPDATE requests SET status = 'pending', error_message = NULL")
        .execute(pool)
        .await
    {
        Ok(result) => {
            let n = result.rows_affected();
            info!(count = n, "All requests reset to pending.");
            n
        }
        Err(e) => {
            warn!(error = %e, "Failed to update all requests to pending");
            0
        }
    };

    count
}

pub async fn redownload_stashdb_images(pool: &SqlitePool, config: &Config, http_client: &reqwest::Client) {
    let Some(api_key) = config.stashdb_api_key.as_deref() else {
        info!("StashDB API key not configured, skipping profile picture redownloads.");
        return;
    };

    let storage_dir = PathBuf::from(&config.storage_dir);

    let persons_with_stashdb: Vec<(String, String)> = match sqlx::query_as(
        "SELECT id, stashdb_id FROM persons WHERE stashdb_id IS NOT NULL"
    )
    .fetch_all(pool)
    .await {
        Ok(rows) => rows,
        Err(e) => {
            warn!(error = %e, "Failed to fetch persons with stashdb_id");
            return;
        }
    };

    info!("Found {} persons with stashdb_id. Retriggering downloads...", persons_with_stashdb.len());

    let persons_dir = storage_dir.join("persons");
    tokio::fs::create_dir_all(&persons_dir).await.ok();
    let thumb_dir = storage_dir.join("thumbnails");
    tokio::fs::create_dir_all(&thumb_dir).await.ok();

    for (person_id, stashdb_id) in persons_with_stashdb {
        match stashdb::get_performer(http_client, api_key, &stashdb_id).await {
            Ok(performer) => {
                for (i, stash_img) in performer.images.iter().enumerate() {
                    match stashdb::download_image(http_client, &stash_img.url).await {
                        Ok(data) => {
                            let mut hasher = Md5::new();
                            hasher.update(&data);
                            let hash = format!("{:x}", hasher.finalize());
                            let extension = stash_img.url.rsplit('.').next().unwrap_or("jpg").split('?').next().unwrap_or("jpg").to_lowercase();
                            let extension = if extension.len() > 4 { "jpg".to_string() } else { extension };

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
                                    pool,
                                    &person_id,
                                    &hash,
                                    &extension,
                                    w,
                                    h,
                                    i == 0,
                                    Some(&stash_img.url)
                                ).await;
                            }
                        }
                        Err(e) => warn!(url = %stash_img.url, error = %e, "Failed to redownload StashDB image"),
                    }
                }
            }
            Err(e) => warn!(person_id = %person_id, error = %e, "Failed to fetch StashDB performer details"),
        }
    }
    info!("StashDB profile pictures redownloaded successfully.");
}

pub async fn run_reset_all(pool: &SqlitePool, config: &Config) {
    info!("Starting full reset: purging all media and DB records");

    let storage_dir = PathBuf::from(&config.storage_dir);
    let subdirs = ["images", "videos", "thumbnails", "trickplay", "temp"];

    for subdir in subdirs.iter() {
        let path = storage_dir.join(subdir);
        if path.exists() {
            if let Ok(mut entries) = tokio::fs::read_dir(&path).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let entry_path = entry.path();
                    if entry_path.is_file() {
                        let _ = tokio::fs::remove_file(entry_path).await;
                    } else if entry_path.is_dir() {
                        let _ = tokio::fs::remove_dir_all(entry_path).await;
                    }
                }
            }
        }
    }

    if let Err(e) = sqlx::query("DELETE FROM galleries").execute(pool).await {
        warn!(error = %e, "Failed to delete from galleries");
    }
    if let Err(e) = sqlx::query("DELETE FROM videos").execute(pool).await {
        warn!(error = %e, "Failed to delete from videos");
    }
    if let Err(e) = sqlx::query("UPDATE requests SET status = 'pending', error_message = NULL").execute(pool).await {
        warn!(error = %e, "Failed to update requests status");
    }

    info!("Full reset complete. All requests requeued and data purged.");
}
