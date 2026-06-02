use crate::config::Config;
use crate::models::gallery::Gallery;
use crate::models::image::Image;
use crate::models::request::DownloadRequest;
use crate::models::video::Video;
use crate::services::auto_link;
use crate::services::custom_downloader;
use crate::services::downloader;
use crate::services::file_processor::{self, MediaType};
use crate::services::site_checker::{self, SiteHealth};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::{mpsc, Semaphore};
use tracing::{error, info, warn};

/// A job submitted to the download queue.
#[derive(Debug, Clone)]
pub struct DownloadJob {
    pub request_id: String,
    pub url: String,
    pub title: Option<String>,
    pub backup_url: Option<String>,
}

/// Sender half for submitting jobs to the download queue.
pub type JobSender = mpsc::UnboundedSender<DownloadJob>;

/// Controls the concurrency limit for a bucket of downloads at runtime.
/// Allows the admin API to update the limit without restarting the server.
pub struct ConcurrencyController {
    pub semaphore: RwLock<Arc<Semaphore>>,
    pub current_max: AtomicUsize,
}

impl ConcurrencyController {
    pub fn new(max: usize) -> Self {
        Self {
            semaphore: RwLock::new(Arc::new(Semaphore::new(max))),
            current_max: AtomicUsize::new(max),
        }
    }

    pub fn set_max(&self, new_max: usize) {
        let new_sem = Arc::new(Semaphore::new(new_max));
        *self.semaphore.write().unwrap() = new_sem;
        self.current_max.store(new_max, Ordering::SeqCst);
    }
}

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
    site_health: SiteHealth,
    http_client: reqwest::Client,
    image_concurrency: Arc<ConcurrencyController>,
    video_concurrency: Arc<ConcurrencyController>,
) -> JobSender {
    let (tx, rx) = mpsc::unbounded_channel::<DownloadJob>();

    tokio::spawn(run_worker(rx, pool, config, site_health, http_client, image_concurrency, video_concurrency));

    tx
}

/// Query the database for unfinished jobs and re-queue them.
/// Jobs whose host is known-down are skipped (left in their current DB state)
/// so they don't clog the queue. The periodic checker will re-queue them
/// when the host recovers. If the primary host is down but a backup URL
/// is available and its host is up, the job is re-queued with the backup URL.
pub async fn recover_pending_jobs(pool: &SqlitePool, tx: JobSender, site_health: &SiteHealth) {
    match DownloadRequest::list_unfinished(pool).await {
        Ok(requests) => {
            if requests.is_empty() {
                return;
            }
            let mut recovered = 0u32;
            let mut skipped = 0u32;
            for req in &requests {
                // Determine primary host status
                let primary_down = if let Some(ref host) = site_checker::extract_host(&req.url) {
                    site_health.is_down(host).await
                } else {
                    false
                };

                if primary_down {
                    // Primary host is down — try backup if available
                    if let Some(ref backup_url) = req.backup_url {
                        let backup_url = backup_url.trim();
                        if !backup_url.is_empty() {
                            let backup_down =
                                if let Some(ref bh) = site_checker::extract_host(backup_url) {
                                    site_health.is_down(bh).await
                                } else {
                                    false
                                };
                            if !backup_down {
                                info!(
                                    backup_url = %backup_url,
                                    request_id = %req.id,
                                    "Primary host down, re-queueing with backup URL"
                                );
                                let _ = tx.send(DownloadJob {
                                    request_id: req.id.clone(),
                                    url: backup_url.to_string(),
                                    title: req.title.clone(),
                                    backup_url: req.backup_url.clone(),
                                });
                                recovered += 1;
                                continue;
                            }
                        }
                    }
                    info!(
                        request_id = %req.id,
                        "Skipping recovery for down-host job (and backup if any)"
                    );
                    skipped += 1;
                    continue;
                }

                info!(request_id = %req.id, url = %req.url, "Re-queueing job");
                let _ = tx.send(DownloadJob {
                    request_id: req.id.clone(),
                    url: req.url.clone(),
                    title: req.title.clone(),
                    backup_url: req.backup_url.clone(),
                });
                recovered += 1;
            }
            info!(recovered = recovered, skipped = skipped, "Finished recovering jobs");
        }
        Err(e) => {
            error!(error = %e, "Failed to list unfinished jobs for recovery");
        }
    }
}

