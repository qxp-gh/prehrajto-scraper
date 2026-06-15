//! HTML parsers for prehraj.to
//!
//! Contains modules for parsing different page types.

pub mod direct_url;
pub mod search;

pub use direct_url::{
    parse_stream_url, parse_original_download, parse_subtitles, parse_stream_sources,
};
pub use search::parse_search_results;
