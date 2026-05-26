mod config;
mod db;
mod handlers;
mod models;
mod pagination;
mod queue;
mod reset_checker;
mod services;

use axum::{routing::get, routing::post, routing::patch, routing::delete, Router};
use axum::http::header;
use queue::worker::{DownloadJob, JobSender};
use sqlx::SqlitePool;
use std::path::PathBuf;
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

    // On startup, probe hosts that have "processing" jobs.
    // If a host is unreachable, mark it down and reset its jobs to "pending"
    // so they don't get re-queued until the host recovers.
    {
        let http_client = &config.http_client;
        let extract_host = services::site_checker::extract_host;
        match models::request::DownloadRequest::list_processing(&pool).await {
            Ok(processing_jobs) if !processing_jobs.is_empty() => {
                let mut hosts: Vec<String> = processing_jobs.iter()
                    .filter_map(|r| extract_host(&r.url))
                    .collect();
                hosts.sort();
                hosts.dedup();
                info!(host_count = hosts.len(), "Probing hosts from processing jobs on startup");
                for host in &hosts {
                    let url = format!("https://{}", host);
                    match http_client.get(&url).send().await {
                        Ok(resp) if resp.status().is_success() => {
                            info!(host = %host, "Host is up on startup");
                        }
                        _ => {
                            info!(host = %host, "Host is down on startup, resetting processing jobs to pending");
                            site_health.mark_down(host).await;
                            for req in &processing_jobs {
                                if extract_host(&req.url).as_deref() == Some(host.as_str()) {
                                    let _ = models::request::DownloadRequest::update_status(
                                        &pool, &req.id, "pending", None,
                                    ).await;
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Start download queue worker with site health tracking
    let job_sender = queue::worker::spawn_worker(pool.clone(), config.clone(), site_health.clone());
    info!("Download queue worker started");

    // Channel for host-recovery notifications from the periodic checker
    let (host_up_tx, mut host_up_rx) = mpsc::unbounded_channel::<String>();

    // Listen for recovered hosts and re-queue their unfinished jobs
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
                        if let Some(req_host) = extract_host(&req.url) {
                            if req_host == host {
                                let _ = recovery_sender.send(DownloadJob {
                                    request_id: req.id.clone(),
                                    url: req.url.clone(),
                                    title: req.title.clone(),
                                });
                                count += 1;
                            }
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
    };

    // Build router
    let app = Router::new()
        // API routes
        .route("/api/requests", post(handlers::requests::create_request))
        .route("/api/requests", get(handlers::requests::list_requests))
        .route("/api/requests/{id}", get(handlers::requests::get_request))
        .route("/api/requests/{id}/requeue", post(handlers::requests::requeue_request))
        .route("/api/requests/nuke", post(handlers::requests::nuke_all))
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
