pub mod cover_art;
pub mod now_playing;
pub mod recents;
pub mod tracklist;

pub use now_playing::NowPlayingPanel;
pub use recents::RecentsPanel;
pub use tracklist::TrackListPanel;

/// Formats a duration in milliseconds as `m:ss` (or `h:mm:ss` for tracks
/// over an hour, which — Spotify being Spotify — does happen).
pub fn format_duration(ms: u32) -> String {
    let total_secs = ms / 1000;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}
