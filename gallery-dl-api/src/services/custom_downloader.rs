use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::info;
use regex::Regex;

/// Check if a URL is for a site we handle with a custom downloader.
/// Returns the site name if recognized, None otherwise.
pub fn recognize_site(url: &str) -> Option<&'static str> {
    let lower = url.to_lowercase();
    if lower.contains("pmvhaven.com") {
        Some("pmvhaven")
    } else {
        None
    }
}

/// Run a custom downloader for a URL that gallery-dl/yt-dlp don't support.
pub async fn run_custom_downloader(
    http_client: &reqwest::Client,
    url: &str,
    site: &str,
    temp_dir: &Path,
    tx: mpsc::UnboundedSender<PathBuf>,
) -> Result<(), String> {
    tokio::fs::create_dir_all(temp_dir)
        .await
        .map_err(|e| format!("Failed to create temp dir: {e}"))?;

    match site {
        "pmvhaven" => download_pmvhaven(http_client, url, temp_dir, tx).await,
        _ => Err(format!("Unknown custom site: {site}")),
    }
}

/// Download a video from PMVHaven using its internal API,
/// selecting the highest quality variant available.
async fn download_pmvhaven(
    http_client: &reqwest::Client,
    url: &str,
    temp_dir: &Path,
    tx: mpsc::UnboundedSender<PathBuf>,
) -> Result<(), String> {
    let video_id = extract_pmvhaven_id(url)
        .ok_or_else(|| format!("Could not extract video ID from PMVHaven URL: {url}"))?;

    info!(video_id = %video_id, "Extracted PMVHaven video ID");

    // Fetch video metadata from the internal API
    let api_url = format!("https://pmvhaven.com/api/videos/{video_id}/watch-page");
    info!(api_url = %api_url, "Fetching PMVHaven API");

    let resp = http_client
        .get(&api_url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
        .header("Referer", "https://pmvhaven.com/")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch PMVHaven API: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("PMVHaven API returned status {}", resp.status()));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse PMVHaven API response: {e}"))?;

    // Pick the highest quality video URL
    let video_url = pick_best_video_url(&body)?;
    info!(video_url = %video_url, "Selected highest quality video URL");

    // Extract metadata for filename
    let title = body
        .pointer("/data/video/title")
        .and_then(|v| v.as_str())
        .unwrap_or("video");

    let uploader = body
        .pointer("/data/video/uploader")
        .and_then(|v| v.as_str())
        .or_else(|| {
            body.pointer("/data/video/creator/0")
                .and_then(|v| v.as_str())
        })
        .unwrap_or("Unknown");

    // Sanitize title for filename
    let clean_title: String = title
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let clean_title = clean_title.trim();

    // If it's an HLS stream, use ffmpeg to download and convert to mp4
    if video_url.contains(".m3u8") {
        download_hls_with_ffmpeg(&video_url, temp_dir, &video_id, &uploader, &clean_title, &tx).await
    } else {
        download_direct_url(http_client, &video_url, temp_dir, &video_id, &uploader, &clean_title, &tx).await
    }
}

/// Pick the highest quality video URL from the API response.
/// Checks hlsVariants first (sorted by bitrate desc), falls back to videoUrl.
fn pick_best_video_url(body: &serde_json::Value) -> Result<String, String> {
    // Try hlsVariants first — these contain different quality levels
    if let Some(variants) = body.pointer("/data/video/hlsVariants").and_then(|v| v.as_array()) {
        if !variants.is_empty() {
            let best = variants
                .iter()
                .filter_map(|v| {
                    let url = v.get("url").and_then(|u| u.as_str())?;
                    let bitrate = v.get("bitrate").and_then(|b| b.as_i64()).unwrap_or(0);
                    Some((url.to_string(), bitrate))
                })
                .max_by_key(|(_, bitrate)| *bitrate);

            if let Some((url, bitrate)) = best {
                info!(bitrate = bitrate, "Selected HLS variant with highest bitrate");
                return Ok(url);
            }
        }
    }

    // Fall back to the default videoUrl
    body.pointer("/data/video/videoUrl")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "PMVHaven API response missing both hlsVariants and videoUrl".to_string())
}

/// Download an HLS stream using ffmpeg, which handles m3u8 natively and outputs mp4.
async fn download_hls_with_ffmpeg(
    video_url: &str,
    temp_dir: &Path,
    video_id: &str,
    uploader: &str,
    clean_title: &str,
    tx: &mpsc::UnboundedSender<PathBuf>,
) -> Result<(), String> {
    // Output directly as mp4 — ffmpeg will remux the HLS stream
    let filename = format!("{uploader} - {video_id} - {clean_title}.mp4");
    let dest_path = temp_dir.join(&filename);

    info!(dest = %filename, "Downloading HLS stream via ffmpeg");

    let status = Command::new("ffmpeg")
        .arg("-i")
        .arg(video_url)
        .arg("-c")
        .arg("copy")
        .arg("-movflags")
        .arg("+faststart")
        .arg("-y")
        .arg(&dest_path)
        .status()
        .await
        .map_err(|e| format!("Failed to run ffmpeg: {e}"))?;

    if !status.success() {
        return Err(format!("ffmpeg exited with status {status}"));
    }

    let file_size = std::fs::metadata(&dest_path)
        .map(|m| m.len())
        .unwrap_or(0);

    if file_size == 0 {
        return Err("ffmpeg produced an empty output file".to_string());
    }

    info!(size = file_size, path = %dest_path.display(), "HLS download complete");

    let _ = tx.send(dest_path);
    Ok(())
}

/// Download a direct video URL via reqwest.
async fn download_direct_url(
    http_client: &reqwest::Client,
    video_url: &str,
    temp_dir: &Path,
    video_id: &str,
    uploader: &str,
    clean_title: &str,
    tx: &mpsc::UnboundedSender<PathBuf>,
) -> Result<(), String> {
    // Determine extension from the video URL
    let ext = video_url
        .rsplit('.')
        .next()
        .and_then(|s| {
            let s = s.split('?').next().unwrap_or(s);
            if s.len() <= 5 { Some(s.to_string()) } else { None }
        })
        .unwrap_or_else(|| "mp4".to_string());

    let filename = format!("{uploader} - {video_id} - {clean_title}.{ext}");
    let dest_path = temp_dir.join(&filename);

    info!(video_url = %video_url, dest = %filename, "Downloading direct video URL");

    let mut resp = http_client
        .get(video_url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
        .header("Referer", "https://pmvhaven.com/")
        .send()
        .await
        .map_err(|e| format!("Failed to start video download: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Video download returned status {}", resp.status()));
    }

    // Stream the response to disk instead of loading into memory
    let mut file = tokio::fs::File::create(&dest_path)
        .await
        .map_err(|e| format!("Failed to create output file: {e}"))?;

    let mut bytes_written: u64 = 0;
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("Failed to read video chunk: {e}"))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Failed to write video chunk: {e}"))?;
        bytes_written += chunk.len() as u64;
    }
    file.flush()
        .await
        .map_err(|e| format!("Failed to flush video file: {e}"))?;

    info!(size = bytes_written, path = %dest_path.display(), "Direct download complete");

    let _ = tx.send(dest_path);
    Ok(())
}

/// Extract the 24-character hex video ID from a PMVHaven URL.
/// URL format: https://pmvhaven.com/video/{title}_{24hexId}
fn extract_pmvhaven_id(url: &str) -> Option<String> {
    let re = Regex::new(r"_([a-fA-F0-9]{24})").ok()?;
    re.captures(url)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_lowercase()))
}
