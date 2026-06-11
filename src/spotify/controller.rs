use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_channel::{Receiver, Sender};
use librespot::core::cache::Cache;
use librespot::core::{Session, SessionConfig, SpotifyUri};
use librespot::playback::audio_backend;
use librespot::playback::config::{AudioFormat, PlayerConfig};
use librespot::playback::mixer::softmixer::SoftMixer;
use librespot::playback::mixer::{Mixer, MixerConfig};
use librespot::playback::player::{Player, PlayerEvent};

use super::api;
use super::auth::{self, Config, Paths};
use super::types::{Command, Event, PlaybackStatus, Track};

/// How many recently played tracks to remember.
const MAX_RECENTS: usize = 30;

/// Tracks playback queue position, play/pause state, and recently played
/// tracks, which the GTK side doesn't need to know the internals of.
#[derive(Default)]
struct PlayerState {
    queue: Vec<Track>,
    index: Option<usize>,
    is_playing: bool,
    /// Recently played tracks, most-recent-first.
    recents: Vec<Track>,
}

/// Spawns the background thread that owns the librespot session and player.
/// All Spotify I/O happens here; the GTK main thread only ever talks to it
/// through `cmd_rx` / `evt_tx`.
pub fn spawn(cmd_rx: Receiver<Command>, evt_tx: Sender<Event>) {
    std::thread::Builder::new()
        .name("velo-spotify".into())
        .spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("failed to start tokio runtime");
            runtime.block_on(run(cmd_rx, evt_tx));
        })
        .expect("failed to spawn Spotify controller thread");
}

async fn run(cmd_rx: Receiver<Command>, evt_tx: Sender<Event>) {
    let paths = Paths::discover();

    // Wait for a usable config (client_id set), prompting the UI to ask the
    // user to fill in the template and hit "retry" if needed.
    let config = loop {
        match Config::load_or_template(&paths) {
            Ok(Some(config)) => break config,
            Ok(None) => {
                let _ = evt_tx.send(Event::ConfigRequired(paths.config_file())).await;
                match cmd_rx.recv().await {
                    Ok(Command::Login) => continue,
                    _ => return,
                }
            }
            Err(e) => {
                let _ = evt_tx.send(Event::Error(format!("Configuration error: {e:#}"))).await;
                return;
            }
        }
    };

    let cache = match auth::open_cache(&paths) {
        Ok(cache) => cache,
        Err(e) => {
            let _ = evt_tx.send(Event::Error(format!("{e:#}"))).await;
            return;
        }
    };

    let session = Session::new(SessionConfig::default(), Some(cache.clone()));

    // Connect, retrying the OAuth login on demand if it fails.
    loop {
        let _ = evt_tx.send(Event::Status("Connecting to Spotify...".into())).await;
        match auth::connect(&session, &paths, &config).await {
            Ok(()) => break,
            Err(e) => {
                let _ = evt_tx.send(Event::Error(format!("{e:#}"))).await;
                match cmd_rx.recv().await {
                    Ok(Command::Login) => continue,
                    _ => return,
                }
            }
        }
    }

    let _ = evt_tx
        .send(Event::LoggedIn { display_name: session.username() })
        .await;

    // --- Local audio playback setup ---
    let mixer = SoftMixer::open(MixerConfig::default()).expect("failed to open mixer");
    if let Some(volume) = cache.volume() {
        mixer.set_volume(volume);
    }
    let _ = evt_tx.send(Event::Volume(mixer.volume())).await;

    let backend = audio_backend::find(None).expect("no audio backend available");
    let audio_format = AudioFormat::default();
    let player_config = PlayerConfig {
        // Drives periodic `PlayerEvent::PositionChanged` events for the seek bar.
        position_update_interval: Some(Duration::from_millis(500)),
        ..PlayerConfig::default()
    };
    let player = Player::new(player_config, session.clone(), mixer.get_soft_volume(), move || {
        backend(None, audio_format)
    });

    let mut player_events = player.get_player_event_channel();
    let mut state = PlayerState { recents: load_recents(&paths), ..PlayerState::default() };
    let _ = evt_tx.send(Event::Recents(state.recents.clone())).await;

    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                match cmd {
                    Ok(cmd) => {
                        handle_command(cmd, &player, &mixer, &cache, &evt_tx, &mut state, &paths, &config).await
                    }
                    Err(_) => break,
                }
            }
            event = player_events.recv() => {
                match event {
                    Some(event) => handle_player_event(event, &player, &evt_tx, &mut state, &paths).await,
                    None => break,
                }
            }
        }
    }
}

