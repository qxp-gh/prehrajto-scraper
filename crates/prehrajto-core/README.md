# prehrajto-core

Async Rust library for searching videos and getting download links from [prehraj.to](https://prehraj.to).

## Features

- 🔍 Search videos by keywords
- 📥 Generate download URLs
- 🎯 Extract direct CDN URLs (premiumcdn.net) for streaming/downloading
- 🎬 **Quality selection** — fetch all quality variants, returns best by default
- 📝 **Subtitle extraction** — parse VTT subtitle tracks with language metadata
- 📦 **Original file download** — original uploaded file via authenticated (`refresh_token`) flow
- ⏱️ Built-in rate limiting to respect server limits
- 🔄 Automatic retry with exponential backoff
- 📦 Serde serialization support

## Installation

```toml
[dependencies]
prehrajto-core = "1.0"
tokio = { version = "1", features = ["full"] }
```

## Usage

### Search & Stream

```rust
use prehrajto_core::{PrehrajtoScraper, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let scraper = PrehrajtoScraper::new()?;

    // Search for videos
    let results = scraper.search("doctor who").await?;

    for video in &results {
        println!("{}", video.name);
        println!("  Duration: {:?}", video.duration);
        println!("  Size: {:?}", video.file_size);
    }

    // Get best quality CDN URL
    if let Some(video) = results.first() {
        let cdn_url = scraper.fetch_stream_url(&video.video_slug, &video.video_id).await?;
        println!("CDN URL: {}", cdn_url);
    }

    Ok(())
}
```

### Video Page Data (Sources + Subtitles)

Fetch all quality variants and subtitle tracks in a **single request**:

```rust
let data = scraper.fetch_video_page(slug, id).await?;

// Quality sources (e.g., 1080p, 720p)
for source in &data.sources {
    println!("{}p {} — {}", source.resolution, source.label, source.url);
}

// Subtitle tracks (VTT)
for track in &data.subtitles {
    println!("{} ({}) — {}", track.label, track.language, track.url);
}
```

### Original File Download (authenticated)

The original uploaded file (e.g. the full `.mkv`) is gated behind a logged-in
session. Build the scraper with a `refresh_token` cookie taken from a logged-in
browser (DevTools → Application → Cookies → `refresh_token`):

```rust
let scraper = PrehrajtoScraper::with_refresh_token("your_refresh_token_cookie")?;

let original = scraper.fetch_original_download(slug, id).await?;
println!("{} — {}", original.label, original.url);
// e.g. the original .mkv on premiumcdn.net
```

Search and streaming (`fetch_stream_url`) work anonymously — only the original
file download needs the token. The short-lived `access_token` is refreshed
automatically; the long-lived `refresh_token` is the only credential you supply.

## Configuration

```rust
use prehrajto_core::{PrehrajtoScraper, ClientConfig};

let config = ClientConfig {
    requests_per_second: 1.0,            // Max requests per second
    timeout_secs: 60,                    // Request timeout
    max_retries: 5,                      // Retry attempts on failure
    refresh_token: Some("…".into()),     // Optional: auth for original-file download
};

let scraper = PrehrajtoScraper::with_config(config)?;
```

## Data Types

### VideoResult (search results)

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Video title |
| `url` | `String` | Video page URL |
| `video_id` | `String` | Unique video ID |
| `video_slug` | `String` | URL-friendly slug |
| `download_url` | `String` | Download page URL |
| `duration` | `Option<String>` | Duration (HH:MM:SS) |
| `quality` | `Option<String>` | Quality (e.g., "HD") |
| `file_size` | `Option<String>` | File size |

### VideoSource (quality variants)

| Field | Type | Description |
|-------|------|-------------|
| `url` | `String` | Direct CDN URL |
| `label` | `String` | Quality label (e.g., "1080p") |
| `resolution` | `u32` | Resolution height (720, 1080, …) |
| `is_default` | `bool` | Default quality in player |
| `format` | `Option<String>` | File extension (mp4, mkv, …) |

### SubtitleTrack

| Field | Type | Description |
|-------|------|-------------|
| `url` | `String` | VTT file CDN URL |
| `language` | `String` | ISO code (e.g., "eng", "cze") |
| `label` | `String` | Display label (e.g., "ENG") |
| `is_default` | `bool` | Default subtitle track |

## API Methods

| Method | Description |
|--------|-------------|
| `search(query)` | Search videos by keywords |
| `download_page_url(slug, id)` | Get download page URL (sync) |
| `fetch_stream_url(slug, id)` | Get best quality CDN URL |
| `fetch_stream_sources(slug, id)` | Get all quality variants |
| `fetch_video_page(slug, id)` | Get sources + subtitles (single fetch) |
| `fetch_subtitles(slug, id)` | Get subtitle tracks |
| `fetch_original_download(slug, id)` | Get original file via download flow |
| `search_movie(name, year)` | Search for a specific movie |
| `search_movies(name, year)` | Search with all matching results |

## License

MIT

## Legal Disclaimer

This project is provided **for educational and research purposes only**.

According to prehraj.to Terms of Service (Articles 7.5 and 7.6), automated requests to their servers are prohibited. By using this library, you acknowledge that:

- You are solely responsible for how you use this software
- The authors are not liable for any misuse or violations of third-party terms of service
- You should obtain proper authorization before scraping any website

**Use at your own risk.**
