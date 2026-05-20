use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Run gallery-dl for a given URL, downloading files to `temp_dir`.
/// Run gallery-dl for a given URL, downloading files to `temp_dir`.
/// Sends downloaded file paths sequentially via the provided channel.
fn build_cmd(
    base_program: &str,
    python_module: Option<&str>,
    use_yt_dlp: bool,
    abs_temp_str: &str,
    url: &str,
    lower_url: &str,
    download_delay: f64,
    cookies_from_browser: Option<&str>,
) -> Command {
    let mut cmd = Command::new(base_program);
    if let Some(module) = python_module {
        cmd.arg("-m").arg(module);
    }

    if use_yt_dlp {
        // Output and location
        cmd.arg("-P").arg(abs_temp_str);
        cmd.arg("-o").arg("%(title)s-%(id)s.%(ext)s");
        
        // YouTube-specific fixes
        cmd.arg("--js-runtimes").arg("node");
        cmd.arg("--remote-components").arg("ejs:github");
        
        // Quality
        cmd.arg("-f").arg("bestvideo+bestaudio/best");
        
        // Resuming
        let archive_path = Path::new(abs_temp_str).join("archive.txt");
        cmd.arg("--download-archive").arg(archive_path);

        // Print the final file path after download+merge so we can process it.
        // --no-simulate is required because --print alone suppresses the download.
        cmd.arg("--no-simulate");
        cmd.arg("--print").arg("after_move:filepath");
        cmd.arg("--no-progress");
    } else {
        cmd.arg("-d").arg(abs_temp_str).arg("--no-mtime");

        // Forum-specific filters
        if url.contains("vipergirls.to") {
            cmd.arg("--chapter-filter").arg("post_num == '1'");
        }

        cmd.arg("-o").arg("cookies-update=false");

        // Mimic a real browser to avoid 503 from CDNs / hotlink protection
        cmd.arg("--user-agent")
            .arg("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36");

        // Add Referer header for known image hosts that check it
        if lower_url.contains("imx.to") || lower_url.contains("imagebam.com") || lower_url.contains("pixhost.to") {
            cmd.arg("-o").arg("headers.Referer=https://imx.to/");
        }

        // Add delay between requests to avoid rate limiting
        if download_delay > 0.0 {
            cmd.arg("--sleep").arg(download_delay.to_string());
            cmd.arg("--sleep-request").arg(download_delay.to_string());
        }

        // Increase retries and don't abort on a few individual file failures
        cmd.arg("--retries").arg("10");
        cmd.arg("--abort").arg("10");
    }

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

    cmd
}

/// Run gallery-dl for a given URL, downloading files to `temp_dir`.
/// Run gallery-dl for a given URL, downloading files to `temp_dir`.
/// Sends downloaded file paths sequentially via the provided channel.
pub async fn run_gallery_dl(
    gallery_dl_bin: &str,
    url: &str,
    temp_dir: &Path,
    download_delay: f64,
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

    // Delete stale download archive so retries don't get silently skipped
    if use_yt_dlp {
        let stale = abs_temp_dir.join("archive.txt");
        let _ = tokio::fs::remove_file(&stale).await;
    }

    let mut cmd = build_cmd(
        if use_yt_dlp { "yt-dlp" } else { gallery_dl_bin },
        None,
        use_yt_dlp,
        &abs_temp_str,
        url,
        &lower_url,
        download_delay,
        cookies_from_browser,
    );

    debug!("Exec: {:?}", cmd);

    let mut child = match cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Try fallback using python -m
            let fallback_module = if use_yt_dlp { "yt_dlp" } else { "gallery_dl" };
            info!("Binary not found in PATH, trying fallback: python -m {}", fallback_module);
            let mut fallback_cmd = build_cmd(
                "python",
                Some(fallback_module),
                use_yt_dlp,
                &abs_temp_str,
                url,
                &lower_url,
                download_delay,
                cookies_from_browser,
            );
            debug!("Exec fallback: {:?}", fallback_cmd);
            fallback_cmd
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|err| format!("Failed to spawn fallback downloader (python -m {}): {}", fallback_module, err))?
        }
        Err(e) => return Err(format!("Failed to spawn downloader: {e}")),
    };

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
    let mut sent_paths: HashSet<PathBuf> = HashSet::new();

    // Send each file path immediately as it's reported by the downloader,
    // so the worker can process/move/insert it without waiting for all downloads to finish.
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

        let canon = p.canonicalize().unwrap_or_else(|_| p.clone());
        if sent_paths.insert(canon) && p.exists() {
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

    // Scan the temp directory as a safety net for any files that weren't
    // properly reported via stdout (e.g. Python fallback path mismatches)
    // or didn't exist on disk yet when the stdout line was processed.
    if let Ok(mut entries) = tokio::fs::read_dir(&abs_temp_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default();
            // Skip yt-dlp metadata, partial downloads, and the archive
            if name.ends_with(".part") || name.ends_with(".ytdl") || name == "archive.txt" {
                continue;
            }
            let canon = path.canonicalize().unwrap_or_else(|_| path.clone());
            if sent_paths.insert(canon) {
                let _ = tx.send(path);
            }
        }
    }

    if sent_paths.is_empty() {
        let msg = "downloader produced no output files".to_string();
        warn!("{}", msg);
        return Err(msg);
    }

    Ok(())
}
