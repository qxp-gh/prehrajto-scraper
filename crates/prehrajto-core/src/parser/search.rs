//! Search results parser for prehraj.to
//!
//! Parses HTML from search results page and extracts video information.

use scraper::{Html, Selector, ElementRef};
use std::sync::LazyLock;
use crate::error::Result;
use crate::types::VideoResult;
use crate::url::{build_download_url, extract_video_info};

// Selectors are compiled once and reused — `parse_video_card` runs per result, so
// re-parsing these on every card (20–50× per search page) was pure wasted CPU.
static SEL_MAIN_A: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("main a[href]").expect("valid 'main a[href]' selector"));
static SEL_H3: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("h3").expect("valid 'h3' selector"));
static SEL_DIV: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div").expect("valid 'div' selector"));
static SEL_FORMAT_TEXT: LazyLock<Selector> = LazyLock::new(|| {
    Selector::parse("span.format__text").expect("valid 'span.format__text' selector")
});

/// Parses search results HTML and returns a list of video results
///
/// # Arguments
/// * `html` - Raw HTML string from search results page
///
/// # Returns
/// Vector of `VideoResult` structs, empty if no results found
///
/// # Errors
/// Returns `ParseError` if HTML structure is invalid
pub fn parse_search_results(html: &str) -> Result<Vec<VideoResult>> {
    let document = Html::parse_document(html);

    // Select all video card links in main content
    // Based on docs: main > div > div contains <a> links for each video
    let mut results = Vec::new();

    for element in document.select(&SEL_MAIN_A) {
        // Try to parse each link as a video card
        if let Some(video) = parse_video_card(&element) {
            results.push(video);
        }
    }
    
    Ok(results)
}

/// Parses a single video card element
///
/// # Arguments
/// * `element` - Reference to an `<a>` element containing video card
///
/// # Returns
/// `Some(VideoResult)` if parsing succeeds, `None` otherwise
fn parse_video_card(element: &ElementRef) -> Option<VideoResult> {
    // Get href attribute
    let href = element.value().attr("href")?;
    
    // Extract slug and id from URL
    let (video_slug, video_id) = extract_video_info(href)?;
    
    // Build URLs
    let url = format!("https://prehraj.to{}", href.split('?').next().unwrap_or(href));
    let download_url = build_download_url(&video_slug, &video_id);
    
    // Extract video name from h3
    let name = element
        .select(&SEL_H3)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())?;
    
    // Skip if name is empty (not a video card)
    if name.is_empty() {
        return None;
    }
    
    // Extract duration, quality, and file size from div elements
    // Only get direct text content from leaf divs (divs without child divs)
    let mut texts: Vec<String> = Vec::new();

    for div in element.select(&SEL_DIV) {
        // Get only direct text nodes, not nested content
        let text: String = div.text()
            .next()
            .map(|t| t.trim().to_string())
            .unwrap_or_default();
        
        if !text.is_empty() && !texts.contains(&text) {
            texts.push(text);
        }
    }
    
    let duration = extract_duration(&texts);
    let quality = extract_quality_from_element(element).or_else(|| extract_quality(&texts));
    let file_size = extract_file_size(&texts);
    
    Some(VideoResult {
        name,
        url,
        video_id,
        video_slug,
        download_url,
        duration,
        quality,
        file_size,
    })
}

/// Extracts duration from div texts
///
/// Looks for time format HH:MM:SS or MM:SS
fn extract_duration(divs: &[String]) -> Option<String> {
    for text in divs {
        if is_duration_format(text) {
            return Some(text.clone());
        }
    }
    None
}

/// Checks if text matches duration format (HH:MM:SS or MM:SS)
fn is_duration_format(text: &str) -> bool {
    let parts: Vec<&str> = text.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return false;
    }
    parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

/// Extracts quality indicator from element
///
/// Looks for span.format__text containing "HD"
fn extract_quality_from_element(element: &ElementRef) -> Option<String> {
    for span in element.select(&SEL_FORMAT_TEXT) {
        let text: String = span.text().collect::<String>().trim().to_string();
        if text.to_uppercase().contains("HD") {
            return Some(text);
        }
    }
    None
}

/// Extracts quality indicator from div texts (fallback)
///
/// Looks for "HD" text
fn extract_quality(divs: &[String]) -> Option<String> {
    for text in divs {
        let upper = text.to_uppercase();
        if upper == "HD" || upper.contains("HD") && text.len() <= 4 {
            return Some("HD".to_string());
        }
    }
    None
}

/// Extracts file size from div texts
///
/// Looks for patterns like "1.7 GB", "500 MB"
fn extract_file_size(divs: &[String]) -> Option<String> {
    for text in divs {
        if is_file_size_format(text) {
            return Some(text.clone());
        }
    }
    None
}

