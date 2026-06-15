# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
(pre-1.0: breaking changes bump the minor version).

## [0.5.0] - 2026-06-15

### Added
- `PrehrajtoScraper::with_refresh_token` and `ClientConfig.refresh_token` for
  authenticated sessions. prehraj.to now gates the original-file download behind
  a logged-in session — supply the long-lived `refresh_token` cookie from a
  logged-in browser and the short-lived `access_token` is minted and refreshed
  automatically by the cookie jar.

### Changed
- **BREAKING:** renamed the public API for clarity, using a consistent verb
  vocabulary — `build_*` / bare accessors are pure (no I/O), `fetch_*` performs a
  network request, `parse_*` parses already-fetched HTML:

  | Old | New |
  | --- | --- |
  | `get_download_url` | `download_page_url` — now returns `String` (was `Result<String>`) |
  | `get_direct_url` | `fetch_stream_url` |
  | `get_original_url` | `fetch_original_download` |
  | `get_video_sources` | `fetch_stream_sources` |
  | `get_video_page_data` | `fetch_video_page` |
  | `get_subtitle_tracks` | `fetch_subtitles` |
  | `search_movie_all` | `search_movies` |
  | `parse_direct_url` | `parse_stream_url` |
  | `parse_original_download_url` | `parse_original_download` |
  | `parse_video_sources` | `parse_stream_sources` |
  | `parse_subtitle_tracks` | `parse_subtitles` |

  Data types (`VideoResult`, `VideoSource`, `SubtitleTrack`, `VideoPageData`) and
  the Tauri command names are unchanged, so the serialized/frontend contract is
  unaffected.

### Fixed
- Original-file download, which prehraj.to broke by requiring an authenticated
  session for `?do=download` (anonymous requests now redirect back to the video
  page). Anonymous search and streaming are unaffected.

## [0.4.0] - 2026-02-24

### Added
- Quality selection (all variants, best by default), subtitle tracks (VTT), and
  original uploaded-file download.

## [0.3.0] - 2026-02-13

### Added
- Movie search by name and year (`search_movie` / `search_movie_all`).

## [0.2.0] - 2026-01-29

### Added
- `get_direct_url` for extracting direct CDN (premiumcdn.net) URLs.

## [0.1.0] - 2025-12-26

### Added
- Initial release: rate-limited async client, video search, and a Tauri plugin.
