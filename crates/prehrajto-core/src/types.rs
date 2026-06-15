//! Core data types for prehraj.to scraper
//!
//! Contains the main data structures used throughout the library.

use serde::{Deserialize, Serialize};

/// Represents a video result from prehraj.to search
///
/// Contains all metadata extracted from video cards in search results.
/// All fields implement Serialize and Deserialize for Tauri compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoResult {
    /// Video title/name
    pub name: String,

    /// Full URL to the video page
    pub url: String,

    /// Unique alphanumeric video ID (e.g., "63aba7f51f6cf")
    pub video_id: String,

    /// URL-friendly video slug (e.g., "doctor-who-s07e05-andele-dobyvaji-manhattan")
    pub video_slug: String,

    /// Direct download URL with ?do=download parameter
    pub download_url: String,

    /// Video duration in format "HH:MM:SS" (e.g., "00:44:20")
    pub duration: Option<String>,

    /// Video quality indicator (e.g., "HD" or None)
    pub quality: Option<String>,

    /// File size as string (e.g., "1.7 GB")
    pub file_size: Option<String>,
}

/// A single video quality source from the player
///
/// Represents one quality variant (e.g., 720p, 1080p) extracted from
/// the video page's JavaScript player initialization blocks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoSource {
    /// Direct CDN URL for this quality variant
    pub url: String,
    /// Quality label as shown in the player (e.g., "720p", "1080p")
    pub label: String,
    /// Numeric resolution height (720, 1080, 2160, etc.)
    pub resolution: u32,
    /// Whether this is marked as the default quality
    pub is_default: bool,
    /// File extension if known (e.g., "mp4", "mkv", "avi")
    pub format: Option<String>,
}

/// A subtitle track from the video page
///
/// Represents a VTT subtitle file extracted from the video page's
/// JavaScript player initialization blocks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtitleTrack {
    /// Direct CDN URL for the VTT subtitle file
    pub url: String,
    /// ISO language code (e.g., "eng", "cze")
    pub language: String,
    /// Human-readable label (e.g., "ENG", "CZE")
    pub label: String,
    /// Whether this is the default subtitle track
    pub is_default: bool,
}

/// Complete video page data — sources + subtitles
///
/// Returned by [`crate::PrehrajtoScraper::fetch_video_page`] to avoid
/// double-fetching the video page when both sources and subtitles are needed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoPageData {
    /// Available video quality sources
    pub sources: Vec<VideoSource>,
    /// Available subtitle tracks
    pub subtitles: Vec<SubtitleTrack>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_result_serialization() {
        let video = VideoResult {
            name: "Test Video".to_string(),
            url: "https://prehraj.to/test-video/abc123".to_string(),
            video_id: "abc123".to_string(),
            video_slug: "test-video".to_string(),
            download_url: "https://prehraj.to/test-video/abc123?do=download".to_string(),
            duration: Some("01:30:00".to_string()),
            quality: Some("HD".to_string()),
            file_size: Some("1.5 GB".to_string()),
        };

        let json = serde_json::to_string(&video).expect("Serialization should succeed");
        let deserialized: VideoResult =
            serde_json::from_str(&json).expect("Deserialization should succeed");

        assert_eq!(video, deserialized);
    }

    #[test]
    fn test_video_result_with_none_fields() {
        let video = VideoResult {
            name: "Minimal Video".to_string(),
            url: "https://prehraj.to/minimal/xyz789".to_string(),
            video_id: "xyz789".to_string(),
            video_slug: "minimal".to_string(),
            download_url: "https://prehraj.to/minimal/xyz789?do=download".to_string(),
            duration: None,
            quality: None,
            file_size: None,
        };

        let json = serde_json::to_string(&video).expect("Serialization should succeed");
        let deserialized: VideoResult =
            serde_json::from_str(&json).expect("Deserialization should succeed");

        assert_eq!(video, deserialized);
    }
}
