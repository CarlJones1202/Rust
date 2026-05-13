use md5::{Digest, Md5};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::task;
use tracing::{error, info};

/// Known image extensions.
const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif", "svg", "avif", "jfif",
];

/// Known video extensions.
const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "webm", "mkv", "avi", "mov", "flv", "wmv", "m4v", "ts", "mpg", "mpeg",
];

/// Classification of a media file.
#[derive(Debug, Clone, PartialEq)]
pub enum MediaType {
    Image,
    Video,
    Unknown,
}

/// A processed file ready for DB insertion.
#[derive(Debug, Clone)]
pub struct ProcessedFile {
    pub media_type: MediaType,
    pub hash: String,
    pub extension: String,
    pub original_filename: String,
    pub file_size_bytes: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_seconds: Option<f64>,
    #[allow(dead_code)]
    pub final_path: PathBuf,
}

/// Classify a file by its extension.
pub fn classify_extension(ext: &str) -> MediaType {
    let ext_lower = ext.to_lowercase();
    if IMAGE_EXTENSIONS.contains(&ext_lower.as_str()) {
        MediaType::Image
    } else if VIDEO_EXTENSIONS.contains(&ext_lower.as_str()) {
        MediaType::Video
    } else {
        MediaType::Unknown
    }
}

/// Compute MD5 hash of a file.
pub async fn compute_md5(path: &Path) -> Result<String, String> {
    let data = fs::read(path)
        .await
        .map_err(|e| format!("Failed to read file {}: {e}", path.display()))?;

    let mut hasher = Md5::new();
    hasher.update(&data);
    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

pub async fn process_single_file(
    file_path: &Path,
    storage_dir: &Path,
) -> Result<Option<ProcessedFile>, String> {
    let images_dir = storage_dir.join("images");
    let videos_dir = storage_dir.join("videos");

    fs::create_dir_all(&images_dir)
        .await
        .map_err(|e| format!("Failed to create images dir: {e}"))?;
    fs::create_dir_all(&videos_dir)
        .await
        .map_err(|e| format!("Failed to create videos dir: {e}"))?;

    // Get original filename
    let original_filename = file_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Get extension
    let extension = file_path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "bin".to_string());

    // Classify
    let media_type = classify_extension(&extension);
    if media_type == MediaType::Unknown {
        info!(
            file = %original_filename,
            ext = %extension,
            "Skipping file with unknown media type"
        );
        return Ok(None);
    }

    // Compute hash
    let hash = compute_md5(file_path).await?;

    // Determine destination
    let dest_dir = match media_type {
        MediaType::Image => &images_dir,
        MediaType::Video => &videos_dir,
        MediaType::Unknown => unreachable!(),
    };

    let new_filename = format!("{hash}.{extension}");
    let dest_path = dest_dir.join(&new_filename);

    // Get file size
    let metadata = fs::metadata(file_path)
        .await
        .map_err(|e| format!("Failed to get metadata for {}: {e}", file_path.display()))?;
    let file_size_bytes = metadata.len() as i64;

    // Move file (copy + delete for cross-device moves)
    if !dest_path.exists() {
        fs::copy(file_path, &dest_path)
            .await
            .map_err(|e| format!("Failed to copy file to {}: {e}", dest_path.display()))?;
    }

    // Generate thumbnail for images or videos
    let thumbnail_dir = storage_dir.join("thumbnails");
    fs::create_dir_all(&thumbnail_dir)
        .await
        .map_err(|e| format!("Failed to create thumbnails dir: {e}"))?;

    let thumbnail_filename = format!("{hash}.jpg"); // Thumbnails are always jpg for consistency
    let thumbnail_path = thumbnail_dir.join(&thumbnail_filename);

    if !thumbnail_path.exists() {
        let src = dest_path.clone();
        let dst = thumbnail_path.clone();
        match media_type {
            MediaType::Image => {
                let _ = task::spawn_blocking(move || {
                    if let Err(e) = generate_thumbnail(&src, &dst) {
                        error!(error = %e, path = %src.display(), "Failed to generate image thumbnail");
                    }
                })
                .await;
            }
            MediaType::Video => {
                let _ = task::spawn_blocking(move || {
                    if let Err(e) = generate_video_thumbnail(&src, &dst) {
                        error!(error = %e, path = %src.display(), "Failed to generate video thumbnail");
                    }
                })
                .await;

                // Also generate trickplay assets
                let trickplay_dir = storage_dir.join("trickplay");
                let _ = fs::create_dir_all(&trickplay_dir).await;
                let trickplay_path = trickplay_dir.join(format!("{hash}.jpg"));
                if !trickplay_path.exists() {
                    let src = dest_path.clone();
                    let _ = task::spawn_blocking(move || {
                        if let Err(e) = generate_trickplay_assets(&src, &trickplay_path) {
                            error!(error = %e, path = %src.display(), "Failed to generate trickplay assets");
                        }
                    })
                    .await;
                }
            }
            MediaType::Unknown => {}
        }
    }

    // Get dimensions if image or video
    let (width, height) = match media_type {
        MediaType::Image => {
            let path = dest_path.clone();
            task::spawn_blocking(move || {
                image::image_dimensions(&path)
                    .map(|(w, h)| (Some(w as i32), Some(h as i32)))
                    .unwrap_or((None, None))
            })
            .await
            .unwrap_or((None, None))
        }
        MediaType::Video => {
            let path = dest_path.clone();
            task::spawn_blocking(move || {
                get_video_dimensions(&path)
                    .map(|(w, h)| (Some(w), Some(h)))
                    .unwrap_or((None, None))
            })
            .await
            .unwrap_or((None, None))
        }
        MediaType::Unknown => (None, None),
    };

    // Get duration if video
    let duration_seconds = if media_type == MediaType::Video {
        let path = dest_path.clone();
        task::spawn_blocking(move || get_video_duration(&path).ok())
            .await
            .unwrap_or(None)
    } else {
        None
    };

    // Remove original
    let _ = fs::remove_file(file_path).await;

    tracing::debug!(
        hash = %hash,
        ext = %extension,
        media_type = ?media_type,
        "Processed file"
    );

    Ok(Some(ProcessedFile {
        media_type,
        hash,
        extension,
        original_filename,
        file_size_bytes,
        width,
        height,
        duration_seconds,
        final_path: dest_path,
    }))
}


