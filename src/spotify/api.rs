use std::sync::LazyLock;
use std::time::Duration;

use anyhow::{anyhow, Result};
use librespot::core::session::Session;
use serde_json::Value;

use super::types::{Playlist, Track};

const API_BASE: &str = "https://api.spotify.com/v1";

/// Comma-separated scopes (librespot's `TokenProvider` convention) needed
/// for the Web API calls below.
const WEB_API_SCOPES: &str =
    "playlist-read-private,playlist-read-collaborative,user-library-read,user-read-private";

static AGENT: LazyLock<ureq::Agent> =
    LazyLock::new(|| ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).build());

/// Fetches a Web API access token via the authenticated librespot session.
/// librespot's `TokenProvider` caches the result until it expires, so this
/// is cheap to call before every batch of requests.
pub async fn web_api_token(session: &Session) -> Result<String> {
    session
        .token_provider()
        .get_token(WEB_API_SCOPES)
        .await
        .map(|t| t.access_token)
        .map_err(|e| anyhow!("failed to get Web API token: {e}"))
}

/// Searches the catalog for tracks matching `query`.
pub fn search_tracks(token: &str, query: &str) -> Result<Vec<Track>> {
    let body: Value = AGENT
        .get(&format!("{API_BASE}/search"))
        .set("Authorization", &format!("Bearer {token}"))
        .query("type", "track")
        .query("limit", "30")
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

/// Fetches the current user's playlists (up to 200, across 4 pages).
pub fn current_user_playlists(token: &str) -> Result<Vec<Playlist>> {
    let mut playlists = Vec::new();
    let mut url = format!("{API_BASE}/me/playlists?limit=50");

    for _ in 0..4 {
        let body = get_json(token, &url)?;

        let items = body.get("items").and_then(|i| i.as_array()).cloned().unwrap_or_default();
        playlists.extend(items.iter().filter_map(parse_playlist));

        match body.get("next").and_then(|n| n.as_str()) {
            Some(next) if !next.is_empty() => url = next.to_string(),
            _ => break,
        }
    }

    Ok(playlists)
}

/// Fetches the tracks of a playlist (up to 500, across 10 pages).
pub fn playlist_tracks(token: &str, playlist_id: &str) -> Result<Vec<Track>> {
    let mut tracks = Vec::new();
    let mut url = format!(
        "{API_BASE}/playlists/{playlist_id}/tracks?limit=50&fields=items(track(uri,name,duration_ms,artists(name),album(name,images))),next"
    );

    for _ in 0..10 {
        let body = get_json(token, &url)?;

        let items = body.get("items").and_then(|i| i.as_array()).cloned().unwrap_or_default();
        tracks.extend(items.iter().filter_map(|item| item.get("track")).filter_map(parse_track));

        match body.get("next").and_then(|n| n.as_str()) {
            Some(next) if !next.is_empty() => url = next.to_string(),
            _ => break,
        }
    }

    Ok(tracks)
}

fn get_json(token: &str, url: &str) -> Result<Value> {
    AGENT
        .get(url)
        .set("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(|e| anyhow!("request to {url} failed: {e}"))?
        .into_json()
        .map_err(|e| anyhow!("failed to parse response from {url}: {e}"))
}

/// Parses a Web API track object, skipping local files, podcast episodes,
/// and other non-track items that may appear in playlists or search results.
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

fn parse_playlist(v: &Value) -> Option<Playlist> {
    let id = v.get("id")?.as_str()?.to_string();
    let uri = v.get("uri")?.as_str()?.to_string();
    let name = v.get("name")?.as_str()?.to_string();

    let image = first_image_url(v.get("images"));

    let track_count = v
        .get("tracks")
        .and_then(|t| t.get("total"))
        .and_then(|t| t.as_u64())
        .unwrap_or(0) as u32;

    Some(Playlist { id, uri, name, image, track_count })
}

/// Spotify image arrays are ordered largest-first; the first entry is good
/// enough for both the vinyl record art and playlist thumbnails since GTK
/// scales it down to fit.
fn first_image_url(images: Option<&Value>) -> Option<String> {
    images
        .and_then(|imgs| imgs.as_array())
        .and_then(|arr| arr.first())
        .and_then(|img| img.get("url"))
        .and_then(|u| u.as_str())
        .map(String::from)
}