/// Resolve the effective URL for a job: if the primary host is down,
/// try the backup URL (if available and its host is up).
/// Returns `None` if the job should be skipped entirely.
async fn resolve_effective_url(
    job: &mut DownloadJob,
    site_health: &SiteHealth,
    http_client: &reqwest::Client,
) -> Option<()> {
    let host = site_checker::extract_host(&job.url)?;

    // If we don't know this host yet, probe it proactively
    if !site_health.is_known(&host).await {
        if site_checker::probe_host(http_client, &host).await {
            info!(host = %host, "Host probed and reachable");
            site_health.mark_up(&host).await;
        } else {
            warn!(host = %host, "Host unreachable on probe, marking down");
            site_health.mark_down(&host).await;
        }
    }

    if !site_health.is_down(&host).await {
        return Some(());
    }

    // Primary host is down — try backup URL
    if let Some(ref backup_url) = job.backup_url {
        let backup_url = backup_url.trim();
        if backup_url.is_empty() {
            warn!(host = %host, request_id = %job.request_id, "Primary host down, no backup URL configured, skipping job");
            return None;
        }

        let backup_host = site_checker::extract_host(backup_url);

        // Also probe backup host if unknown
        if let Some(ref bh) = backup_host {
            if !site_health.is_known(bh).await {
                if site_checker::probe_host(http_client, bh).await {
                    info!(backup_host = %bh, "Backup host probed and reachable");
                    site_health.mark_up(bh).await;
                } else {
                    warn!(backup_host = %bh, "Backup host unreachable on probe, marking down");
                    site_health.mark_down(bh).await;
                }
            }
        }

        let backup_down = if let Some(ref bh) = backup_host {
            site_health.is_down(bh).await
        } else {
            false
        };

        if backup_down {
            warn!(host = %host, backup_host = ?backup_host, request_id = %job.request_id, "Primary and backup hosts both down, skipping job");
            return None;
        }

        warn!(host = %host, request_id = %job.request_id, "Primary host down, falling back to backup URL");
        job.url = backup_url.to_string();
        return Some(());
    }

    warn!(host = %host, request_id = %job.request_id, "Primary host down, no backup URL configured, skipping job");
    None
}

/// Main worker dispatcher: receives jobs, resolves effective URL (primary vs backup),
/// and routes them to bucket workers.
async fn run_worker(
    mut rx: mpsc::UnboundedReceiver<DownloadJob>,
    pool: SqlitePool,
    config: Config,
    site_health: SiteHealth,
    http_client: reqwest::Client,
    image_concurrency: Arc<ConcurrencyController>,
    video_concurrency: Arc<ConcurrencyController>,
) {
    let (image_tx, image_rx) = mpsc::unbounded_channel::<DownloadJob>();
    let (video_tx, video_rx) = mpsc::unbounded_channel::<DownloadJob>();

    // Spawn bucket workers
    tokio::spawn(bucket_worker(
        image_rx,
        pool.clone(),
        config.clone(),
        image_concurrency,
        "Image",
        site_health.clone(),
    ));
    tokio::spawn(bucket_worker(
        video_rx,
        pool.clone(),
        config.clone(),
        video_concurrency,
        "Video",
        site_health.clone(),
    ));

    info!(
        max_images = config.max_concurrent_downloads,
        max_videos = config.max_concurrent_video_downloads,
        "Download dispatcher started"
    );

    while let Some(mut job) = rx.recv().await {
        let skip = resolve_effective_url(&mut job, &site_health, &http_client).await;
        if skip.is_none() {
            continue;
        }

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
    concurrency: Arc<ConcurrencyController>,
    name: &'static str,
    site_health: SiteHealth,
) {
    info!(
        bucket = name,
        concurrency = concurrency.current_max.load(Ordering::Relaxed),
        "Bucket worker started"
    );

    while let Some(job) = rx.recv().await {
        // If the host is known-down, skip the job entirely.
        // The periodic checker will mark it up when it recovers,
        // and pending jobs can be re-queued via the API or restart.
        if let Some(host) = site_checker::extract_host(&job.url) {
            if site_health.is_down(&host).await {
                warn!(
                    host = %host,
                    request_id = %job.request_id,
                    "Skipping job for down host (will retry on restart or manual re-queue)"
                );
                continue;
            }
        }

        let semaphore = concurrency.semaphore.read().unwrap().clone();
        let permit = semaphore.acquire_owned().await;
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
    let download_delay = config.download_delay;

    // Check if this is a site we need a custom downloader for
    let custom_site = custom_downloader::recognize_site(&url);

    // Spawn gallery-dl in the background, or custom downloader for unsupported sites
    let dl_task = if let Some(site) = custom_site {
        let http_client = config.http_client.clone();
        tokio::spawn(async move {
            custom_downloader::run_custom_downloader(
                &http_client,
                &url,
                site,
                &temp_dir_clone,
                tx,
            )
            .await
        })
    } else {
        tokio::spawn(async move {
            downloader::run_gallery_dl(
                &gallery_dl_bin,
                &url,
                &temp_dir_clone,
                download_delay,
                cookies_from_browser.as_deref(),
                tx,
            )
            .await
        })
    };

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
    } else if image_count == 0 && video_count == 0 {
        error!("Downloader reported success but produced no media files");
        let _ = DownloadRequest::update_status(
            pool,
            &job.request_id,
            "failed",
            Some("Downloader exited successfully but no media files were produced"),
        ).await;
    } else {
        let _ = DownloadRequest::update_status(pool, &job.request_id, "completed", None).await;
        // Auto-link gallery to person if person exists
        if let Some(ref gid) = gallery_id {
            let _ = auto_link::auto_link_gallery(pool, &job.url, gid).await;
        }
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