async fn handle_command(
    cmd: Command,
    player: &Arc<Player>,
    mixer: &SoftMixer,
    cache: &Cache,
    evt_tx: &Sender<Event>,
    state: &mut PlayerState,
    paths: &Paths,
    config: &Config,
) {
    match cmd {
        // Already connected by the time the main loop runs.
        Command::Login => {}

        Command::Search(query) => {
            if query.trim().is_empty() {
                let _ = evt_tx.send(Event::SearchResults(Vec::new())).await;
                return;
            }
            spawn_api_call(paths, config, evt_tx, move |token| api::search_tracks(&token, &query), Event::SearchResults);
        }

        Command::PlayQueue { tracks, start_index } => {
            if tracks.is_empty() {
                return;
            }
            let index = start_index.min(tracks.len() - 1);
            let track = tracks[index].clone();
            state.queue = tracks;
            state.index = Some(index);
            load_track(player, &track, evt_tx, state, paths).await;
        }

        Command::TogglePlayPause => {
            if state.index.is_none() {
                return;
            }
            if state.is_playing {
                player.pause();
            } else {
                player.play();
            }
        }

        Command::Next => step_queue(player, state, evt_tx, paths, 1).await,
        Command::Previous => step_queue(player, state, evt_tx, paths, -1).await,

        Command::Seek(position_ms) => player.seek(position_ms),

        Command::SetVolume(fraction) => {
            let volume = (fraction.clamp(0.0, 1.0) * f64::from(u16::MAX)).round() as u16;
            mixer.set_volume(volume);
            player.emit_volume_changed_event(volume);
            cache.save_volume(volume);
        }
    }
}

async fn handle_player_event(
    event: PlayerEvent,
    player: &Arc<Player>,
    evt_tx: &Sender<Event>,
    state: &mut PlayerState,
    paths: &Paths,
) {
    match event {
        PlayerEvent::Playing { position_ms, .. } => {
            state.is_playing = true;
            let _ = evt_tx.send(Event::PlaybackStatus(PlaybackStatus::Playing)).await;
            send_position(evt_tx, state, position_ms).await;
        }
        PlayerEvent::Paused { position_ms, .. } => {
            state.is_playing = false;
            let _ = evt_tx.send(Event::PlaybackStatus(PlaybackStatus::Paused)).await;
            send_position(evt_tx, state, position_ms).await;
        }
        PlayerEvent::PositionCorrection { position_ms, .. }
        | PlayerEvent::PositionChanged { position_ms, .. }
        | PlayerEvent::Seeked { position_ms, .. } => {
            send_position(evt_tx, state, position_ms).await;
        }
        PlayerEvent::Stopped { .. } => {
            state.is_playing = false;
            let _ = evt_tx.send(Event::PlaybackStatus(PlaybackStatus::Stopped)).await;
        }
        PlayerEvent::EndOfTrack { .. } => {
            step_queue(player, state, evt_tx, paths, 1).await;
        }
        PlayerEvent::Unavailable { .. } => {
            let track_name = state.index.and_then(|i| state.queue.get(i)).map(|t| t.name.clone());
            let message = match track_name {
                Some(name) => format!("Skipping \"{name}\": unavailable for playback"),
                None => "Skipping track: unavailable for playback".to_string(),
            };
            let _ = evt_tx.send(Event::Error(message)).await;
            step_queue(player, state, evt_tx, paths, 1).await;
        }
        PlayerEvent::VolumeChanged { volume } => {
            let _ = evt_tx.send(Event::Volume(volume)).await;
        }
        _ => {}
    }
}