/// Checks if text matches file size format (e.g., "1.7 GB", "500 MB")
fn is_file_size_format(text: &str) -> bool {
    let text_upper = text.to_uppercase();
    (text_upper.contains("GB") || text_upper.contains("MB") || text_upper.contains("KB"))
        && text.chars().any(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_html() {
        let html = "<html><body></body></html>";
        let results = parse_search_results(html).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_search_results_single_video() {
        let html = r#"
        <html>
        <body>
        <main>
            <div>
                <div>
                    <a href="/doctor-who-s07e05/63aba7f51f6cf">
                        <div>
                            <img src="thumb.jpg" alt="thumbnail">
                            <div>00:44:20</div>
                            <div>HD</div>
                            <div>1.7 GB</div>
                        </div>
                        <h3>Doctor Who s07e05 - Andělé dobývají Manhattan</h3>
                    </a>
                </div>
            </div>
        </main>
        </body>
        </html>
        "#;
        
        let results = parse_search_results(html).unwrap();
        assert_eq!(results.len(), 1);
        
        let video = &results[0];
        assert_eq!(video.name, "Doctor Who s07e05 - Andělé dobývají Manhattan");
        assert_eq!(video.video_id, "63aba7f51f6cf");
        assert_eq!(video.video_slug, "doctor-who-s07e05");
        assert_eq!(video.url, "https://prehraj.to/doctor-who-s07e05/63aba7f51f6cf");
        assert_eq!(video.download_url, "https://prehraj.to/doctor-who-s07e05/63aba7f51f6cf?do=download");
        assert_eq!(video.duration, Some("00:44:20".to_string()));
        assert_eq!(video.quality, Some("HD".to_string()));
        assert_eq!(video.file_size, Some("1.7 GB".to_string()));
    }

    #[test]
    fn test_parse_search_results_multiple_videos() {
        let html = r#"
        <html>
        <body>
        <main>
            <div>
                <a href="/video-one/abc123">
                    <div><div>01:00:00</div><div>500 MB</div></div>
                    <h3>Video One</h3>
                </a>
                <a href="/video-two/def456">
                    <div><div>02:00:00</div><div>HD</div><div>2 GB</div></div>
                    <h3>Video Two</h3>
                </a>
            </div>
        </main>
        </body>
        </html>
        "#;
        
        let results = parse_search_results(html).unwrap();
        assert_eq!(results.len(), 2);
        
        assert_eq!(results[0].name, "Video One");
        assert_eq!(results[0].video_id, "abc123");
        assert_eq!(results[0].quality, None);
        
        assert_eq!(results[1].name, "Video Two");
        assert_eq!(results[1].video_id, "def456");
        assert_eq!(results[1].quality, Some("HD".to_string()));
    }

    #[test]
    fn test_parse_video_without_optional_fields() {
        let html = r#"
        <html>
        <body>
        <main>
            <a href="/minimal-video/xyz789">
                <h3>Minimal Video</h3>
            </a>
        </main>
        </body>
        </html>
        "#;
        
        let results = parse_search_results(html).unwrap();
        assert_eq!(results.len(), 1);
        
        let video = &results[0];
        assert_eq!(video.name, "Minimal Video");
        assert_eq!(video.video_id, "xyz789");
        assert_eq!(video.duration, None);
        assert_eq!(video.quality, None);
        assert_eq!(video.file_size, None);
    }

    #[test]
    fn test_is_duration_format() {
        assert!(is_duration_format("00:44:20"));
        assert!(is_duration_format("01:30:00"));
        assert!(is_duration_format("44:20"));
        assert!(!is_duration_format("HD"));
        assert!(!is_duration_format("1.7 GB"));
        assert!(!is_duration_format("invalid"));
    }

    #[test]
    fn test_is_file_size_format() {
        assert!(is_file_size_format("1.7 GB"));
        assert!(is_file_size_format("500 MB"));
        assert!(is_file_size_format("100 KB"));
        assert!(is_file_size_format("2GB"));
        assert!(!is_file_size_format("HD"));
        assert!(!is_file_size_format("00:44:20"));
    }

    #[test]
    fn test_skip_links_without_video_structure() {
        let html = r#"
        <html>
        <body>
        <main>
            <a href="/some-page">Not a video</a>
            <a href="/video/abc123">
                <h3>Real Video</h3>
            </a>
        </main>
        </body>
        </html>
        "#;
        
        let results = parse_search_results(html).unwrap();
        // Should only get the one with h3
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Real Video");
    }
}
