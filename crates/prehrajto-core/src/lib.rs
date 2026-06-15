//! Prehraj.to Scraper Core Library
//!
//! Provides async API for searching videos and getting download/streaming URLs from prehraj.to.
//!
//! # Overview
//!
//! This crate provides a complete scraping solution for prehraj.to with:
//! - Rate-limited HTTP client to avoid overwhelming the server
//! - HTML parsers for extracting video information
//! - High-level API for searching videos and getting direct CDN URLs
//!
//! # Example
//!
//! ```no_run
//! use prehrajto_core::{PrehrajtoScraper, Result};
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let scraper = PrehrajtoScraper::new()?;
//!     
//!     // Search for videos
//!     let results = scraper.search("doctor who").await?;
//!     
//!     for video in &results {
//!         println!("{}: {}", video.name, video.download_url);
//!     }
//!     
//!     // Get direct CDN URL for streaming/downloading
//!     if let Some(video) = results.first() {
//!         let cdn_url = scraper.fetch_stream_url(&video.video_slug, &video.video_id).await?;
//!         println!("Direct CDN URL: {}", cdn_url);
//!         // Returns: https://pf-storage4.premiumcdn.net/...?token=...&expires=...
//!     }
//!     
//!     Ok(())
//! }
//! ```
//!
//! # Direct CDN URLs
//!
//! The [`PrehrajtoScraper::fetch_stream_url`] method extracts the actual CDN URL
//! (premiumcdn.net) from the download page. This URL can be used for direct
//! file download or video streaming.
//!
//! **Important:** CDN URLs contain `token` and `expires` parameters and will
//! stop working after expiration (typically hours). Do not cache them long-term.

mod client;
mod error;
pub mod parser;
mod scraper;
mod types;
pub mod url;

// Re-export client types
pub use client::{ClientConfig, PrehrajtoClient, RateLimiter};

// Re-export error types
pub use error::{PrehrajtoError, Result};

// Re-export parser functions
pub use parser::{
    parse_stream_url, parse_original_download, parse_search_results, parse_subtitles,
    parse_stream_sources,
};

// Re-export main scraper API
pub use scraper::PrehrajtoScraper;

// Re-export data types
pub use types::{SubtitleTrack, VideoPageData, VideoResult, VideoSource};

// Re-export URL helper functions for convenience
pub use url::{build_download_url, build_search_url, build_video_url, extract_video_info};
