mod config;
mod db;
mod handlers;
mod models;
mod pagination;
mod queue;
mod reset_checker;
mod services;

use axum::{routing::get, routing::post, routing::put, routing::patch, routing::delete, Router};

use axum::http::header;
use queue::worker::{ConcurrencyController, DownloadJob, JobSender};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing::info;

/// Shared application state available to all handlers.
#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub job_sender: JobSender,
    pub config: config::Config,
    pub site_health: services::site_checker::SiteHealth,
    pub image_concurrency: Arc<ConcurrencyController>,
    pub video_concurrency: Arc<ConcurrencyController>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "gallery_dl_api=info,tower_http=warn,sqlx=warn".into()),
        )
        .init();

    // Load config
    let config = config::Config::from_env();
    info!(?config, "Loaded configuration");

    // Ensure storage directories exist
    let storage_dir = PathBuf::from(&config.storage_dir);
    tokio::fs::create_dir_all(storage_dir.join("images")).await?;
    tokio::fs::create_dir_all(storage_dir.join("videos")).await?;
    tokio::fs::create_dir_all(storage_dir.join("thumbnails")).await?;
    tokio::fs::create_dir_all(storage_dir.join("temp")).await?;
    tokio::fs::create_dir_all(storage_dir.join("trickplay")).await?;
    tokio::fs::create_dir_all(storage_dir.join("persons")).await?;
    info!(dir = %config.storage_dir, "Storage directories ready");

    // Initialize database
    let pool = db::init_pool(&config.database_url).await?;

    // Check for CLI flags. `reset` and `requeue-failed` used to run and
    // exit before the server started; now they all spawn background work
    // and let the server come up normally.
    let args: Vec<String> = std::env::args().collect();
    // Special developer/testing command: `test-download <url>`
    // Runs the custom downloader for a single URL and exits.
    if args.len() >= 2 && args[1].to_lowercase() == "test-download" {
        if args.len() < 3 {
            eprintln!("Usage: test-download <url>");
            return Ok(());
        }
        let url = &args[2];
        let site = if crate::services::custom_downloader::recognize_site(url).is_some() {
            crate::services::custom_downloader::recognize_site(url).unwrap()
        } else {
            eprintln!("URL not recognized as a custom site");
            return Ok(());
        };

        let client = reqwest::Client::builder()
            .user_agent(crate::services::custom_downloader::BROWSER_UA)
            .http1_only()
            .build()
            .expect("failed to build client");

        let temp_dir = std::path::Path::new("./tmp_test_kitty");
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        println!("Running custom downloader for site {} and url {}", site, url);
        match crate::services::custom_downloader::run_custom_downloader(&client, url, site, temp_dir, tx).await {
            Ok(()) => println!("Custom downloader completed successfully"),
            Err(e) => println!("Custom downloader failed: {}", e),
        }
        return Ok(());
    }
    // Developer helper: reprocess files in ./tmp_test_kitty using the normal
    // file processing pipeline. Useful to reproduce thumbnail generation errors.
    if args.len() >= 2 && args[1].to_lowercase() == "reprocess-tmp" {
        let tmp_dir = std::path::Path::new("./tmp_test_kitty");
        let storage_dir = std::path::Path::new("./storage");
        if !tmp_dir.exists() {
            eprintln!("tmp_test_kitty not found");
            return Ok(());
        }

        let mut entries = tokio::fs::read_dir(tmp_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                println!("Processing {}", path.display());
                match crate::services::file_processor::process_single_file(&path, &storage_dir).await {
                    Ok(Some(p)) => println!("Processed: {} type={:?}", p.hash, p.media_type),
                    Ok(None) => println!("Skipped unknown type: {}", path.display()),
                    Err(e) => eprintln!("Error processing {}: {}", path.display(), e),
                }
            }
        }
        return Ok(());
    }
    if args.iter().any(|a| a.to_lowercase() == "reset") {
        info!("Reset flag detected: spawning full reset in background, server will start normally");
        let bg_pool = pool.clone();
        let bg_config = config.clone();
        tokio::spawn(async move {
            reset_checker::run_reset_all(&bg_pool, &bg_config).await;
        });
    }
    if args.iter().any(|a| a.to_lowercase() == "requeue-failed") {
        info!("Requeue-failed flag detected: spawning requeue in background, server will start normally");
        let bg_pool = pool.clone();
        tokio::spawn(async move {
            reset_checker::run_requeue_failed(&bg_pool).await;
        });
    }
    if args.iter().any(|a| a.to_lowercase() == "requeue-all") {
        info!("Requeue-all flag detected: spawning background purge/redownload, server will start normally");
        let bg_pool = pool.clone();
        let bg_config = config.clone();
        tokio::spawn(async move {
            reset_checker::run_requeue_all(&bg_pool, &bg_config).await;
        });
        let bg_pool2 = pool.clone();
        let bg_config2 = config.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                reset_checker::redownload_stashdb_images(&bg_pool2, &bg_config2, &bg_config2.http_client).await;
            });
        });
    }

    // Track which hosts are known to be down
    let site_health = services::site_checker::SiteHealth::new();

    // On startup, probe hosts from all unfinished jobs (pending + processing).
    // If a host is unreachable, mark it down and reset its processing jobs to
    // "pending" so they don't get re-queued until the host recovers.
    // Backup URL hosts are also probed.
    {
        let http_client = &config.http_client;
        let extract_host = services::site_checker::extract_host;
        let all_unfinished = models::request::DownloadRequest::list_unfinished(&pool).await.unwrap_or_default();
        if !all_unfinished.is_empty() {
            let mut hosts: Vec<String> = all_unfinished.iter()
                .filter_map(|r| extract_host(&r.url))
                .chain(
                    all_unfinished.iter().filter_map(|r| {
                        r.backup_url.as_ref().and_then(|u| extract_host(u))
                    })
                )
                .collect();
            hosts.sort();
            hosts.dedup();
            info!(host_count = hosts.len(), "Probing hosts from unfinished jobs on startup");
            for host in &hosts {
                if services::site_checker::probe_host(http_client, host).await {
                    info!(host = %host, "Host is up on startup");
                    site_health.mark_up(host).await;
                } else {
                    info!(host = %host, "Host is down on startup, marking down and resetting processing jobs");
                    site_health.mark_down(host).await;
                    for req in &all_unfinished {
                        if req.status == "processing" {
                            let primary_match = extract_host(&req.url).as_deref() == Some(host.as_str());
                            let backup_match = req.backup_url.as_ref()
                                .and_then(|u| extract_host(u))
                                .as_deref() == Some(host.as_str());
                            if primary_match || backup_match {
                                let _ = models::request::DownloadRequest::update_status(
                                    &pool, &req.id, "pending", None,
                                ).await;
                            }
                        }
                    }
                }
            }
        }
    }

    // Runtime-adjustable concurrency limits for image and video downloads
    let image_concurrency = Arc::new(ConcurrencyController::new(config.max_concurrent_downloads));
    let video_concurrency = Arc::new(ConcurrencyController::new(config.max_concurrent_video_downloads));

    // Start download queue worker with site health tracking
    let job_sender = queue::worker::spawn_worker(
        pool.clone(),
        config.clone(),
        site_health.clone(),
        config.http_client.clone(),
        image_concurrency.clone(),
        video_concurrency.clone(),
    );
    info!("Download queue worker started");

    // Channel for host-recovery notifications from the periodic checker
    let (host_up_tx, mut host_up_rx) = mpsc::unbounded_channel::<String>();

    // Listen for recovered hosts and re-queue their unfinished jobs.
    // Matches both primary URLs and backup URLs belonging to the recovered host.
    let recovery_pool = pool.clone();
    let recovery_sender = job_sender.clone();
    tokio::spawn(async move {
        use crate::services::site_checker::extract_host;
        while let Some(host) = host_up_rx.recv().await {
            info!(host = %host, "Host recovered, re-queueing unfinished jobs");
            match crate::models::request::DownloadRequest::list_unfinished(&recovery_pool).await {
                Ok(requests) => {
                    let mut count = 0u32;
                    for req in &requests {
                        let primary_match = extract_host(&req.url)
                            .map(|h| h == host)
                            .unwrap_or(false);
                        let backup_match = req.backup_url.as_ref()
                            .and_then(|u| extract_host(u))
                            .map(|h| h == host)
                            .unwrap_or(false);

                        if primary_match || backup_match {
                            // Use primary URL if its host is the recovered one,
                            // otherwise use backup URL (primary host may still be down)
                            let effective_url = if primary_match {
                                req.url.clone()
                            } else if let Some(ref bu) = req.backup_url {
                                bu.clone()
                            } else {
                                req.url.clone()
                            };
                            let _ = recovery_sender.send(DownloadJob {
                                request_id: req.id.clone(),
                                url: effective_url,
                                title: req.title.clone(),
                                backup_url: req.backup_url.clone(),
                            });
                            count += 1;
                        }
                    }
                    if count > 0 {
                        info!(host = %host, count = count, "Re-queued jobs for recovered host");
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to list unfinished jobs for host recovery");
                }
            }
        }
    });

    // Spawn background checker for down hosts
    services::site_checker::spawn_periodic_checker(site_health.clone(), config.http_client.clone(), host_up_tx);

    // Recover unfinished jobs (skips hosts already marked down)
    queue::worker::recover_pending_jobs(&pool, job_sender.clone(), &site_health).await;

    // Build application state
    let state = AppState {
        db: pool,
        job_sender,
        config: config.clone(),
        site_health,
        image_concurrency,
        video_concurrency,
    };

    // Build router
    let app = Router::new()
        // API routes
        .route("/api/requests", post(handlers::requests::create_request))
        .route("/api/requests", get(handlers::requests::list_requests))
        .route("/api/requests/{id}", get(handlers::requests::get_request))
        .route("/api/requests/{id}/requeue", post(handlers::requests::requeue_request))
        .route("/api/requests/nuke", post(handlers::requests::nuke_all))
        .route("/api/requests/{id}", patch(handlers::requests::update_request))
        .route("/api/requests/{id}", delete(handlers::requests::delete_request))
        .route("/api/requests/guess-title", get(handlers::requests::guess_request_title))
        .route("/api/galleries", get(handlers::galleries::list_galleries))
        .route("/api/galleries/retroactive-update", post(handlers::galleries::retroactive_update_titles))
        .route("/api/galleries/{id}", get(handlers::galleries::get_gallery))
        .route("/api/galleries/{id}", patch(handlers::galleries::update_gallery))
        .route("/api/galleries/{id}", delete(handlers::galleries::delete_gallery))
        .route("/api/galleries/{id}/persons", get(handlers::persons::get_gallery_persons))
        .route("/api/images", get(handlers::images::list_images))
        .route("/api/images/{id}", delete(handlers::images::delete_image))
        .route("/api/images/{id}/favorite", patch(handlers::images::toggle_favorite))
        .route("/api/videos", get(handlers::videos::list_videos))
        .route("/api/videos/{id}", patch(handlers::videos::update_video))
        .route("/api/videos/{id}", delete(handlers::videos::delete_video))
        .route("/api/videos/{id}/progress", get(handlers::videos::get_video_progress))
        .route("/api/videos/{id}/progress", post(handlers::videos::save_video_progress))
        // Person routes
        .route("/api/persons", post(handlers::persons::create_person))
        .route("/api/persons", get(handlers::persons::list_persons))
        .route("/api/persons/{id}", get(handlers::persons::get_person))
        .route("/api/persons/{id}", patch(handlers::persons::update_person))
        .route("/api/persons/{id}", delete(handlers::persons::delete_person))
        .route("/api/persons/{id}/images", post(handlers::persons::upload_person_image))
        .route("/api/persons/{id}/images/{image_id}", delete(handlers::persons::delete_person_image))
        .route("/api/persons/{id}/images/{image_id}/primary", patch(handlers::persons::set_primary_image))
        .route("/api/persons/{id}/galleries/{gallery_id}", post(handlers::persons::link_gallery))
        .route("/api/persons/{id}/galleries/{gallery_id}", delete(handlers::persons::unlink_gallery))
        .route("/api/persons/{id}/galleries", get(handlers::persons::get_person_galleries))
                .route("/api/persons/{id}/relink", post(handlers::persons::relink_person))
        .route("/api/persons/{id}/stashdb-import", post(handlers::persons::import_from_stashdb))
        .route("/api/stashdb/search", get(handlers::persons::search_stashdb))
        // Admin / utility routes
        .route("/api/admin/hosts", get(handlers::admin::list_hosts))
        .route("/api/admin/hosts/{host}/check", post(handlers::admin::check_host))
        .route("/api/admin/hosts/{host}/mark-down", post(handlers::admin::mark_host_down))
        .route("/api/admin/regather-stashdb", post(handlers::admin::regather_stashdb))
        .route("/api/admin/config", get(handlers::admin::get_config))
        .route("/api/admin/config", put(handlers::admin::update_config))
        // Static file serving for media
        .nest_service(
            "/media/images",
            ServeDir::new(storage_dir.join("images")),
        )
        .nest_service(
            "/media/videos",
            ServeDir::new(storage_dir.join("videos")),
        )
        .nest_service(
            "/media/thumbnails",
            ServeDir::new(storage_dir.join("thumbnails")),
        )
        .nest_service(
            "/media/trickplay",
            ServeDir::new(storage_dir.join("trickplay")),
        )
        .nest_service(
            "/media/persons",
            ServeDir::new(storage_dir.join("persons")),
        )
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(vec![header::CONTENT_TYPE]),
        );

    // Start server
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(addr = %addr, "Server listening");

    axum::serve(listener, app).await?;

    Ok(())
}
