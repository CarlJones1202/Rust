use std::env;
use std::fmt;

/// Application configuration loaded from environment variables.
pub struct Config {
    pub host: String,
    pub port: u16,
    pub storage_dir: String,
    pub database_url: String,
    pub max_concurrent_downloads: usize,
    pub max_concurrent_video_downloads: usize,
    pub gallery_dl_bin: String,
    pub download_delay: f64,
    pub cookies_from_browser: Option<String>,
    pub stashdb_api_key: Option<String>,
    pub http_client: reqwest::Client,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("storage_dir", &self.storage_dir)
            .field("database_url", &self.database_url)
            .field("max_concurrent_downloads", &self.max_concurrent_downloads)
            .field("max_concurrent_video_downloads", &self.max_concurrent_video_downloads)
            .field("gallery_dl_bin", &self.gallery_dl_bin)
            .field("download_delay", &self.download_delay)
            .field("cookies_from_browser", &self.cookies_from_browser)
            .field("stashdb_api_key", &self.stashdb_api_key)
            .finish()
    }
}

impl Clone for Config {
    fn clone(&self) -> Self {
        Self {
            host: self.host.clone(),
            port: self.port,
            storage_dir: self.storage_dir.clone(),
            database_url: self.database_url.clone(),
            max_concurrent_downloads: self.max_concurrent_downloads,
            max_concurrent_video_downloads: self.max_concurrent_video_downloads,
            gallery_dl_bin: self.gallery_dl_bin.clone(),
            download_delay: self.download_delay,
            cookies_from_browser: self.cookies_from_browser.clone(),
            stashdb_api_key: self.stashdb_api_key.clone(),
            http_client: self.http_client.clone(),
        }
    }
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
            max_concurrent_video_downloads: env::var("MAX_CONCURRENT_VIDEO_DOWNLOADS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2),
            gallery_dl_bin: env::var("GALLERY_DL_BIN")
                .unwrap_or_else(|_| "gallery-dl".to_string()),
            download_delay: env::var("DOWNLOAD_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
            cookies_from_browser: env::var("COOKIES_FROM_BROWSER").ok(),
            stashdb_api_key: env::var("STASHDB_API_KEY").ok().filter(|s| !s.is_empty() && s != "your_api_key_here"),
            http_client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
                .build()
                .expect("Failed to build HTTP client"),
        }
    }
}
