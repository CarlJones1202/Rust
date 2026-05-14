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

    let abs_temp_str = abs_temp_dir.to_string_lossy().to_string();
    let abs_temp_str = abs_temp_str.strip_prefix(r"\\?\").unwrap_or(&abs_temp_str).to_string();

    let lower_url = url.to_lowercase();

    let use_yt_dlp = lower_url.contains("youtube.com") 
        || lower_url.contains("youtu.be")
        || lower_url.contains("tnaflix.com")
        || lower_url.contains("vimeo.com")
        || lower_url.contains("dailymotion.com")
        || lower_url.contains("tiktok.com");

    let mut cmd = if use_yt_dlp {
        info!(url = %url, "Using yt-dlp directly for video site");
        let mut c = Command::new("yt-dlp");
        
        // Output and location
        c.arg("-P").arg(&abs_temp_str);
        c.arg("-o").arg("%(title)s-%(id)s.%(ext)s");
        
        // YouTube-specific fixes
        c.arg("--js-runtimes").arg("node");
        c.arg("--remote-components").arg("ejs:github");
        
        // Quality
        c.arg("-f").arg("bestvideo+bestaudio/best");
        
        // Resuming
        let archive_path = abs_temp_dir.join("archive.txt");
        c.arg("--download-archive").arg(archive_path);

        // Printing filepath for our processor (video trigger ensures it prints even if skipped/already exists)
        c.arg("--print").arg("video:filepath");
        c.arg("--no-progress");
        
        c
    } else {
        // Detect if we should use ytdl prefix for other video sites (social/misc)
        let is_video_site = lower_url.contains("twitter.com")
            || lower_url.contains("x.com")
            || lower_url.contains("instagram.com");

        let final_url = if is_video_site && !url.starts_with("ytdl:") {
            format!("ytdl:{}", url)
        } else {
            url.to_string()
        };

        info!(url = final_url, dir = %abs_temp_str, "Running gallery-dl");
        let mut c = Command::new(gallery_dl_bin);
        c.arg("-d").arg(&abs_temp_str).arg("--no-mtime");

        // Forum-specific filters
        if url.contains("vipergirls.to") {
            c.arg("--chapter-filter").arg("post_num == '1'");
        }
        
        c.arg("-o").arg("cookies-update=false");
        c
    };

    if std::path::Path::new("cookies.txt").exists() {
        cmd.arg("--cookies").arg("cookies.txt");
    } else if let Some(browser) = cookies_from_browser {
        cmd.arg("--cookies-from-browser").arg(browser);
    }

    // Append the URL at the end
    if use_yt_dlp {
        cmd.arg(url);
    } else {
        let is_video_site = lower_url.contains("twitter.com")
            || lower_url.contains("x.com")
            || lower_url.contains("instagram.com");

        let final_url = if is_video_site && !url.starts_with("ytdl:") {
            format!("ytdl:{}", url)
        } else {
            url.to_string()
        };
        cmd.arg(final_url);
    }

    debug!("Exec: {:?}", cmd);

    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn downloader: {e}"))?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // Log stderr (use info! for yt-dlp to see challenge solving)
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if use_yt_dlp {
                info!(stderr = %line, "yt-dlp");
            } else {
                warn!(stderr = %line, "gallery-dl");
            }
        }
    });

    let mut reader = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = reader.next_line().await {
        let line = line.trim();
        if line.is_empty() || line == "NA" {
            continue;
        }

        info!(line = %line, "Downloader stdout");

        let p = PathBuf::from(line);
        let p = if p.is_absolute() {
            p
        } else {
            abs_temp_dir.join(p)
        };

        if p.exists() {
            let _ = tx.send(p);
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait on downloader: {e}"))?;

    if !status.success() {
        let code = status.code().unwrap_or(-1);
        error!(code = code, "downloader exited with non-zero code");
        return Err(format!("downloader exited with code {code}"));
    }

    Ok(())
}
