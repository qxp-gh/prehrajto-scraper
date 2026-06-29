# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
(pre-1.0: breaking changes bump the minor version).

## [1.1.0] - 2026-06-29

Performance release. No API changes — fully backwards compatible with 1.0.0.
Focused on cutting prehraj.to round-trips and per-request CPU on the hot paths
(search, CDN-URL resolution, page parsing).

### Changed
- **Rate limiter no longer holds its mutex across the sleep.** `RateLimiter::acquire`
  now reserves the next `min_interval`-spaced slot under the lock and releases it
  before sleeping. Concurrent callers (e.g. a fan-out of CDN-URL resolutions) get
  distinct staggered slots without serializing their lock acquisition — same
  throughput, far less head-of-line blocking under concurrency.
- **`fetch_original_download` warms the session at most once per scraper instance.**
  The two-step cookie flow (warmup video page → `?do=download`) previously fetched
  the video page on *every* call. The shared cookie jar keeps the `_nss`/`u_uid`
  session cookies, so subsequent calls skip the warmup — roughly halving the
  round-trips on the original-file resolve path. If a download page comes back
  without a CDN link (a likely sign the session lapsed), the warmup is re-run once
  and the download retried before returning `NotFound`.
- **Connection reuse tuning.** The HTTP client now sets `tcp_keepalive` (60s) and a
  longer `pool_idle_timeout` (120s) so pooled TLS connections to prehraj.to stay
  warm between requests, skipping repeat handshakes.

### Performance
- **Regexes and CSS selectors are compiled once.** Every parser regex (VideoJS /
  JWPlayer source & track blocks, JS-redirect, generic CDN, resolution) and every
  selector (search cards, `a[href]`, `video`/`source[src]`, meta-refresh) is now a
  `LazyLock` static instead of being recompiled on each call. `parse_search_results`
  in particular re-parsed `h3`/`div`/`span.format__text` per result card.

## [1.0.0] - 2026-06-15

First stable release. The API introduced in 0.5.0 is now committed to as stable —
no further breaking changes before a 2.0. No code or API changes since 0.5.0.

### Fixed
- `repository` metadata now points at the current location
  (`github.com/jaymadeapp/prehrajto-scraper`). crates.io releases are immutable, so
  0.5.0 and earlier keep the old (redirected) URL; the crate's main page reflects
  this latest release.

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
