//! Direct URL parser for prehraj.to
//!
//! Parses HTML from video/download pages to extract CDN URLs.
//! Supports multiple quality variants and original file downloads.

use crate::error::{PrehrajtoError, Result};
use crate::types::{SubtitleTrack, VideoSource};
use regex::Regex;
use scraper::{Html, Selector};
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Lazily-compiled regexes & selectors
//
// Compiling a `Regex`/`Selector` is comparatively expensive (and pointless to
// repeat for a pattern that never changes). These are built once on first use
// and reused across every parse — both a latency and a CPU win under load.
// ---------------------------------------------------------------------------

/// `videos.push({ src: "URL", res: 'NUM', label: 'LABEL' ... })` — VideoJS sources.
static RE_VIDEOJS_SOURCES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"videos\.push\(\{[^}]*src:\s*"([^"]+)"[^}]*res:\s*'(\d+)'[^}]*label:\s*'([^']+)'([^}]*)\}"#,
    )
    .expect("valid VideoJS sources regex")
});

/// `{ file: "...premiumcdn...", label: 'LABEL' }` — JWPlayer sources.
static RE_JWPLAYER_SOURCES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\{\s*file:\s*"([^"]*premiumcdn[^"]*)"[^}]*label:\s*'([^']+)'"#)
        .expect("valid JWPlayer sources regex")
});

/// `{ src: "URL", srclang: "LANG", label: "LABEL", kind: "captions" ... }` — VideoJS tracks.
static RE_VIDEOJS_TRACKS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"\{\s*src:\s*"([^"]+)"[^}]*srclang:\s*"([^"]+)"[^}]*label:\s*"([^"]+)"[^}]*kind:\s*"captions"([^}]*)\}"#,
    )
    .expect("valid VideoJS tracks regex")
});

/// `{ file: "URL.vtt...", label: "LABEL", kind: "captions" }` — JWPlayer tracks.
static RE_JWPLAYER_TRACKS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"\{\s*file:\s*"([^"]+\.vtt[^"]*)"[^}]*label:\s*"([^"]+)"[^}]*kind:\s*"captions"([^}]*)\}"#,
    )
    .expect("valid JWPlayer tracks regex")
});

/// JS-redirect patterns (`window.location[.href] = "...premiumcdn..."`).
static RE_JS_REDIRECTS: LazyLock<[Regex; 4]> = LazyLock::new(|| {
    [
        Regex::new(r#"window\.location\.href\s*=\s*["']([^"']+premiumcdn[^"']+)["']"#)
            .expect("valid window.location.href regex"),
        Regex::new(r#"window\.location\s*=\s*["']([^"']+premiumcdn[^"']+)["']"#)
            .expect("valid window.location regex"),
        Regex::new(r#"location\.href\s*=\s*["']([^"']+premiumcdn[^"']+)["']"#)
            .expect("valid location.href regex"),
        Regex::new(r#"location\s*=\s*["']([^"']+premiumcdn[^"']+)["']"#)
            .expect("valid location regex"),
    ]
});

/// Generic premiumcdn URL carrying a `token`/`expires` query param.
static RE_CDN_GENERIC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https?://[^"'\s<>]+premiumcdn\.net[^"'\s<>]*(?:token|expires)[^"'\s<>]*"#)
        .expect("valid generic CDN regex")
});

/// Looser premiumcdn URL fallback (no token/expires requirement).
static RE_CDN_FALLBACK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https?://[^"'\s<>]+premiumcdn\.net[^"'\s<>]+"#).expect("valid fallback CDN regex")
});

/// `(\d{3,4})p` — resolution embedded in freeform text (filenames).
static RE_RESOLUTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d{3,4})p").expect("valid resolution regex"));

static SEL_A_HREF: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("a[href]").expect("valid a[href] selector"));
static SEL_VIDEO_SRC: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("video[src]").expect("valid video[src] selector"));
static SEL_SOURCE_SRC: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("source[src]").expect("valid source[src] selector"));
static SEL_META_REFRESH: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse(r#"meta[http-equiv="refresh"]"#).expect("valid meta-refresh selector")
});

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parses video page HTML and extracts all quality variants
///
/// Tries VideoJS `videos.push(...)` blocks first (best structured data),
/// then falls back to JWPlayer `var sources = [...]` blocks.
///
/// # Arguments
/// * `html` - Raw HTML string from the video page (NOT the download page)
///
/// # Returns
/// Vector of [`VideoSource`] sorted by resolution ascending.
/// Empty vec if no player blocks found.
pub fn parse_stream_sources(html: &str) -> Vec<VideoSource> {
    // Primary: VideoJS videos.push({...}) blocks
    let sources = extract_videojs_sources(html);
    if !sources.is_empty() {
        return sources;
    }

    // Fallback: JWPlayer var sources = [...] block
    extract_jwplayer_sources(html)
}

