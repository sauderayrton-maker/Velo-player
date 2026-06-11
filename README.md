# Velo Player

A native Spotify streaming client for Linux, in the same family as
[Velo](https://github.com/sauderayrton-maker/Velo-Browser) — same GTK4 +
libadwaita "Lexus Cockpit / Hyprland Glass" dark theme, same low-glare
aesthetic, but for music instead of the web. Think
[ncspot](https://github.com/hrkfdn/ncspot), with a GUI: a turntable-style
"now playing" panel, your playlists, and search, all in one window.

Playback is powered by [librespot](https://github.com/librespot-org/librespot)
— audio is streamed and decoded locally, the same way the official Spotify
client does it.

## Features

- **Now Playing panel** (left) — a spinning vinyl record with the current
  track's album art as the label, transport controls (previous / play-pause /
  next), a seek bar, and a volume slider. The record spins while a track is
  playing and stops when paused.
- **Track list** (centre) — shows either your search results or the contents
  of a playlist. Click any track to replace the queue and start playing from
  that track.
- **Playlists sidebar** (right) — your Spotify playlists with cover art and
  track counts, with a refresh button.
- **Search bar** (bottom centre) — search Spotify's catalog by track, artist,
  or album.
- Album art everywhere it's relevant (now playing, track rows, playlist rows).
- Credentials are cached locally after the first login, so you only go
  through the OAuth flow once.
- Same dark cockpit theme as Velo — accent blue (`#8ab4d4`), near-black
  backgrounds, glass-style header bar.

## Requirements

- **A Spotify Premium account.** librespot (and the official Spotify client
  it's compatible with) requires Premium for full-track audio playback.
- **A Spotify Developer application** to get a free OAuth Client ID — see
  [Setup](#setup) below. This is a one-time, ~2 minute setup.
- **GTK4 4.12+** and **libadwaita 1.4+**.
- **ALSA** (audio output, via `rodio`/`cpal`).
- A recent [Rust toolchain](https://rustup.rs) (stable, 2021 edition).

| Component  | Arch package | Debian/Ubuntu package          | Fedora package    |
|------------|--------------|---------------------------------|-------------------|
| Build tools| `base-devel` | `build-essential`, `pkg-config` | `gcc`, `pkg-config` |
| GTK4       | `gtk4`       | `libgtk-4-dev`                  | `gtk4-devel`      |
| libadwaita | `libadwaita` | `libadwaita-1-dev`              | `libadwaita-devel`|
| ALSA       | `alsa-lib`   | `libasound2-dev`                | `alsa-lib-devel`  |
| TLS        | `openssl`    | `libssl-dev`                    | `openssl-devel`   |

## Setup

### 1. Create a Spotify app (one-time)

Velo Player needs its own Spotify "app" to authenticate through — this is
free and just identifies the app to Spotify's API.

1. Go to the [Spotify Developer Dashboard](https://developer.spotify.com/dashboard)
   and log in with your Spotify account.
2. Click **Create app**. Give it any name/description (e.g. "Velo Player").
3. In **Redirect URIs**, add exactly:
   ```
   http://127.0.0.1:8978/login
   ```
4. Save, then open the app's **Settings** and copy the **Client ID**.

### 2. Build

```bash
cargo build --release
```

### 3. First run

```bash
cargo run --release
```

On first launch, Velo Player writes a config template to
`~/.config/velo-player/config.toml` and shows a banner asking you to fill in
your Client ID:

```toml
client_id = "your-client-id-here"
redirect_port = 8978
```

Save the file, then click **Retry** in the banner. This opens your default
browser for Spotify's login/consent screen (the standard OAuth
authorization-code-with-PKCE flow — Velo Player never sees your password).
Once you approve, the browser redirects back to the local server on
`127.0.0.1:8978`, the window connects, and your playlists load.

Your session is then cached, so you won't need to log in again on future
runs.

## Usage

- **Search**: type into the bar at the bottom and press Enter. Results appear
  in the centre panel — click a track to play it (and queue the rest of the
  results after it).
- **Playlists**: click a playlist on the right to load its tracks into the
  centre panel. Use the refresh icon in the sidebar header to reload your
  playlist list.
- **Now playing**: the centre track that's currently playing is highlighted.
  Use the transport buttons or the seek bar to control playback, and the
  slider at the bottom of the left panel for volume.

## Configuration & data locations

| Path                                   | Contents                                               |
|-----------------------------------------|---------------------------------------------------------|
| `~/.config/velo-player/config.toml`    | Spotify Client ID and OAuth redirect port               |
| `~/.cache/velo-player/credentials`     | Cached login credentials (so you don't re-auth each run) |
| `~/.cache/velo-player/volume`          | Last-used volume level                                  |
| `~/.cache/velo-player/audio`           | On-disk audio cache (capped at 2 GiB) to avoid re-downloading recently played tracks |

Delete `~/.cache/velo-player/credentials` to log out / switch accounts.

## Architecture

- **UI** — GTK4 + libadwaita on the main thread (`src/window.rs`, `src/ui/`).
- **Spotify controller** — a dedicated OS thread running a Tokio runtime
  (`src/spotify/controller.rs`), owning the `librespot` `Session` and
  `Player`. The UI and controller communicate over async channels
  (`Command`s in, `Event`s out) so GTK never blocks on network I/O.
- **Playback** — `librespot` handles the Spotify Connect session, audio
  decoding, and output via `rodio`.
- **Browsing/search** — the Spotify Web API (playlists, tracks, search) is
  called directly with `ureq`, authenticated via a token derived from the
  librespot session.

## Limitations

- No Spotify Connect remote control: Velo Player doesn't appear as a "device"
  that other Spotify clients can cast to, and it won't take over playback
  already running on another device.
- No liked-songs/library browsing beyond playlists, no queue reordering, and
  no lyrics.
- Audio output uses whatever ALSA considers the default device.

## Security & privacy

- Login uses Spotify's standard OAuth PKCE flow in your system browser — your
  Spotify password is never seen by Velo Player.
- Cached credentials and the audio cache are stored only under
  `~/.cache/velo-player`, never transmitted anywhere besides Spotify's own
  servers.
- The only network requests Velo Player makes are to Spotify's accounts,
  audio streaming, and Web API endpoints.
