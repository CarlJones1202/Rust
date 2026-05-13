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

    // Update status to processing immediately, since files stream in real-time
    if let Err(e) =
        DownloadRequest::update_status(pool, &job.request_id, "processing", None).await
    {
        error!(error = %e, "Failed to update request status to processing");
        return;
    }

    // Create a per-request temp directory
    let temp_dir = PathBuf::from(&config.storage_dir)
        .join("temp")
        .join(&job.request_id);

    let (tx, mut rx) = mpsc::unbounded_channel();
    let gallery_dl_bin = config.gallery_dl_bin.clone();
    let url = job.url.clone();
    let temp_dir_clone = temp_dir.clone();

    // Spawn gallery-dl in the background
    let dl_task = tokio::spawn(async move {
        downloader::run_gallery_dl(&gallery_dl_bin, &url, &temp_dir_clone, tx).await
    });

    let storage_dir = PathBuf::from(&config.storage_dir);
    let mut gallery_id = None;
    let mut image_count = 0;
    let mut video_count = 0;

    // Process files sequentially as they arrive
    while let Some(file_path) = rx.recv().await {
        match file_processor::process_single_file(&file_path, &storage_dir).await {
            Ok(Some(processed)) => {
                match processed.media_type {
                    MediaType::Image => {
                        // Lazily create gallery on first image
                        if gallery_id.is_none() {
                            match Gallery::create(pool, &job.request_id, None).await {
                                Ok(g) => {
                                    info!(gallery_id = %g.id, "Created gallery");
                                    gallery_id = Some(g.id);
                                }
                                Err(e) => {
                                    error!(error = %e, "Failed to create gallery");
                                    continue;
                                }
                            }
                        }

                        if let Some(ref gid) = gallery_id {
                            if let Err(e) = Image::create(
                                pool,
                                gid,
                                &processed.hash,
                                &processed.extension,
                                Some(&processed.original_filename),
                                processed.file_size_bytes,
                                processed.width,
                                processed.height,
                            )
                            .await
                            {
                                error!(error = %e, hash = %processed.hash, "Failed to insert image record");
                            } else {
                                image_count += 1;
                            }
                        }
                    }
                    MediaType::Video => {
                        if let Err(e) = Video::create(
                            pool,
                            &job.request_id,
                            &processed.hash,
                            &processed.extension,
                            Some(&processed.original_filename),
                            processed.file_size_bytes,
                        )
                        .await
                        {
                            error!(error = %e, hash = %processed.hash, "Failed to insert video record");
                        } else {
                            video_count += 1;
                        }
                    }
                    MediaType::Unknown => {}
                }
            }
            Ok(None) => {}
            Err(e) => {
                error!(error = %e, path = %file_path.display(), "File processing failed");
            }
        }
    }

    // Wait for the gallery-dl process to finish to check for final errors
    let dl_result = match dl_task.await {
        Ok(res) => res,
        Err(e) => {
            error!(error = %e, "gallery-dl task panicked");
            Err("gallery-dl task panicked".to_string())
        }
    };

    if let Err(e) = dl_result {
        error!(error = %e, "gallery-dl failed");
        let _ = DownloadRequest::update_status(pool, &job.request_id, "failed", Some(&e)).await;
    } else {
        let _ = DownloadRequest::update_status(pool, &job.request_id, "completed", None).await;
        info!(
            request_id = %job.request_id,
            images = image_count,
            videos = video_count,
            "Download job completed successfully"
        );
    }

    // Clean up temp dir
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}