/// Loads `track` into the player, tells the UI it's now the current track,
/// and records it in the recently-played list.
async fn load_track(player: &Arc<Player>, track: &Track, evt_tx: &Sender<Event>, state: &mut PlayerState, paths: &Paths) {
    match SpotifyUri::from_uri(&track.uri) {
        Ok(uri) => {
            player.load(uri, true, 0);
            let _ = evt_tx.send(Event::NowPlaying(Some(track.clone()))).await;

            state.recents.retain(|t| t.uri != track.uri);
            state.recents.insert(0, track.clone());
            state.recents.truncate(MAX_RECENTS);
            save_recents(paths, &state.recents);
            let _ = evt_tx.send(Event::Recents(state.recents.clone())).await;
        }
        Err(e) => {
            let _ = evt_tx.send(Event::Error(format!("Couldn't play \"{}\": {e}", track.name))).await;
        }
    }
}

/// Moves the queue cursor by `delta` (+1 for next, -1 for previous), loading
/// the new track or stopping playback if the queue is exhausted.
async fn step_queue(player: &Arc<Player>, state: &mut PlayerState, evt_tx: &Sender<Event>, paths: &Paths, delta: i64) {
    let Some(index) = state.index else { return };
    let new_index = index as i64 + delta;

    if new_index < 0 || new_index as usize >= state.queue.len() {
        player.stop();
        state.index = None;
        let _ = evt_tx.send(Event::NowPlaying(None)).await;
        return;
    }

    let new_index = new_index as usize;
    state.index = Some(new_index);
    let track = state.queue[new_index].clone();
    load_track(player, &track, evt_tx, state, paths).await;
}

async fn send_position(evt_tx: &Sender<Event>, state: &PlayerState, position_ms: u32) {
    let duration_ms = state.index.and_then(|i| state.queue.get(i)).map(|t| t.duration_ms).unwrap_or(0);
    let _ = evt_tx.send(Event::Position { position_ms, duration_ms }).await;
}

fn recents_file(paths: &Paths) -> PathBuf {
    paths.cache_dir.join("recent_tracks.json")
}

fn load_recents(paths: &Paths) -> Vec<Track> {
    std::fs::read_to_string(recents_file(paths))
        .ok()
        .and_then(|data| serde_json::from_str(&data).ok())
        .unwrap_or_default()
}

fn save_recents(paths: &Paths, tracks: &[Track]) {
    if std::fs::create_dir_all(&paths.cache_dir).is_err() {
        return;
    }
    if let Ok(data) = serde_json::to_string(tracks) {
        let _ = std::fs::write(recents_file(paths), data);
    }
}

/// Fetches a Web API token, runs `call` on a blocking thread with it, and
/// reports the result (or any error) back to the UI via `evt_tx`.
fn spawn_api_call<F, T, M>(paths: &Paths, config: &Config, evt_tx: &Sender<Event>, call: F, on_success: M)
where
    F: FnOnce(String) -> Result<T> + Send + 'static,
    T: Send + 'static,
    M: FnOnce(T) -> Event + Send + 'static,
{
    let paths = paths.clone();
    let config = config.clone();
    let evt_tx = evt_tx.clone();
    tokio::spawn(async move {
        let token = match auth::web_api_token(&paths, &config).await {
            Ok(token) => token,
            Err(e) => {
                let _ = evt_tx.send(Event::Error(format!("{e:#}"))).await;
                return;
            }
        };

        match tokio::task::spawn_blocking(move || call(token)).await {
            Ok(Ok(value)) => {
                let _ = evt_tx.send(on_success(value)).await;
            }
            Ok(Err(e)) => {
                let _ = evt_tx.send(Event::Error(format!("{e:#}"))).await;
            }
            Err(e) => {
                let _ = evt_tx.send(Event::Error(format!("internal error: {e}"))).await;
            }
        }
    });
}