/// Parses video page HTML and extracts all subtitle tracks
///
/// Tries VideoJS tracks block first (has `srclang` for language code),
/// then falls back to JWPlayer tracks block (extracts language from label).
///
/// # Arguments
/// * `html` - Raw HTML string from the video page
///
/// # Returns
/// Vector of [`SubtitleTrack`]. Empty vec if no tracks found.
pub fn parse_subtitles(html: &str) -> Vec<SubtitleTrack> {
    let tracks = extract_videojs_tracks(html);
    if !tracks.is_empty() {
        return tracks;
    }

    extract_jwplayer_tracks(html)
}

/// Parses download redirect page and extracts the original file URL
///
/// The download page (with cookies) contains an `<a>` tag pointing to the
/// original uploaded file on premiumcdn.net.
///
/// # Arguments
/// * `html` - Raw HTML string from the `?do=download` page (fetched with cookies)
///
/// # Returns
/// A [`VideoSource`] representing the original file
///
/// # Errors
/// Returns `NotFound` if no CDN link found in the redirect page
pub fn parse_original_download(html: &str) -> Result<VideoSource> {
    let document = Html::parse_document(html);

    for element in document.select(&SEL_A_HREF) {
        if let Some(href) = element.value().attr("href")
            && is_cdn_url(href)
        {
            let url = decode_html_entities(href);
            let filename = extract_filename_from_url(&url);
            let resolution = filename
                .as_deref()
                .map(parse_resolution_from_text)
                .unwrap_or(0);
            let label = if resolution > 0 {
                format!("{}p", resolution)
            } else {
                "original".to_string()
            };
            let format = extract_format_from_url(&url);

            return Ok(VideoSource {
                url,
                label,
                resolution,
                is_default: false,
                format,
            });
        }
    }

    Err(PrehrajtoError::NotFound(
        "Could not find original download URL in redirect page".to_string(),
    ))
}

/// Parses download page HTML and extracts the direct CDN URL
///
/// First tries to extract structured quality sources and returns the
/// highest resolution. Falls back to generic extraction methods.
///
/// # Arguments
/// * `html` - Raw HTML string from the page
///
/// # Returns
/// Direct CDN URL string (highest quality available)
///
/// # Errors
/// Returns `NotFound` if no CDN URL could be extracted
pub fn parse_stream_url(html: &str) -> Result<String> {
    // Try structured source parsing first — pick highest resolution
    let sources = parse_stream_sources(html);
    if let Some(best) = sources.iter().max_by_key(|s| s.resolution) {
        return Ok(best.url.clone());
    }

    // Fall back to generic extraction chain
    if let Some(url) = extract_from_anchor(html) {
        return Ok(url);
    }
    if let Some(url) = extract_from_video_element(html) {
        return Ok(url);
    }
    if let Some(url) = extract_from_javascript(html) {
        return Ok(url);
    }
    if let Some(url) = extract_from_meta_refresh(html) {
        return Ok(url);
    }
    if let Some(url) = extract_cdn_url_generic(html) {
        return Ok(url);
    }

    Err(PrehrajtoError::NotFound(
        "Could not find direct CDN URL in download page".to_string(),
    ))
}

// ---------------------------------------------------------------------------
// Helpers — resolution & format parsing
// ---------------------------------------------------------------------------

/// Parses numeric resolution from a quality label like "1080p"
fn parse_resolution_from_label(label: &str) -> u32 {
    let trimmed = label.trim().to_lowercase();
    let numeric = trimmed.trim_end_matches('p');
    numeric.parse::<u32>().unwrap_or(0)
}

/// Tries to find a resolution pattern in freeform text (e.g. filenames)
fn parse_resolution_from_text(text: &str) -> u32 {
    // Match patterns like "2160p", "1080p", "4K"
    if let Some(caps) = RE_RESOLUTION.captures(text)
        && let Some(m) = caps.get(1)
        && let Ok(res) = m.as_str().parse::<u32>()
    {
        return res;
    }
    // Handle "4K" → 2160
    if text.contains("4K") || text.contains("4k") {
        return 2160;
    }
    0
}

