use std::path::PathBuf;

/// A single track, normalised from the Spotify Web API into just the
/// fields the UI needs.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub uri: String,
    pub name: String,
    pub artists: Vec<String>,
    pub album: String,
    pub album_art: Option<String>,
    pub duration_ms: u32,
}

impl Track {
    pub fn artist_names(&self) -> String {
        self.artists.join(", ")
    }
}

/// A playlist from the current user's library.
#[derive(Debug, Clone, PartialEq)]
pub struct Playlist {
    pub id: String,
    pub uri: String,
    pub name: String,
    pub image: Option<String>,
    pub track_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackStatus {
    Stopped,
    Playing,
    Paused,
}

/// Requests sent from the UI thread to the background Spotify controller.
#[derive(Debug, Clone)]
pub enum Command {
    /// Run (or re-run) the OAuth login flow.
    Login,
    /// Search the catalog for tracks matching `query`.
    Search(String),
    /// Refresh the current user's playlists.
    LoadPlaylists,
    /// Fetch the tracks of a playlist.
    LoadPlaylist(Playlist),
    /// Replace the play queue and start playing `tracks[start_index]`.
    PlayQueue { tracks: Vec<Track>, start_index: usize },
    TogglePlayPause,
    Next,
    Previous,
    /// Seek to an absolute position within the current track, in milliseconds.
    Seek(u32),
    /// Set the output volume, in the range `0.0..=1.0`.
    SetVolume(f64),
}

/// Updates sent from the background Spotify controller to the UI thread.
#[derive(Debug, Clone)]
pub enum Event {
    /// A short human-readable status message (e.g. "Connecting...").
    Status(String),
    /// No `client_id` is configured yet; a template config file was written
    /// to this path (if it didn't already exist) for the user to fill in.
    ConfigRequired(PathBuf),
    LoggedIn { display_name: String },
    Error(String),
    Playlists(Vec<Playlist>),
    PlaylistTracks { playlist: Playlist, tracks: Vec<Track> },
    SearchResults(Vec<Track>),
    /// The currently loaded track, or `None` if playback has stopped.
    NowPlaying(Option<Track>),
    PlaybackStatus(PlaybackStatus),
    Position { position_ms: u32, duration_ms: u32 },
    /// Current mixer volume, in the range `0..=u16::MAX`.
    Volume(u16),
}
