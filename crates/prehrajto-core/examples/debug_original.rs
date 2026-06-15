//! Debug script to test the authenticated original-file download flow.
//!
//! prehraj.to gates the original file behind a logged-in session. Provide a
//! `refresh_token` cookie value from a logged-in browser session:
//!
//! ```sh
//! PREHRAJTO_REFRESH_TOKEN=<token> \
//!   cargo run --example debug_original -p prehrajto-core
//! ```
//!
//! Streaming (`fetch_stream_url`) is tested too — it works without a token.

use prehrajto_core::PrehrajtoScraper;

// Example video: Doctor Who S05E10 — Vincent a Doktor (CZ)
const SLUG: &str = "doctor-who-s05e10-vincent-a-doktor-cz";
const VIDEO_ID: &str = "63abf30b7a068";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("PREHRAJTO_REFRESH_TOKEN").ok();

    // 1. Streaming — works anonymously.
    println!("== Streaming (anonymous) ==");
    let anon = PrehrajtoScraper::new()?;
    match anon.fetch_stream_url(SLUG, VIDEO_ID).await {
        Ok(url) => println!("✓ direct stream URL: {url}\n"),
        Err(e) => println!("✗ streaming failed: {e}\n"),
    }

    // 2. Original file — requires an authenticated session.
    println!("== Original download (authenticated) ==");
    let Some(token) = token else {
        println!("⚠ set PREHRAJTO_REFRESH_TOKEN to test the authenticated download");
        return Ok(());
    };

    let scraper = PrehrajtoScraper::with_refresh_token(token)?;
    match scraper.fetch_original_download(SLUG, VIDEO_ID).await {
        Ok(source) => {
            println!("✓ original file URL: {}", source.url);
            println!("  label: {}  resolution: {}  format: {:?}", source.label, source.resolution, source.format);
            assert!(source.url.contains("premiumcdn.net"), "expected a CDN URL");
            println!("\n✓ authenticated download flow works");
        }
        Err(e) => println!("✗ original download failed: {e}"),
    }

    Ok(())
}
