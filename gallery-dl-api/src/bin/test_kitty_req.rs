use reqwest::Client;

#[tokio::main]
async fn main() {
    let url = "https://kitty-kats.net/threads/danna-bliss-teasing-eyes-x107-26-may-2026.2918490/";

    let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";

    // Minimal client (User-Agent + Referer)
    let client_min = Client::builder().user_agent(ua).http1_only().build().unwrap();
    let resp_min = client_min
        .get(url)
        .header("User-Agent", ua)
        .header("Referer", "https://kitty-kats.net/")
        .send()
        .await;

    match resp_min {
        Ok(r) => println!("Minimal GET status: {}", r.status()),
        Err(e) => println!("Minimal GET error: {}", e),
    }

    // Enhanced client with extra headers (what downloader uses)
    let client_enh = Client::builder().user_agent(ua).http1_only().build().unwrap();
    let resp_enh = client_enh
        .get(url)
        .header("User-Agent", ua)
        .header("Referer", "https://kitty-kats.net/")
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header("Accept-Language", "en-US,en;q=0.5")
        .header("Upgrade-Insecure-Requests", "1")
        .header("Connection", "keep-alive")
        .send()
        .await;

    match resp_enh {
        Ok(r) => println!("Enhanced GET status: {}", r.status()),
        Err(e) => println!("Enhanced GET error: {}", e),
    }

    // HEAD with enhanced headers
    let head = client_enh
        .head(url)
        .header("User-Agent", ua)
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header("Accept-Language", "en-US,en;q=0.5")
        .header("Connection", "keep-alive")
        .send()
        .await;

    match head {
        Ok(r) => println!("HEAD status: {}", r.status()),
        Err(e) => println!("HEAD error: {}", e),
    }
}
