use crate::config::Config;
use crate::models::gallery::Gallery;
use crate::models::image::Image;
use crate::models::request::DownloadRequest;
use crate::models::video::Video;
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

pub async fn run_reset_check(pool: &SqlitePool, config: &Config) {
    info!("Starting reset verification: checking completed requests and person images");

    let storage_dir = PathBuf::from(&config.storage_dir);
    let mut total_reset = 0i64;
    let mut checked_requests = 0i64;

    // 1. Check completed requests (images + videos)
    let completed = match sqlx::query_as::<_, DownloadRequest>(
        "SELECT r.*,
         (SELECT COUNT(*) FROM images i JOIN galleries g ON i.gallery_id = g.id WHERE g.request_id = r.id) as image_count,
         (SELECT COUNT(*) FROM videos v WHERE v.request_id = r.id) as video_count
         FROM requests r WHERE r.status = 'completed'"
    )
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            warn!(error = %e, "Failed to fetch completed requests for reset check");
            return;
        }
    };

    for request in completed {
        checked_requests += 1;
        let mut any_missing = false;

        // Check images
        if let Ok(galleries) = Gallery::get_by_request_id(pool, &request.id).await {
            for gallery in galleries {
                if let Ok(images) = Image::get_by_gallery_id(pool, &gallery.id).await {
                    for image in images {
                        let path = storage_dir.join("images").join(format!("{}.{}", image.hash, image.extension));
                        if !path.exists() {
                            warn!(hash = %image.hash, path = %path.display(), "Missing image file for completed request");
                            any_missing = true;
                        }
                    }
                }
            }
        }

        // Check videos
        if let Ok(videos) = Video::get_by_request_id(pool, &request.id).await {
            for video in videos {
                let path = storage_dir.join("videos").join(format!("{}.{}", video.hash, video.extension));
                if !path.exists() {
                    warn!(hash = %video.hash, path = %path.display(), "Missing video file for completed request");
                    any_missing = true;
                }
            }
        }

        if any_missing {
            if let Err(e) = DownloadRequest::update_status(pool, &request.id, "pending", None).await {
                warn!(error = %e, request_id = %request.id, "Failed to reset request status");
            } else {
                info!(request_id = %request.id, url = %request.url, "Reset to pending due to missing media files");
                total_reset += 1;
            }
        }
    }

    // 2. Check person images (profile pictures)
    let person_images = match sqlx::query_as::<_, crate::models::person::PersonImage>(
        "SELECT * FROM person_images"
    )
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            warn!(error = %e, "Failed to fetch person images for reset check");
            info!(checked = checked_requests, reset = total_reset, "Request reset verification complete (person images skipped due to error)");
            return;
        }
    };

    for pi in person_images {
        let path = storage_dir.join("persons").join(format!("{}.{}", pi.hash, pi.extension));
        if !path.exists() {
            warn!(person_id = %pi.person_id, hash = %pi.hash, "Missing person image, removing stale record");
            if let Err(e) = sqlx::query("DELETE FROM person_images WHERE id = ?")
                .bind(&pi.id)
                .execute(pool)
                .await
            {
                warn!(error = %e, "Failed to delete stale person image record");
            }
            // Also remove thumbnail if it exists
            let thumb_path = storage_dir.join("thumbnails").join(format!("{}.jpg", pi.hash));
            if thumb_path.exists() {
                let _ = tokio::fs::remove_file(&thumb_path).await;
            }
        }
    }

    info!(
        checked = checked_requests,
        reset = total_reset,
        "Reset verification complete"
    );
}
