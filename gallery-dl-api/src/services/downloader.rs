use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Run gallery-dl for a given URL, downloading files to `temp_dir`.
/// Run gallery-dl for a given URL, downloading files to `temp_dir`.
/// Sends downloaded file paths sequentially via the provided channel.
pub async fn run_gallery_dl(
    gallery_dl_bin: &str,
    url: &str,
    temp_dir: &Path,
    cookies_from_browser: Option<&str>,
    tx: mpsc::UnboundedSender<PathBuf>,
) -> Result<(), String> {
    // Ensure temp directory exists
    tokio::fs::create_dir_all(temp_dir)
        .await
        .map_err(|e| format!("Failed to create temp dir: {e}"))?;

    // Convert to absolute normalized path
    let abs_temp_dir = std::fs::canonicalize(temp_dir)
        .map_err(|e| format!("Failed to canonicalize temp dir: {e}"))?;

    // On Windows, canonicalize returns UNC paths (\\?\C:\...). 
    // Strip the prefix to ensure compatibility with gallery-dl.
    let abs_temp_str = abs_temp_dir.to_string_lossy().to_string();
    let abs_temp_str = abs_temp_str.strip_prefix(r"\\?\").unwrap_or(&abs_temp_str).to_string();

    // Detect if we should use ytdl prefix
    let lower_url = url.to_lowercase();
    let is_video_site = lower_url.contains("youtube.com") 
        || lower_url.contains("youtu.be")
        || lower_url.contains("vimeo.com")
        || lower_url.contains("dailymotion.com")
        || lower_url.contains("tiktok.com")
        || lower_url.contains("twitter.com")
        || lower_url.contains("x.com")
        || lower_url.contains("instagram.com")
        || lower_url.contains("tnaflix.com");

    let final_url = if is_video_site && !url.starts_with("ytdl:") {
        format!("ytdl:{}", url)
    } else {
        url.to_string()
    };

    info!(url = final_url, dir = %abs_temp_str, "Running gallery-dl");

    let mut cmd = Command::new(gallery_dl_bin);
    cmd.arg("-d")
        .arg(&abs_temp_str)
        .arg("--no-mtime");

    cmd.arg("-o")
        .arg("ytdl-args=[\"--js-runtimes\", \"node\", \"--remote-components\", \"ejs:github\"]");

    // Use cookies.txt if it exists, otherwise fall back to browser extraction if configured
    if std::path::Path::new("cookies.txt").exists() {
        cmd.arg("--cookies").arg("cookies.txt");
    } else if let Some(browser) = cookies_from_browser {
        cmd.arg("--cookies-from-browser").arg(browser);
    }

    // Add download archive to support efficient resuming
    let archive_path = abs_temp_dir.join("archive.txt");
    let archive_str = archive_path.to_string_lossy().to_string();
    let archive_str = archive_str.strip_prefix(r"\\?\").unwrap_or(&archive_str).to_string();
    cmd.arg("--download-archive")
        .arg(archive_str);

    // Forum-specific filters
    if url.contains("vipergirls.to") {
        cmd.arg("--chapter-filter")
            .arg("post_num == '1'");
    }

    cmd.arg(final_url);

    debug!("Exec: {:?}", cmd);

    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn gallery-dl: {e}"))?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // Spawn a task to log stderr so we don't block
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            warn!(stderr = %line, "gallery-dl stderr");
        }
    });

    let mut reader = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = reader.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let p = PathBuf::from(line);
        let p = if p.is_absolute() {
            p
        } else {
            abs_temp_dir.join(p)
        };

        if p.exists() {
            // Ignore send errors in case the receiver dropped
            let _ = tx.send(p);
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait on gallery-dl: {e}"))?;

    if !status.success() {
        let code = status.code().unwrap_or(-1);
        error!(code = code, "gallery-dl exited with non-zero code");
        return Err(format!("gallery-dl exited with code {code}"));
    }

    Ok(())
}
