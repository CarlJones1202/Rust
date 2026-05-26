use gallery_dl_api::services::custom_downloader;
use tokio::sync::mpsc;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    // Example kitty-kats thread URL (public)
    let url = "https://kitty-kats.net/threads/danna-bliss-teasing-eyes-x107-26-may-2026.2918490/";

    let client = reqwest::Client::builder()
        .user_agent(custom_downloader::BROWSER_UA)
        .http1_only()
        .build()
        .expect("failed to build client");

    let temp_dir = PathBuf::from("./tmp_test_kitty");
    let (tx, mut rx) = mpsc::unbounded_channel();

    match custom_downloader::download_kitty_kats(&client, url, &temp_dir, tx).await {
        Ok(()) => println!("download_kitty_kats returned Ok()"),
        Err(e) => println!("download_kitty_kats returned Err: {}", e),
    }

    // Drain any file paths sent
    while let Ok(path) = rx.try_recv() {
        println!("Downloaded file: {}", path.display());
    }
}
