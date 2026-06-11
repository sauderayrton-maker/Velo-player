use std::sync::LazyLock;
use std::time::Duration;

use anyhow::{anyhow, Result};
use serde_json::Value;

use super::types::Track;

const API_BASE: &str = "https://api.spotify.com/v1";

static AGENT: LazyLock<ureq::Agent> =
    LazyLock::new(|| ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).build());

/// Searches the catalog for tracks matching `query`.
///
/// This app's Spotify access tier rejects `/search` requests with `limit`
/// above 10 (`400 Invalid limit`), so results are capped accordingly.
pub fn search_tracks(token: &str, query: &str) -> Result<Vec<Track>> {
    let body: Value = AGENT
        .get(&format!("{API_BASE}/search"))
        .set("Authorization", &format!("Bearer {token}"))
        .query("type", "track")
        .query("limit", "10")
        .query("q", query)
        .call()
        .map_err(|e| anyhow!("search request failed: {e}"))?
        .into_json()
        .map_err(|e| anyhow!("failed to parse search response: {e}"))?;

    let items = body
        .get("tracks")
        .and_then(|t| t.get("items"))
        .and_then(|i| i.as_array())
        .cloned()
        .unwrap_or_default();

    Ok(items.iter().filter_map(parse_track).collect())
}

/// Parses a Web API track object, skipping local files, podcast episodes,
/// and other non-track items that may appear in search results.
fn parse_track(v: &Value) -> Option<Track> {
    let uri = v.get("uri")?.as_str()?.to_string();
    if !uri.starts_with("spotify:track:") {
        return None;
    }

    let name = v.get("name")?.as_str()?.to_string();
    let duration_ms = v.get("duration_ms")?.as_u64()? as u32;

    let artists = v
        .get("artists")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let album = v
        .get("album")
        .and_then(|a| a.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or_default()
        .to_string();

    let album_art = first_image_url(v.get("album").and_then(|a| a.get("images")));

    Some(Track { uri, name, artists, album, album_art, duration_ms })
}

/// Spotify image arrays are ordered largest-first; the first entry is good
/// enough for both the vinyl record art and track thumbnails since GTK
/// scales it down to fit.
fn first_image_url(images: Option<&Value>) -> Option<String> {
    images
        .and_then(|imgs| imgs.as_array())
        .and_then(|arr| arr.first())
        .and_then(|img| img.get("url"))
        .and_then(|u| u.as_str())
        .map(String::from)
}