/// Extracts file extension from URL path or `filename=` query param
fn extract_format_from_url(url: &str) -> Option<String> {
    // Try filename= query param first
    if let Some(filename) = extract_filename_from_url(url)
        && let Some(ext) = filename.rsplit('.').next()
    {
        let ext = ext.to_lowercase();
        if matches!(
            ext.as_str(),
            "mp4" | "mkv" | "avi" | "webm" | "mov" | "flv" | "wmv" | "m4v"
        ) {
            return Some(ext);
        }
    }

    // Try URL path extension
    let path = url.split('?').next().unwrap_or(url);
    if let Some(ext) = path.rsplit('.').next() {
        let ext = ext.to_lowercase();
        if matches!(
            ext.as_str(),
            "mp4" | "mkv" | "avi" | "webm" | "mov" | "flv" | "wmv" | "m4v"
        ) {
            return Some(ext);
        }
    }

    None
}

/// Extracts filename from `filename=` query parameter
fn extract_filename_from_url(url: &str) -> Option<String> {
    let query = url.split('?').nth(1)?;
    for param in query.split('&') {
        if let Some(value) = param.strip_prefix("filename=") {
            // URL-decode the filename
            return Some(urlencoding::decode(value).unwrap_or_default().into_owned());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// VideoJS & JWPlayer extraction
// ---------------------------------------------------------------------------

/// Extracts sources from VideoJS `videos.push({...})` blocks
fn extract_videojs_sources(html: &str) -> Vec<VideoSource> {
    let mut sources = Vec::new();

    // Match: videos.push({ src: "URL", type: '...', res: 'NUM', label: 'LABEL' ... })
    // The `default: true` may or may not be present
    for caps in RE_VIDEOJS_SOURCES.captures_iter(html) {
        let url = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        let res_str = caps.get(2).map(|m| m.as_str()).unwrap_or("0");
        let label = caps.get(3).map(|m| m.as_str().to_string()).unwrap_or_default();
        let rest = caps.get(4).map(|m| m.as_str()).unwrap_or("");
        let is_default = rest.contains("default: true") || rest.contains("default:true");
        let resolution = res_str.parse::<u32>().unwrap_or(0);
        let format = extract_format_from_url(&url);

        sources.push(VideoSource {
            url,
            label,
            resolution,
            is_default,
            format,
        });
    }

    sources
}

/// Extracts sources from JWPlayer `var sources = [{ file: "...", label: '...' }]` block
fn extract_jwplayer_sources(html: &str) -> Vec<VideoSource> {
    let mut sources = Vec::new();

    // Match: { file: "URL...premiumcdn...", label: 'LABEL' }
    for caps in RE_JWPLAYER_SOURCES.captures_iter(html) {
        let url = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        let label = caps.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
        let resolution = parse_resolution_from_label(&label);
        let format = extract_format_from_url(&url);

        sources.push(VideoSource {
            url,
            label,
            resolution,
            is_default: false,
            format,
        });
    }

    sources
}

// ---------------------------------------------------------------------------
// Subtitle track extraction
// ---------------------------------------------------------------------------

/// Extracts subtitle tracks from VideoJS `var tracks = [{...}]` blocks
///
/// VideoJS tracks have `srclang` which gives the ISO language code directly.
fn extract_videojs_tracks(html: &str) -> Vec<SubtitleTrack> {
    let mut tracks = Vec::new();

    // Match: { src: "URL", srclang: "LANG", label: "LABEL", kind: "captions" ... }
    // `default: true` may or may not be present
    for caps in RE_VIDEOJS_TRACKS.captures_iter(html) {
        let url = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        let language = caps.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
        let raw_label = caps.get(3).map(|m| m.as_str()).unwrap_or("");
        let rest = caps.get(4).map(|m| m.as_str()).unwrap_or("");
        let is_default = rest.contains("default: true") || rest.contains("default:true");
        let label = clean_subtitle_label(raw_label);

        tracks.push(SubtitleTrack {
            url,
            language,
            label,
            is_default,
        });
    }

    tracks
}

/// Extracts subtitle tracks from JWPlayer `var tracks = [{...}]` blocks
///
/// JWPlayer tracks don't have `srclang`, so language is inferred from label.
fn extract_jwplayer_tracks(html: &str) -> Vec<SubtitleTrack> {
    let mut tracks = Vec::new();

    // Match: { file: "URL.vtt...", ... label: "LABEL", kind: "captions" }
    // "default": true may appear with quoted key
    for caps in RE_JWPLAYER_TRACKS.captures_iter(html) {
        let url = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        let raw_label = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let rest = caps.get(3).map(|m| m.as_str()).unwrap_or("");
        let is_default = rest.contains("\"default\": true")
            || rest.contains("\"default\":true")
            || html_before_match_has_default(html, &url);
        let label = clean_subtitle_label(raw_label);
        let language = extract_language_from_label(raw_label);

        tracks.push(SubtitleTrack {
            url,
            language,
            label,
            is_default,
        });
    }

    tracks
}

/// Checks if `"default": true` appears before the file URL in a JWPlayer track entry
fn html_before_match_has_default(html: &str, url: &str) -> bool {
    if let Some(pos) = html.find(url) {
        // Look back up to 200 chars for "default": true within the same object
        let start = pos.saturating_sub(200);
        let window = &html[start..pos];
        // Only count it if no `{` appears between (meaning same object)
        if let Some(brace_pos) = window.rfind('{') {
            let within_obj = &window[brace_pos..];
            return within_obj.contains("\"default\": true")
                || within_obj.contains("\"default\":true");
        }
    }
    false
}

/// Cleans subtitle label: "ENG - 8175377 - eng" → "ENG"
fn clean_subtitle_label(raw: &str) -> String {
    raw.split(" - ").next().unwrap_or(raw).trim().to_string()
}

/// Extracts language code from label: "ENG - 8175377 - eng" → "eng"
fn extract_language_from_label(raw: &str) -> String {
    let parts: Vec<&str> = raw.split(" - ").collect();
    if parts.len() >= 3 {
        parts[parts.len() - 1].trim().to_string()
    } else {
        // Fallback: lowercase the first part
        parts[0].trim().to_lowercase()
    }
}

// ---------------------------------------------------------------------------
// Generic extraction (existing logic, used as fallback)
// ---------------------------------------------------------------------------

/// Extracts CDN URL from anchor tags
fn extract_from_anchor(html: &str) -> Option<String> {
    let document = Html::parse_document(html);

    for element in document.select(&SEL_A_HREF) {
        if let Some(href) = element.value().attr("href")
            && is_cdn_url(href)
        {
            return Some(decode_html_entities(href));
        }
    }
    None
}

/// Extracts CDN URL from video/source elements
fn extract_from_video_element(html: &str) -> Option<String> {
    let document = Html::parse_document(html);

    for element in document.select(&SEL_VIDEO_SRC) {
        if let Some(src) = element.value().attr("src")
            && is_cdn_url(src)
        {
            return Some(decode_html_entities(src));
        }
    }

    for element in document.select(&SEL_SOURCE_SRC) {
        if let Some(src) = element.value().attr("src")
            && is_cdn_url(src)
        {
            return Some(decode_html_entities(src));
        }
    }

    None
}

/// Extracts CDN URL from JavaScript redirects
fn extract_from_javascript(html: &str) -> Option<String> {
    for re in RE_JS_REDIRECTS.iter() {
        if let Some(caps) = re.captures(html)
            && let Some(url) = caps.get(1)
        {
            return Some(url.as_str().to_string());
        }
    }

    None
}

/// Extracts CDN URL from meta refresh tag
fn extract_from_meta_refresh(html: &str) -> Option<String> {
    let document = Html::parse_document(html);

    for element in document.select(&SEL_META_REFRESH) {
        if let Some(content) = element.value().attr("content")
            && let Some(url_part) = content.split("url=").nth(1)
        {
            let url = url_part.trim();
            if is_cdn_url(url) {
                return Some(decode_html_entities(url));
            }
        }
    }

    None
}

/// Generic regex search for CDN URLs in HTML
fn extract_cdn_url_generic(html: &str) -> Option<String> {
    if let Some(m) = RE_CDN_GENERIC.find(html) {
        return Some(decode_html_entities(m.as_str()));
    }

    RE_CDN_FALLBACK
        .find(html)
        .map(|m| decode_html_entities(m.as_str()))
}

/// Decodes common HTML entities in URLs
fn decode_html_entities(url: &str) -> String {
    url.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// Checks if URL is a CDN URL (premiumcdn.net)
fn is_cdn_url(url: &str) -> bool {
    url.contains("premiumcdn.net") || url.contains("cdn.") && url.contains("premium")
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // parse_stream_sources — VideoJS
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_stream_sources_videojs() {
        let html = r#"
        <script>
            var videos = [];
            videos.push({ src: "https://pf-storage3.premiumcdn.net/abc/1080p.mp4?token=x&expires=1", type: 'video/mp4', res: '1080', label: '1080p', default: true });
            videos.push({ src: "https://pf-storage3.premiumcdn.net/abc/720p.mp4?token=y&expires=2", type: 'video/mp4', res: '720', label: '720p' });
        </script>
        "#;

        let sources = parse_stream_sources(html);
        assert_eq!(sources.len(), 2);

        assert_eq!(sources[0].resolution, 1080);
        assert_eq!(sources[0].label, "1080p");
        assert!(sources[0].is_default);
        assert!(sources[0].url.contains("1080p.mp4"));
        assert_eq!(sources[0].format, Some("mp4".to_string()));

        assert_eq!(sources[1].resolution, 720);
        assert_eq!(sources[1].label, "720p");
        assert!(!sources[1].is_default);
    }

    // -----------------------------------------------------------------------
    // parse_stream_sources — JWPlayer
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_stream_sources_jwplayer() {
        let html = r#"
        <script>
            if(player === "jwplayer") {
                var sources = [
                    { file: "https://pf-storage3.premiumcdn.net/abc/720p.mp4?token=a&expires=1", label: '720p' },
                    { file: "https://pf-storage3.premiumcdn.net/abc/1080p.mp4?token=b&expires=2", label: '1080p' }
                ];
            }
        </script>
        "#;

        let sources = parse_stream_sources(html);
        assert_eq!(sources.len(), 2);

        assert_eq!(sources[0].resolution, 720);
        assert_eq!(sources[0].label, "720p");

        assert_eq!(sources[1].resolution, 1080);
        assert_eq!(sources[1].label, "1080p");
    }

    // -----------------------------------------------------------------------
    // parse_stream_sources — both blocks (VideoJS preferred)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_stream_sources_prefers_videojs() {
        let html = r#"
        <script>
            var videos = [];
            videos.push({ src: "https://cdn.premiumcdn.net/videojs/1080p.mp4?token=x", type: 'video/mp4', res: '1080', label: '1080p', default: true });

            if(player === "jwplayer") {
                var sources = [
                    { file: "https://cdn.premiumcdn.net/jwplayer/720p.mp4?token=y", label: '720p' }
                ];
            }
        </script>
        "#;

        let sources = parse_stream_sources(html);
        // VideoJS found → should use VideoJS, not JWPlayer
        assert_eq!(sources.len(), 1);
        assert!(sources[0].url.contains("videojs"));
    }

    // -----------------------------------------------------------------------
    // parse_stream_sources — empty
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_stream_sources_empty() {
        let html = r#"<html><body><p>No video here</p></body></html>"#;

        let sources = parse_stream_sources(html);
        assert!(sources.is_empty());
    }

    // -----------------------------------------------------------------------
    // parse_original_download
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_original_download() {
        let html = r#"
        <html><body>
            <h1>Redirect</h1>
            <p><a href="https://pf-storage3.premiumcdn.net/165065360/abc?filename=Movie+2160p+HEVC.mkv&token=xyz&expires=123">Please click here to continue</a>.</p>
        </body></html>
        "#;

        let source = parse_original_download(html).unwrap();
        assert!(source.url.contains("premiumcdn.net"));
        assert_eq!(source.resolution, 2160);
        assert_eq!(source.label, "2160p");
        assert_eq!(source.format, Some("mkv".to_string()));
        assert!(!source.is_default);
    }

    #[test]
    fn test_parse_original_download_no_link() {
        let html = r#"<html><body><p>No link here</p></body></html>"#;

        let result = parse_original_download(html);
        assert!(result.is_err());
        match result {
            Err(PrehrajtoError::NotFound(_)) => {}
            _ => panic!("Expected NotFound error"),
        }
    }

    // -----------------------------------------------------------------------
    // parse_stream_url — best quality selection
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_stream_url_picks_best_quality() {
        let html = r#"
        <script>
            var videos = [];
            videos.push({ src: "https://pf-storage3.premiumcdn.net/abc/720p.mp4?token=a", type: 'video/mp4', res: '720', label: '720p' });
            videos.push({ src: "https://pf-storage3.premiumcdn.net/abc/1080p.mp4?token=b", type: 'video/mp4', res: '1080', label: '1080p', default: true });
        </script>
        "#;

        let result = parse_stream_url(html).unwrap();
        // Should return 1080p, NOT 720p
        assert!(result.contains("1080p.mp4"));
    }

    // -----------------------------------------------------------------------
    // Resolution & format helpers
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_resolution_from_label() {
        assert_eq!(parse_resolution_from_label("720p"), 720);
        assert_eq!(parse_resolution_from_label("1080p"), 1080);
        assert_eq!(parse_resolution_from_label("2160p"), 2160);
        assert_eq!(parse_resolution_from_label("480p"), 480);
        assert_eq!(parse_resolution_from_label("unknown"), 0);
    }

    #[test]
    fn test_parse_resolution_from_text() {
        assert_eq!(parse_resolution_from_text("Movie 2160p HEVC.mkv"), 2160);
        assert_eq!(parse_resolution_from_text("Movie 1080p.mp4"), 1080);
        assert_eq!(parse_resolution_from_text("Movie 4K HDR"), 2160);
        assert_eq!(parse_resolution_from_text("Movie HD"), 0);
    }

    #[test]
    fn test_extract_format_from_url() {
        assert_eq!(
            extract_format_from_url("https://cdn.premiumcdn.net/abc/file.mp4?token=x"),
            Some("mp4".to_string())
        );
        assert_eq!(
            extract_format_from_url(
                "https://cdn.premiumcdn.net/abc?filename=Movie+2160p.mkv&token=x"
            ),
            Some("mkv".to_string())
        );
        assert_eq!(
            extract_format_from_url("https://cdn.premiumcdn.net/abc?token=x"),
            None
        );
    }

    // -----------------------------------------------------------------------
    // Existing tests (generic extraction fallback)
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_from_anchor() {
        let html = r#"
        <html>
        <body>
            <a href="https://prg-c8-storage5.premiumcdn.net/123/file.mp4?token=abc&expires=123">Download</a>
        </body>
        </html>
        "#;

        let result = parse_stream_url(html);
        assert!(result.is_ok());
        let url = result.unwrap();
        assert!(url.contains("premiumcdn.net"));
        assert!(url.contains("token="));
    }

    #[test]
    fn test_extract_from_video_element() {
        let html = r#"
        <html>
        <body>
            <video src="https://prg-c8-storage5.premiumcdn.net/123/file.mp4?token=abc&expires=123"></video>
        </body>
        </html>
        "#;

        let result = parse_stream_url(html);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("premiumcdn.net"));
    }

    #[test]
    fn test_extract_from_source_element() {
        let html = r#"
        <html>
        <body>
            <video>
                <source src="https://prg-c8-storage5.premiumcdn.net/123/file.mp4?token=abc&expires=123">
            </video>
        </body>
        </html>
        "#;

        let result = parse_stream_url(html);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("premiumcdn.net"));
    }

    #[test]
    fn test_extract_from_javascript_location() {
        let html = r#"
        <html>
        <body>
            <script>
                window.location = "https://prg-c8-storage5.premiumcdn.net/123/file.mp4?token=abc&expires=123";
            </script>
        </body>
        </html>
        "#;

        let result = parse_stream_url(html);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("premiumcdn.net"));
    }

    #[test]
    fn test_extract_from_javascript_location_href() {
        let html = r#"
        <html>
        <body>
            <script>
                window.location.href = "https://prg-c8-storage5.premiumcdn.net/123/file.mp4?token=abc&expires=123";
            </script>
        </body>
        </html>
        "#;

        let result = parse_stream_url(html);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("premiumcdn.net"));
    }

    #[test]
    fn test_extract_from_meta_refresh() {
        let html = r#"
        <html>
        <head>
            <meta http-equiv="refresh" content="0;url=https://prg-c8-storage5.premiumcdn.net/123/file.mp4?token=abc&expires=123">
        </head>
        </html>
        "#;

        let result = parse_stream_url(html);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("premiumcdn.net"));
    }

    #[test]
    fn test_no_cdn_url_found() {
        let html = r#"
        <html>
        <body>
            <p>No video here</p>
        </body>
        </html>
        "#;

        let result = parse_stream_url(html);
        assert!(result.is_err());
        match result {
            Err(PrehrajtoError::NotFound(_)) => {}
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_is_cdn_url() {
        assert!(is_cdn_url(
            "https://prg-c8-storage5.premiumcdn.net/123/file.mp4"
        ));
        assert!(is_cdn_url("https://cdn.premiumcdn.net/file.mp4"));
        assert!(!is_cdn_url("https://prehraj.to/video/123"));
        assert!(!is_cdn_url("https://example.com/file.mp4"));
    }

    #[test]
    fn test_decode_html_entities() {
        let url = "https://example.com?a=1&amp;b=2&amp;c=3";
        let decoded = decode_html_entities(url);
        assert_eq!(decoded, "https://example.com?a=1&b=2&c=3");
    }

    #[test]
    fn test_extract_url_with_html_entities() {
        let html = r#"
        <html>
        <body>
            <a href="https://prg-c8-storage5.premiumcdn.net/123/file.mp4?token=abc&amp;expires=123">Download</a>
        </body>
        </html>
        "#;

        let result = parse_stream_url(html);
        assert!(result.is_ok());
        let url = result.unwrap();
        assert!(url.contains("token=abc&expires=123"));
        assert!(!url.contains("&amp;"));
    }

    // -----------------------------------------------------------------------
    // parse_subtitles — VideoJS
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_videojs_tracks() {
        let html = r#"
        var tracks = [
            {
                src: "https://pf-storage3.premiumcdn.net/123/sub1.vtt?token=abc&expires=123",
                srclang: "eng",
                label: "ENG - 8175377 - eng",
                kind: "captions",
                default: true
            },
            {
                src: "https://pf-storage3.premiumcdn.net/123/sub2.vtt?token=abc&expires=123",
                srclang: "cze",
                label: "CZE - 8175379 - cze",
                kind: "captions"
            }
        ];
        "#;

        let tracks = parse_subtitles(html);
        assert_eq!(tracks.len(), 2);

        assert_eq!(tracks[0].language, "eng");
        assert_eq!(tracks[0].label, "ENG");
        assert!(tracks[0].is_default);
        assert!(tracks[0].url.contains("sub1.vtt"));

        assert_eq!(tracks[1].language, "cze");
        assert_eq!(tracks[1].label, "CZE");
        assert!(!tracks[1].is_default);
    }

    // -----------------------------------------------------------------------
    // parse_subtitles — JWPlayer
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_jwplayer_tracks() {
        let html = r#"
        var tracks = [
            { file: "https://pf-storage3.premiumcdn.net/123/sub1.vtt?token=abc", label: "ENG - 8175377 - eng", kind: "captions" },
            { file: "https://pf-storage3.premiumcdn.net/123/sub2.vtt?token=def", label: "CZE - 8175379 - cze", kind: "captions" }
        ];
        "#;

        let tracks = parse_subtitles(html);
        assert_eq!(tracks.len(), 2);

        assert_eq!(tracks[0].language, "eng");
        assert_eq!(tracks[0].label, "ENG");
        assert!(!tracks[0].is_default);

        assert_eq!(tracks[1].language, "cze");
        assert_eq!(tracks[1].label, "CZE");
    }

    // -----------------------------------------------------------------------
    // parse_subtitles — empty
    // -----------------------------------------------------------------------

    #[test]
    fn test_no_tracks() {
        let html = "<html><body>no tracks here</body></html>";
        let tracks = parse_subtitles(html);
        assert!(tracks.is_empty());
    }

    // -----------------------------------------------------------------------
    // Label cleaning helpers
    // -----------------------------------------------------------------------

    #[test]
    fn test_clean_subtitle_label() {
        assert_eq!(clean_subtitle_label("ENG - 8175377 - eng"), "ENG");
        assert_eq!(clean_subtitle_label("CZE - 8175379 - cze"), "CZE");
        assert_eq!(clean_subtitle_label("ENG1 - 8175378 - eng1"), "ENG1");
        assert_eq!(clean_subtitle_label("Simple"), "Simple");
    }

    #[test]
    fn test_extract_language_from_label() {
        assert_eq!(extract_language_from_label("ENG - 8175377 - eng"), "eng");
        assert_eq!(extract_language_from_label("CZE - 8175379 - cze"), "cze");
        assert_eq!(
            extract_language_from_label("ENG1 - 8175378 - eng1"),
            "eng1"
        );
        // Fallback: lowercase first part
        assert_eq!(extract_language_from_label("Simple"), "simple");
    }
}