/// Generate a thumbnail for an image.
fn generate_thumbnail(src: &Path, dst: &Path) -> Result<(), String> {
    let img = image::open(src).map_err(|e| format!("Failed to open image: {e}"))?;

    // Use thumbnail method which maintains aspect ratio
    // 500x500 is a good balance for grid views
    let thumbnail = img.thumbnail(500, 500);

    thumbnail
        .save(dst)
        .map_err(|e| format!("Failed to save thumbnail: {e}"))?;

    Ok(())
}

/// Generate a thumbnail for a video using ffmpeg.
fn generate_video_thumbnail(src: &Path, dst: &Path) -> Result<(), String> {
    use std::process::Command;

    // Try multiple timestamps: 5s, 1s, 0s
    let timestamps = ["00:00:05", "00:00:01", "00:00:00"];
    
    for ts in timestamps {
        let status = Command::new("ffmpeg")
            .arg("-ss")
            .arg(ts)
            .arg("-i")
            .arg(src)
            .arg("-vframes")
            .arg("1")
            .arg("-q:v")
            .arg("2")
            .arg("-s")
            .arg("500x500")
            .arg("-f")
            .arg("image2")
            .arg("-y")
            .arg(dst)
            .status()
            .map_err(|e| format!("Failed to run ffmpeg: {e}"))?;

        if status.success() {
            // Check if file size is reasonable (> 3KB) to avoid black frames
            if let Ok(meta) = std::fs::metadata(dst) {
                if meta.len() > 3000 || ts == "00:00:00" {
                    return Ok(());
                }
            }
        }
    }

    Err("Failed to generate a valid video thumbnail after multiple attempts".to_string())
}

/// Get the duration of a video using ffprobe.
fn get_video_duration(path: &Path) -> Result<f64, String> {
    use std::process::Command;

    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(path)
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {e}"))?;

    if !output.status.success() {
        return Err(format!("ffprobe failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let duration_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    duration_str.parse::<f64>().map_err(|e| format!("Failed to parse duration: {e}"))
}

/// Get the dimensions of a video using ffprobe.
fn get_video_dimensions(path: &Path) -> Result<(i32, i32), String> {
    use std::process::Command;

    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=width,height")
        .arg("-of")
        .arg("csv=s=x:p=0")
        .arg(path)
        .output()
        .map_err(|e| format!("Failed to run ffprobe for dimensions: {e}"))?;

    if !output.status.success() {
        return Err(format!("ffprobe failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let dim_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let parts: Vec<&str> = dim_str.split('x').collect();
    if parts.len() == 2 {
        let width = parts[0].parse::<i32>().map_err(|_| "Failed to parse width".to_string())?;
        let height = parts[1].parse::<i32>().map_err(|_| "Failed to parse height".to_string())?;
        Ok((width, height))
    } else {
        Err(format!("Invalid dimension format: {}", dim_str))
    }
}

/// Generate a trickplay sprite sheet (10x10 tile of frames).
fn generate_trickplay_assets(src: &Path, dst: &Path) -> Result<(), String> {
    use std::process::Command;

    // Get duration to calculate interval
    let duration = get_video_duration(src)?;
    let interval = duration / 100.0;
    let fps = 1.0 / interval;

    // Run ffmpeg to create a 10x10 tile of small frames (160px width)
    let status = Command::new("ffmpeg")
        .arg("-i")
        .arg(src)
        .arg("-vf")
        .arg(format!("fps={},scale=160:-1,tile=10x10", fps))
        .arg("-q:v")
        .arg("2")
        .arg("-y")
        .arg(dst)
        .status()
        .map_err(|e| format!("Failed to run ffmpeg for trickplay: {e}"))?;

    if !status.success() {
        return Err(format!("ffmpeg trickplay failed with status: {status}"));
    }

    Ok(())
}
