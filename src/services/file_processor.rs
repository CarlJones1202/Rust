use md5::{Digest, Md5};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::info;

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

/// Process all downloaded files: hash, classify, rename, and move to permanent storage.
pub async fn process_files(
    files: Vec<PathBuf>,
    storage_dir: &Path,
) -> Result<Vec<ProcessedFile>, String> {
    let images_dir = storage_dir.join("images");
    let videos_dir = storage_dir.join("videos");

    fs::create_dir_all(&images_dir)
        .await
        .map_err(|e| format!("Failed to create images dir: {e}"))?;
    fs::create_dir_all(&videos_dir)
        .await
        .map_err(|e| format!("Failed to create videos dir: {e}"))?;

    let mut processed = Vec::new();

    for file_path in &files {
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
            continue;
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

        // Remove original
        let _ = fs::remove_file(file_path).await;

        info!(
            hash = %hash,
            ext = %extension,
            media_type = ?media_type,
            "Processed file"
        );

        processed.push(ProcessedFile {
            media_type,
            hash,
            extension,
            original_filename,
            file_size_bytes,
            final_path: dest_path,
        });
    }

    Ok(processed)
}
