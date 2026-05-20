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
use queue::worker::JobSender;
use sqlx::SqlitePool;
use std::path::PathBuf;
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

    // Check for CLI flags that run before the worker starts.
    // These must return early to avoid conflicting with the running server.
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a.to_lowercase() == "reset") {
        info!("Reset flag detected: running full reset of all requests");
        reset_checker::run_reset_all(&pool, &config).await;
        return Ok(());
    }
    if args.iter().any(|a| a.to_lowercase() == "requeue-failed") {
        reset_checker::run_requeue_failed(&pool).await;
        return Ok(());
    } else if args.iter().any(|a| a.to_lowercase() == "requeue-all") {
        reset_checker::run_requeue_all(&pool, &config).await;
        reset_checker::redownload_stashdb_images(&pool, &config, &config.http_client).await;
        return Ok(());
    }

    // Start download queue worker
    let job_sender = queue::worker::spawn_worker(pool.clone(), config.clone());
    info!("Download queue worker started");

    // Recover unfinished jobs
    queue::worker::recover_pending_jobs(&pool, job_sender.clone()).await;

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
        .route("/api/requests/guess-title", get(handlers::requests::guess_request_title))
        .route("/api/galleries", get(handlers::galleries::list_galleries))
        .route("/api/galleries/retroactive-update", post(handlers::galleries::retroactive_update_titles))
        .route("/api/galleries/{id}", get(handlers::galleries::get_gallery))
        .route("/api/galleries/{id}", patch(handlers::galleries::update_gallery))
        .route("/api/galleries/{id}/persons", get(handlers::persons::get_gallery_persons))
        .route("/api/images", get(handlers::images::list_images))
        .route("/api/images/{id}/favorite", patch(handlers::images::toggle_favorite))
        .route("/api/videos", get(handlers::videos::list_videos))
        .route("/api/videos/{id}", patch(handlers::videos::update_video))
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
