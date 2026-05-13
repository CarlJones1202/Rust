use std::env;

/// Application configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub storage_dir: String,
    pub database_url: String,
    pub max_concurrent_downloads: usize,
    pub gallery_dl_bin: String,
}

impl Config {
    /// Load configuration from environment variables (with defaults).
    pub fn from_env() -> Self {
        Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            storage_dir: env::var("STORAGE_DIR").unwrap_or_else(|_| "./storage".to_string()),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:gallery_dl.db?mode=rwc".to_string()),
            max_concurrent_downloads: env::var("MAX_CONCURRENT_DOWNLOADS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            gallery_dl_bin: env::var("GALLERY_DL_BIN")
                .unwrap_or_else(|_| "gallery-dl".to_string()),
        }
    }
}
