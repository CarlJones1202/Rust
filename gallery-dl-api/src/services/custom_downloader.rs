use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{info, warn};
use regex::Regex;
use scraper::{Html, Selector};

/// Check if a URL is for a site we handle with a custom downloader.
/// Returns the site name if recognized, None otherwise.
pub fn recognize_site(url: &str) -> Option<&'static str> {
    let lower = url.to_lowercase();
    if lower.contains("pmvhaven.com") {
        Some("pmvhaven")
    } else if lower.contains("kitty-kats.net") {
        Some("kitty-kats")
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
        "kitty-kats" => download_kitty_kats(http_client, url, temp_dir, tx).await,
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

/// ── Cookie helpers ─────────────────────────────────────────────────────

/// Try to load a Cookie header from `cookies.txt` in the current directory
/// (Netscape cookie file format). Returns `None` if the file doesn't exist.
fn load_cookie_header(domain: &str) -> Option<String> {
    let path = std::path::Path::new("cookies.txt");
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    let mut cookies: HashMap<String, String> = HashMap::new();
    let domain_lower = domain.to_lowercase();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        // Netscape format: domain, domain_flag, path, secure, expires, name, value
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 7 {
            continue;
        }
        let cookie_domain = parts[0].trim().to_lowercase();
        let trimmed = cookie_domain.trim_start_matches('.');
        // Match if exact or if the request domain is a subdomain of the cookie domain
        let matches = domain_lower == trimmed
            || domain_lower.ends_with(&format!(".{}", trimmed));
        if !matches {
            continue;
        }
        let name = parts[5].trim();
        let value = parts[6].trim();
        if !name.is_empty() && !value.is_empty() {
            cookies.insert(name.to_string(), value.to_string());
        }
    }
    if cookies.is_empty() {
        return None;
    }
    Some(
        cookies
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("; "),
    )
}

/// ── kitty-kats.net downloader ──────────────────────────────────────────

pub const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";

const IMAGE_EXTS: &[&str] = &[".jpg", ".jpeg", ".png", ".gif", ".webp", ".bmp", ".tiff", ".tif", ".avif"];

const KNOWN_IMAGE_HOSTS: &[&str] = &[
    "imagetwist.com",
    "imgbox.com",
    "imagebam.com",
    "imgbb.com",
    "postimg.cc",
    "imagevenue.com",
    "pixhost.to",
    "imgur.com",
    "imagepond.net",
    "imgth.com",
];

