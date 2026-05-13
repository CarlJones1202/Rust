use crate::config::Config;
use crate::models::gallery::Gallery;
use crate::models::image::Image;
use crate::models::request::DownloadRequest;
use crate::models::video::Video;
use crate::services::downloader;
use crate::services::file_processor::{self, MediaType};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tracing::{error, info};

/// A job submitted to the download queue.
#[derive(Debug, Clone)]
pub struct DownloadJob {
    pub request_id: String,
    pub url: String,
}

/// Sender half for submitting jobs to the download queue.
pub type JobSender = mpsc::UnboundedSender<DownloadJob>;

/// Spawn the background download worker that processes jobs from the queue.
pub fn spawn_worker(
    pool: SqlitePool,
    config: Config,
) -> JobSender {
    let (tx, rx) = mpsc::unbounded_channel::<DownloadJob>();

    tokio::spawn(run_worker(rx, pool, config));

    tx
}

/// Main worker loop: receives jobs and spawns bounded concurrent tasks.
async fn run_worker(
    mut rx: mpsc::UnboundedReceiver<DownloadJob>,
    pool: SqlitePool,
    config: Config,
) {
    let semaphore = Arc::new(Semaphore::new(config.max_concurrent_downloads));

    info!(
        max_concurrent = config.max_concurrent_downloads,
        "Download worker started"
    );

    while let Some(job) = rx.recv().await {
        let permit = semaphore.clone().acquire_owned().await;
        if permit.is_err() {
            error!("Semaphore closed unexpectedly");
            break;
        }
        let permit = permit.unwrap();

        let pool = pool.clone();
        let config = config.clone();

        tokio::spawn(async move {
            process_job(&pool, &config, &job).await;
            drop(permit); // Release the semaphore permit
        });
    }

    info!("Download worker shutting down");
}

/// Process a single download job.
async fn process_job(pool: &SqlitePool, config: &Config, job: &DownloadJob) {
    info!(request_id = %job.request_id, url = %job.url, "Processing download job");

    // Update status to downloading
    if let Err(e) =
        DownloadRequest::update_status(pool, &job.request_id, "downloading", None).await
    {
        error!(error = %e, "Failed to update request status to downloading");
        return;
    }

    // Create a per-request temp directory
    let temp_dir = PathBuf::from(&config.storage_dir)
        .join("temp")
        .join(&job.request_id);

    // Run gallery-dl
    let download_result =
        match downloader::run_gallery_dl(&config.gallery_dl_bin, &job.url, &temp_dir).await {
            Ok(result) => result,
            Err(e) => {
                error!(error = %e, "gallery-dl failed");
                let _ = DownloadRequest::update_status(
                    pool,
                    &job.request_id,
                    "failed",
                    Some(&e),
                )
                .await;
                // Clean up temp dir
                let _ = tokio::fs::remove_dir_all(&temp_dir).await;
                return;
            }
        };

    // Update status to processing
    if let Err(e) =
        DownloadRequest::update_status(pool, &job.request_id, "processing", None).await
    {
        error!(error = %e, "Failed to update request status to processing");
        return;
    }

    // Process files (hash, classify, move)
    let storage_dir = PathBuf::from(&config.storage_dir);
    let processed_files =
        match file_processor::process_files(download_result.files, &storage_dir).await {
            Ok(files) => files,
            Err(e) => {
                error!(error = %e, "File processing failed");
                let _ = DownloadRequest::update_status(
                    pool,
                    &job.request_id,
                    "failed",
                    Some(&e),
                )
                .await;
                let _ = tokio::fs::remove_dir_all(&temp_dir).await;
                return;
            }
        };

    // Separate images and videos
    let images: Vec<_> = processed_files
        .iter()
        .filter(|f| f.media_type == MediaType::Image)
        .collect();
    let videos: Vec<_> = processed_files
        .iter()
        .filter(|f| f.media_type == MediaType::Video)
        .collect();

    // Create gallery + image records if there are images
    if !images.is_empty() {
        match Gallery::create(pool, &job.request_id, None).await {
            Ok(gallery) => {
                for img in &images {
                    if let Err(e) = Image::create(
                        pool,
                        &gallery.id,
                        &img.hash,
                        &img.extension,
                        Some(&img.original_filename),
                        img.file_size_bytes,
                    )
                    .await
                    {
                        error!(error = %e, hash = %img.hash, "Failed to insert image record");
                    }
                }
                info!(
                    gallery_id = %gallery.id,
                    image_count = images.len(),
                    "Created gallery with images"
                );
            }
            Err(e) => {
                error!(error = %e, "Failed to create gallery");
                let _ = DownloadRequest::update_status(
                    pool,
                    &job.request_id,
                    "failed",
                    Some(&format!("Failed to create gallery: {e}")),
                )
                .await;
                let _ = tokio::fs::remove_dir_all(&temp_dir).await;
                return;
            }
        }
    }

    // Create video records (linked directly to request)
    for vid in &videos {
        if let Err(e) = Video::create(
            pool,
            &job.request_id,
            &vid.hash,
            &vid.extension,
            Some(&vid.original_filename),
            vid.file_size_bytes,
        )
        .await
        {
            error!(error = %e, hash = %vid.hash, "Failed to insert video record");
        }
    }

    if !videos.is_empty() {
        info!(video_count = videos.len(), "Created video records");
    }

    // Mark request as completed
    if let Err(e) =
        DownloadRequest::update_status(pool, &job.request_id, "completed", None).await
    {
        error!(error = %e, "Failed to update request status to completed");
    }

    // Clean up temp dir
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;

    info!(
        request_id = %job.request_id,
        images = images.len(),
        videos = videos.len(),
        "Download job completed"
    );
}
