mod config;
mod db;
mod handlers;
mod models;
mod pagination;
mod queue;
mod services;

use axum::{routing::get, routing::post, Router};
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
                .unwrap_or_else(|_| "gallery_dl_api=info,tower_http=info".into()),
        )
        .init();

    // Load config
    let config = config::Config::from_env();
    info!(?config, "Loaded configuration");

    // Ensure storage directories exist
    let storage_dir = PathBuf::from(&config.storage_dir);
    tokio::fs::create_dir_all(storage_dir.join("images")).await?;
    tokio::fs::create_dir_all(storage_dir.join("videos")).await?;
    tokio::fs::create_dir_all(storage_dir.join("temp")).await?;
    info!(dir = %config.storage_dir, "Storage directories ready");

    // Initialize database
    let pool = db::init_pool(&config.database_url).await?;

    // Start download queue worker
    let job_sender = queue::worker::spawn_worker(pool.clone(), config.clone());
    info!("Download queue worker started");

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
        .route("/api/galleries", get(handlers::galleries::list_galleries))
        .route("/api/galleries/{id}", get(handlers::galleries::get_gallery))
        .route("/api/images", get(handlers::images::list_images))
        .route("/api/videos", get(handlers::videos::list_videos))
        // Static file serving for media
        .nest_service(
            "/media/images",
            ServeDir::new(storage_dir.join("images")),
        )
        .nest_service(
            "/media/videos",
            ServeDir::new(storage_dir.join("videos")),
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