/// Download all images from a kitty-kats.net thread page.
/// Finds external image links in the post body and resolves each to its
/// full-size image URL via a generic cascade that works with any image host.
/// Uses a separate reqwest client forced to HTTP/1.1 because Cloudflare
/// blocks HTTP/2 connections from rustls-based clients.
async fn download_kitty_kats(
    _http_client: &reqwest::Client,
    url: &str,
    temp_dir: &Path,
    tx: mpsc::UnboundedSender<PathBuf>,
) -> Result<(), String> {
    tokio::fs::create_dir_all(temp_dir)
        .await
        .map_err(|e| format!("Failed to create temp dir: {e}"))?;

    // Create a dedicated HTTP/1.1-only client so Cloudflare doesn't block us.
    // reqwest's default client negotiates HTTP/2 which triggers Cloudflare's
    // bot detection when combined with rustls's TLS fingerprint.
    let http_client = reqwest::Client::builder()
        .user_agent(BROWSER_UA)
        .http1_only()
        .build()
        .map_err(|e| format!("Failed to build HTTP/1.1 client: {e}"))?;

    let cookie_header = load_cookie_header("kitty-kats.net");

    info!(url = %url, "Fetching kitty-kats.net thread");

    let mut req = http_client
        .get(url)
        .header("User-Agent", BROWSER_UA)
        .header("Referer", "https://kitty-kats.net/")
        // Add common browser headers to reduce bot-detection blocks
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header("Accept-Language", "en-US,en;q=0.5")
        .header("Upgrade-Insecure-Requests", "1")
        .header("Connection", "keep-alive");
    if let Some(cookies) = &cookie_header {
        req = req.header("Cookie", cookies);
    }
    let resp = req.send().await
        .map_err(|e| format!("Failed to fetch thread page: {e}"))?;

    if !resp.status().is_success() {
        // If Cloudflare returned 403 to reqwest, try a curl fallback which
        // uses the system TLS stack / curl fingerprint. This often bypasses
        // Cloudflare checks that block rustls/hyper clients.
        if resp.status().as_u16() == 403 {
            match tokio::process::Command::new("curl")
                .arg("-sSL")
                .arg("-A")
                .arg(BROWSER_UA)
                .arg("-H")
                .arg("Referer: https://kitty-kats.net/")
                .arg(url)
                .output()
                .await
            {
                Ok(out) if out.status.success() => {
                    let html = String::from_utf8_lossy(&out.stdout).to_string();
                    // proceed using html below
                    let candidates = extract_image_candidates(&html);
                    info!(count = candidates.len(), "Found candidate image links (curl fallback)");

                    if candidates.is_empty() {
                        return Err("No image links found in thread (curl fallback)".to_string());
                    }

                    // Continue with the rest of the logic using the extracted candidates
                    let mut success_count = 0u64;
                    let mut error_count = 0u64;

                    for (i, candidate_url) in candidates.iter().enumerate() {
                        info!(num = i + 1, total = candidates.len(), url = %candidate_url, "Resolving image URL (curl fallback)");

                        match resolve_full_image(&http_client, candidate_url).await {
                            Ok(full_url) => {
                                match download_image(&http_client, &full_url, temp_dir, i, &tx).await {
                                    Ok(()) => success_count += 1,
                                    Err(e) => {
                                        warn!(url = %candidate_url, error = %e, "Failed to download image");
                                        error_count += 1;
                                    }
                                }
                            }
                            Err(e) => {
                                warn!(url = %candidate_url, error = %e, "Failed to resolve image URL");
                                error_count += 1;
                            }
                        }
                    }

                    info!(success = success_count, errors = error_count, "Kitty-kats download complete (curl fallback)");

                    if success_count == 0 {
                        return Err("Failed to download any images (curl fallback)".to_string());
                    }

                    return Ok(());
                }
                Ok(out) => return Err(format!("curl failed with status {}", out.status)),
                Err(e) => return Err(format!("Failed to spawn curl fallback: {e}")),
            }
        }

        return Err(format!("Thread page returned status {}", resp.status()));
    }
    let html = resp.text().await.map_err(|e| format!("Failed to read thread page: {e}"))?;
    let candidates = extract_image_candidates(&html);
    info!(count = candidates.len(), "Found candidate image links");

    if candidates.is_empty() {
        return Err("No image links found in thread".to_string());
    }

    let mut success_count = 0u64;
    let mut error_count = 0u64;

    for (i, candidate_url) in candidates.iter().enumerate() {
        info!(num = i + 1, total = candidates.len(), url = %candidate_url, "Resolving image URL");

        match resolve_full_image(&http_client, candidate_url).await {
            Ok(full_url) => {
                match download_image(&http_client, &full_url, temp_dir, i, &tx).await {
                    Ok(()) => success_count += 1,
                    Err(e) => {
                        warn!(url = %candidate_url, error = %e, "Failed to download image");
                        error_count += 1;
                    }
                }
            }
            Err(e) => {
                warn!(url = %candidate_url, error = %e, "Failed to resolve image URL");
                error_count += 1;
            }
        }
    }

    info!(success = success_count, errors = error_count, "Kitty-kats download complete");

    if success_count == 0 {
        return Err("Failed to download any images".to_string());
    }

    Ok(())
}

/// Extract candidate image URLs from a kitty-kats.net thread HTML page.
/// Looks for `<a>` links inside `.bbWrapper` that point to external image content.
fn extract_image_candidates(html: &str) -> Vec<String> {
    let doc = Html::parse_document(html);
    let wrapper_sel = Selector::parse("div.bbWrapper").unwrap();
    let link_sel = Selector::parse("a[href]").unwrap();
    let img_child_sel = Selector::parse("img").unwrap();

    let mut urls = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let wrapper = match doc.select(&wrapper_sel).next() {
        Some(w) => w,
        None => return urls,
    };

    for link in wrapper.select(&link_sel) {
        let href = link.value().attr("href").unwrap();

        if !is_image_candidate(href) {
            continue;
        }

        if !seen.insert(href.to_string()) {
            continue;
        }

        let has_img_child = link.select(&img_child_sel).next().is_some();
        let looks_like_image = IMAGE_EXTS.iter().any(|ext| href.to_lowercase().ends_with(ext));
        let is_known_host = KNOWN_IMAGE_HOSTS.iter().any(|host| href.to_lowercase().contains(host));

        if has_img_child || looks_like_image || is_known_host {
            urls.push(href.to_string());
        }
    }

    urls
}

