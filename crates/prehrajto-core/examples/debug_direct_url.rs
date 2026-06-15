//! Debug script to test fetch_stream_url functionality
//!
//! Run with: cargo run --example debug_direct_url -p prehrajto-core

use prehrajto_core::PrehrajtoScraper;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scraper = PrehrajtoScraper::new()?;

    println!("Searching for 'Teorie Velkého Třesku S01E01'...\n");

    let results = scraper.search("Teorie Velkého Třesku S01E01").await?;

    if results.is_empty() {
        println!("No results found!");
        return Ok(());
    }

    println!("Found {} results:\n", results.len());

    for (i, video) in results.iter().take(3).enumerate() {
        println!("{}. {}", i + 1, video.name);
        println!("   Slug: {}", video.video_slug);
        println!("   ID: {}", video.video_id);
        println!("   Download URL: {}", video.download_url);
        if let Some(ref size) = video.file_size {
            println!("   Size: {}", size);
        }
        println!();
    }

    // Try to get direct URL for the first result
    let video = &results[0];
    println!("Getting direct URL for: {}", video.name);
    println!("Fetching {}...\n", video.download_url);

    match scraper
        .fetch_stream_url(&video.video_slug, &video.video_id)
        .await
    {
        Ok(direct_url) => {
            println!("✓ Direct CDN URL found:");
            println!("{}\n", direct_url);

            // Verify URL properties
            if direct_url.contains("premiumcdn.net") {
                println!("✓ Contains premiumcdn.net");
            }
            if direct_url.contains("token=") {
                println!("✓ Contains token parameter");
            }
            if direct_url.contains("expires=") {
                println!("✓ Contains expires parameter");
            }
        }
        Err(e) => {
            println!("✗ Failed to get direct URL: {}", e);

            // Debug: fetch and save the HTML for inspection
            println!("\nFetching raw HTML for debugging...");
            let client = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .build()?;

            let html = client
                .get(&video.download_url)
                .header("Accept-Language", "cs-CZ")
                .send()
                .await?
                .text()
                .await?;

            std::fs::write("debug_download_page.html", &html)?;
            println!("HTML saved to debug_download_page.html");

            // Print first 2000 chars
            println!("\n=== HTML snippet (first 2000 chars) ===\n");
            println!("{}", &html[..std::cmp::min(2000, html.len())]);
        }
    }

    Ok(())
}
