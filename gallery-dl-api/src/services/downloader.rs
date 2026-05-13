use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, error, info};

/// Result of a gallery-dl download.
pub struct DownloadResult {
    pub files: Vec<PathBuf>,
}

/// Run gallery-dl for a given URL, downloading files to `temp_dir`.
/// Returns a list of file paths that were downloaded.
pub async fn run_gallery_dl(
    gallery_dl_bin: &str,
    url: &str,
    temp_dir: &Path,
) -> Result<DownloadResult, String> {
    info!(url = url, dir = ?temp_dir, "Starting gallery-dl download");

    // Ensure temp directory exists
    tokio::fs::create_dir_all(temp_dir)
        .await
        .map_err(|e| format!("Failed to create temp dir: {e}"))?;

    let output = Command::new(gallery_dl_bin)
        .arg("-D")  // exact download directory (no subdirectories)
        .arg(temp_dir.to_string_lossy().to_string())
        .arg("--print")
        .arg("after:{_path}")
        .arg("--no-mtime")
        .arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Failed to spawn gallery-dl: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stderr.is_empty() {
        debug!(stderr = %stderr, "gallery-dl stderr");
    }

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        error!(code = code, stderr = %stderr, "gallery-dl exited with error");
        return Err(format!(
            "gallery-dl exited with code {code}: {stderr}"
        ));
    }

    // Parse printed file paths from stdout (one per line)
    let files: Vec<PathBuf> = stdout
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .collect();

    info!(count = files.len(), "gallery-dl downloaded files");

    if files.is_empty() {
        // Fallback: scan the temp directory for any files
        let mut fallback_files = Vec::new();
        if let Ok(mut entries) = tokio::fs::read_dir(temp_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_file() {
                    fallback_files.push(path);
                }
            }
        }

        if fallback_files.is_empty() {
            return Err("gallery-dl produced no files".to_string());
        }

        info!(
            count = fallback_files.len(),
            "Using fallback directory scan for downloaded files"
        );
        return Ok(DownloadResult {
            files: fallback_files,
        });
    }

    Ok(DownloadResult { files })
}