/// Check if a URL is a valid image candidate (not internal, not a known non-image host).
fn is_image_candidate(href: &str) -> bool {
    let lower = href.to_lowercase();
    !(href.starts_with('/')
        || href.starts_with('#')
        || href.starts_with("javascript:")
        || lower.contains("kitty-kats.net")
        || lower.contains("k2s.cc")
        || lower.contains("keep2share")
        || lower.contains("picstate"))
}

/// Resolve an image page URL to the actual full-size image URL.
/// Uses a cascade of strategies to handle any image host.
async fn resolve_full_image(
    http_client: &reqwest::Client,
    page_url: &str,
) -> Result<String, String> {
    // Strategy 1: HEAD to check if the URL itself is a direct image
    let head_resp = http_client
        .head(page_url)
        .header("User-Agent", BROWSER_UA)
        // make the HEAD look more like a browser call
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header("Accept-Language", "en-US,en;q=0.5")
        .header("Connection", "keep-alive")
        .send()
        .await
        .map_err(|e| format!("HEAD request failed: {e}"))?;

    if let Some(content_type) = head_resp.headers().get("content-type") {
        if let Ok(ct) = content_type.to_str() {
            if ct.starts_with("image/") {
                return Ok(page_url.to_string());
            }
        }
    }

    // Strategy 2: GET the page and parse HTML for the full-size image
    let resp = http_client
        .get(page_url)
        .header("User-Agent", BROWSER_UA)
        .header("Referer", "https://kitty-kats.net/")
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header("Accept-Language", "en-US,en;q=0.5")
        .header("Connection", "keep-alive")
        .send()
        .await
        .map_err(|e| format!("GET request failed: {e}"))?;

    // Check if response is itself a direct image (some hosts redirect)
    if let Some(content_type) = resp.headers().get("content-type") {
        if let Ok(ct) = content_type.to_str() {
            if ct.starts_with("image/") {
                return Ok(page_url.to_string());
            }
        }
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !content_type.contains("text/html") && !content_type.contains("application/xhtml") {
        return Ok(page_url.to_string());
    }

    let body = resp.text().await.map_err(|e| format!("Failed to read response body: {e}"))?;
    let doc = Html::parse_document(&body);

    // 2a. Open Graph image meta tag
    let og_sel = Selector::parse(r#"meta[property="og:image"]"#).unwrap();
    if let Some(el) = doc.select(&og_sel).next() {
        if let Some(url) = el.value().attr("content") {
            if !url.is_empty() {
                info!("Resolved via og:image");
                return Ok(absolutize_url(page_url, url));
            }
        }
    }

    // 2b. image_src link tag
    let img_src_sel = Selector::parse(r#"link[rel="image_src"]"#).unwrap();
    if let Some(el) = doc.select(&img_src_sel).next() {
        if let Some(url) = el.value().attr("href") {
            if !url.is_empty() {
                info!("Resolved via image_src");
                return Ok(absolutize_url(page_url, url));
            }
        }
    }

    // 2c. Fancybox gallery link (imagetwist, many others)
    let fancybox_sel = Selector::parse(r#"a[data-fancybox="gallery"]"#).unwrap();
    if let Some(el) = doc.select(&fancybox_sel).next() {
        if let Some(url) = el.value().attr("href") {
            if !url.is_empty() {
                info!("Resolved via fancybox gallery link");
                return Ok(absolutize_url(page_url, url));
            }
        }
    }

    // 2d. Download link (common pattern on image hosts)
    let download_sel = Selector::parse("a[download]").unwrap();
    if let Some(el) = doc.select(&download_sel).next() {
        if let Some(url) = el.value().attr("href") {
            if !url.is_empty() && is_likely_content_image(url) {
                info!("Resolved via download link");
                return Ok(absolutize_url(page_url, url));
            }
        }
    }

    // 2e. Fallback: first content <img> with absolute src
    let img_sel = Selector::parse("img[src]").unwrap();
    for el in doc.select(&img_sel) {
        if let Some(src) = el.value().attr("src") {
            if is_likely_content_image(src) {
                info!("Resolved via fallback <img> tag");
                return Ok(absolutize_url(page_url, src));
            }
        }
    }

    Err(format!("Could not find any image URL on page {page_url}"))
}

/// Check if an image src looks like a real content image vs a UI element.
fn is_likely_content_image(src: &str) -> bool {
    let lower = src.to_lowercase();
    if lower.contains("icon")
        || lower.contains("logo")
        || lower.contains("favicon")
        || lower.contains("avatar")
        || lower.contains("sprite")
        || lower.contains("banner")
        || lower.contains("button")
        || lower.contains("spacer")
        || lower.contains("pixel")
    {
        return false;
    }
    src.starts_with("http://") || src.starts_with("https://") || src.starts_with("//")
}

/// Convert a potentially relative URL to absolute using a base URL.
fn absolutize_url(base: &str, url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        return url.to_string();
    }
    if url.starts_with("//") {
        return format!("https:{}", url);
    }
    if url.starts_with('/') {
        if let Some(scheme_end) = base.find("://") {
            let rest = &base[scheme_end + 3..];
            if let Some(host_end) = rest.find('/') {
                return format!("{}/{}", &base[..scheme_end + 3 + host_end], url.trim_start_matches('/'));
            }
            return format!("{}{}", base.trim_end_matches('/'), url);
        }
    }
    if let Some(last_slash) = base.rfind('/') {
        if last_slash > base.find("://").map(|i| i + 2).unwrap_or(0) {
            return format!("{}/{}", &base[..last_slash], url.trim_start_matches('/'));
        }
    }
    format!("{}/{}", base.trim_end_matches('/'), url.trim_start_matches('/'))
}

/// Download an image from a URL to temp_dir and send the path via tx.
async fn download_image(
    http_client: &reqwest::Client,
    image_url: &str,
    temp_dir: &Path,
    index: usize,
    tx: &mpsc::UnboundedSender<PathBuf>,
) -> Result<(), String> {
    let url_path = image_url.split('?').next().unwrap_or(image_url);
    let filename = url_path.rsplit('/').next().unwrap_or("image.jpg");

    let clean_name: String = filename
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .collect();

    let dest_path = temp_dir.join(format!("{:03}_{}", index + 1, clean_name));

    info!(url = %image_url, dest = %dest_path.display(), "Downloading image");

    let resp = http_client
        .get(image_url)
        .header("User-Agent", BROWSER_UA)
        .header("Referer", "https://kitty-kats.net/")
        .header("Accept", "image/avif,image/webp,image/apng,image/*,*/*;q=0.8")
        .header("Accept-Language", "en-US,en;q=0.5")
        .header("Connection", "keep-alive")
        .send()
        .await
        .map_err(|e| format!("Failed to start image download: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Image download returned status {}", resp.status()));
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !content_type.starts_with("image/") {
        return Err(format!("Response is not an image (Content-Type: {content_type})"));
    }

    let mut file = tokio::fs::File::create(&dest_path)
        .await
        .map_err(|e| format!("Failed to create output file: {e}"))?;

    let mut bytes_written: u64 = 0;
    let mut stream = resp;
    while let Some(chunk) = stream
        .chunk()
        .await
        .map_err(|e| format!("Failed to read image chunk: {e}"))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Failed to write image chunk: {e}"))?;
        bytes_written += chunk.len() as u64;
    }
    file.flush()
        .await
        .map_err(|e| format!("Failed to flush image file: {e}"))?;

    if bytes_written == 0 {
        let _ = tokio::fs::remove_file(&dest_path).await;
        return Err("Downloaded image is empty".to_string());
    }

    info!(size = bytes_written, path = %dest_path.display(), "Image download complete");

    let _ = tx.send(dest_path);
    Ok(())
}
