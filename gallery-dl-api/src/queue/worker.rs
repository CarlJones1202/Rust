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
    pub title: Option<String>,
}

/// Sender half for submitting jobs to the download queue.
pub type JobSender = mpsc::UnboundedSender<DownloadJob>;

#[derive(Debug, Clone, Copy, PartialEq)]
enum JobBucket {
    Image,
    Video,
}

/// Simple classification of URLs into buckets.
fn classify_job(url: &str) -> JobBucket {
    let url = url.to_lowercase();
    // Common video-only or video-centric domains
    if url.contains("youtube.com")
        || url.contains("youtu.be")
        || url.contains("vimeo.com")
        || url.contains("dailymotion.com")
        || url.contains("bilibili.com")
        || url.contains("twitch.tv")
        || url.contains("tnaflix.com")
        || url.contains("pmvhaven.com")
    {
        JobBucket::Video
    } else {
        JobBucket::Image
    }
}

/// Spawn the background download worker that processes jobs from the queue.
pub fn spawn_worker(
    pool: SqlitePool,
    config: Config,
) -> JobSender {
    let (tx, rx) = mpsc::unbounded_channel::<DownloadJob>();

    tokio::spawn(run_worker(rx, pool, config));

    tx
}

/// Query the database for unfinished jobs and re-queue them.
pub async fn recover_pending_jobs(pool: &SqlitePool, tx: JobSender) {
    match DownloadRequest::list_unfinished(pool).await {
        Ok(requests) => {
            if requests.is_empty() {
                return;
            }
            info!(count = requests.len(), "Recovering unfinished download jobs");
            for req in requests {
                info!(request_id = %req.id, url = %req.url, "Re-queueing job");
                let _ = tx.send(DownloadJob {
                    request_id: req.id,
                    url: req.url,
                    title: req.title,
                });
            }
        }
        Err(e) => {
            error!(error = %e, "Failed to list unfinished jobs for recovery");
        }
    }
}

/// Main worker dispatcher: receives jobs and routes them to bucket workers.
async fn run_worker(
    mut rx: mpsc::UnboundedReceiver<DownloadJob>,
    pool: SqlitePool,
    config: Config,
) {
    let (image_tx, image_rx) = mpsc::unbounded_channel::<DownloadJob>();
    let (video_tx, video_rx) = mpsc::unbounded_channel::<DownloadJob>();

    // Spawn bucket workers
    tokio::spawn(bucket_worker(
        image_rx,
        pool.clone(),
        config.clone(),
        config.max_concurrent_downloads,
        "Image",
    ));
    tokio::spawn(bucket_worker(
        video_rx,
        pool.clone(),
        config.clone(),
        config.max_concurrent_video_downloads,
        "Video",
    ));

    info!(
        max_images = config.max_concurrent_downloads,
        max_videos = config.max_concurrent_video_downloads,
        "Download dispatcher started"
    );

    while let Some(job) = rx.recv().await {
        let bucket = classify_job(&job.url);
        match bucket {
            JobBucket::Image => {
                let _ = image_tx.send(job);
            }
            JobBucket::Video => {
                let _ = video_tx.send(job);
            }
        }
    }

    info!("Download dispatcher shutting down");
}

/// A worker that processes jobs from a specific bucket with its own concurrency limit.
async fn bucket_worker(
    mut rx: mpsc::UnboundedReceiver<DownloadJob>,
    pool: SqlitePool,
    config: Config,
    concurrency: usize,
    name: &'static str,
) {
    let semaphore = Arc::new(Semaphore::new(concurrency));

    info!(
        bucket = name,
        concurrency = concurrency,
        "Bucket worker started"
    );

    while let Some(job) = rx.recv().await {
        let permit = semaphore.clone().acquire_owned().await;
        if permit.is_err() {
            error!(bucket = name, "Bucket semaphore closed unexpectedly");
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

    info!(bucket = name, "Bucket worker shutting down");
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
    let cookies_from_browser = config.cookies_from_browser.clone();

    // Spawn gallery-dl in the background
    let dl_task = tokio::spawn(async move {
        downloader::run_gallery_dl(
            &gallery_dl_bin,
            &url,
            &temp_dir_clone,
            cookies_from_browser.as_deref(),
            tx,
        )
        .await
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
                        // Lazily reuse or create gallery
                        if gallery_id.is_none() {
                            match Gallery::get_by_request_id(pool, &job.request_id).await {
                                Ok(galleries) if !galleries.is_empty() => {
                                    gallery_id = Some(galleries[0].id.clone());
                                    info!(gallery_id = %gallery_id.as_ref().unwrap(), "Reusing existing gallery");
                                }
                                _ => {
                                    match Gallery::create(pool, &job.request_id, job.title.as_deref()).await {
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
                            }
                        }

                        if let Some(ref gid) = gallery_id {
                            match Image::create(
                                pool,
                                gid,
                                &processed.hash,
                                &processed.extension,
                                Some(&processed.original_filename),
                                processed.file_size_bytes,
                                processed.width,
                                processed.height,
                                processed.top_colors,
                            )
                            .await
                            {
                                Ok(_) => {
                                    image_count += 1;
                                }
                                Err(sqlx::Error::RowNotFound) => {
                                    // This happens with INSERT OR IGNORE if the row already exists
                                    tracing::debug!(hash = %processed.hash, "Skipping duplicate image");
                                }
                                Err(e) => {
                                    error!(error = %e, hash = %processed.hash, "Failed to insert image record");
                                }
                            }
                        }
                    }
                    MediaType::Video => {
                        match Video::create(
                            pool,
                            &job.request_id,
                            &processed.hash,
                            &processed.extension,
                            Some(&processed.original_filename),
                            processed.file_size_bytes,
                            processed.duration_seconds,
                            processed.width,
                            processed.height,
                        )
                        .await
                        {
                            Ok(_) => {
                                video_count += 1;
                            }
                            Err(sqlx::Error::RowNotFound) => {
                                tracing::debug!(hash = %processed.hash, "Skipping duplicate video");
                            }
                            Err(e) => {
                                error!(error = %e, hash = %processed.hash, "Failed to insert video record");
                            }
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
            if e.is_cancelled() {
                info!(request_id = %job.request_id, "Download task cancelled (likely shutdown)");
                return; // Return without updating status, leaving it as 'processing' for recovery
            }
            error!(error = %e, "gallery-dl task panicked");
            Err("gallery-dl task panicked".to_string())
        }
    };

    if let Err(ref e) = dl_result {
        error!(error = %e, "gallery-dl failed");
        let _ = DownloadRequest::update_status(pool, &job.request_id, "failed", Some(e)).await;
    } else {
        let _ = DownloadRequest::update_status(pool, &job.request_id, "completed", None).await;
        info!(
            request_id = %job.request_id,
            images = image_count,
            videos = video_count,
            "Download job completed successfully"
        );
    }

    // Clean up temp dir only on success to allow resumption if failed/interrupted
    if dl_result.is_ok() {
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
}
