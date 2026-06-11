# Velo Player

A native Spotify streaming client for Linux, in the same family as
[Velo](https://github.com/sauderayrton-maker/Velo-Browser) — same GTK4 +
libadwaita "Lexus Cockpit / Hyprland Glass" dark theme, same low-glare
aesthetic, but for music instead of the web. Think
[ncspot](https://github.com/hrkfdn/ncspot), with a GUI: a turntable-style
"now playing" panel, search, and your recently played tracks, all in one
window.

Playback is powered by [librespot](https://github.com/librespot-org/librespot)
— audio is streamed and decoded locally, the same way the official Spotify
client does it.

## Features

- **Now Playing panel** (left) — a spinning vinyl record with the current
  track's album art as the label, transport controls (previous / play-pause /
  next), a seek bar, and a volume slider. The record spins while a track is
  playing and stops when paused.
- **Track list** (centre) — your search results. Click any track to replace
  the queue and start playing from that track.
- **Recents sidebar** (right) — tracks you've played recently in Velo Player,
  with cover art, ready to replay with one click.
- **Search bar** (bottom centre) — search Spotify's catalog by track, artist,
  or album.
- Album art everywhere it's relevant (now playing, track rows, recents rows).
- Credentials are cached locally after the first login, so you only go
  through the OAuth flow once.
- **Self-update** — "Check for Updates…" in the menu pulls, rebuilds, and
  reinstalls the latest commit.
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

## Installation

### Quick install (recommended)

```bash
git clone https://github.com/sauderayrton-maker/Velo-player.git
cd Velo-player
./install.sh
```

This detects your package manager, installs the system dependencies above,
builds `velo-player` in release mode, and installs it plus a desktop entry
and icon via `make install` (requires `sudo` for the final install step).

### Manual build

```bash
cargo build --release                  # the player
sudo make install PREFIX=/usr/local    # install + desktop entry
```

Run it with `velo-player`, or launch "Velo Player" from your application
menu.

To remove everything Velo Player installed:

```bash
make uninstall            # or: ./uninstall.sh
```

This prompts for `sudo` itself, removes `velo-player`, the desktop entry, and
the icon (refreshing the icon cache), and asks before deleting your local
config/cache (Client ID, login session, recents). A copy is also installed as
`velo-player-uninstall`, so it works even if you've deleted this cloned repo.

### Run without installing

```bash
cargo run --release
```

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

### 2. First run

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
`127.0.0.1:8978` and the window connects.

Your session is then cached, so you won't need to log in again on future
runs.

## Updating

Velo Player can update itself: open the menu (☰ in the header bar) and choose
**"Check for Updates…"**. This fetches the repo it was built from, and if a
newer commit exists on the remote, offers to pull, rebuild, and reinstall it
(you'll be prompted for your password via `pkexec` to finish the install).
When it's done, choose "Restart Now" to relaunch with the new version.

This only works if the cloned repo Velo Player was built from is still
present and unmodified on disk. From the command line, the same process is:

```bash
./update.sh
```

## Usage

- **Search**: type into the bar at the bottom and press Enter. Results appear
  in the centre panel — click a track to play it (and queue the rest of the
  results after it).
- **Recents**: tracks you play are automatically added to the sidebar on the
  right. Click one to play it again.
- **Now playing**: the centre/sidebar track that's currently playing is
  highlighted. Use the transport buttons or the seek bar to control playback,
  and the slider at the bottom of the left panel for volume.

## Configuration & data locations

| Path                                   | Contents                                               |
|-----------------------------------------|---------------------------------------------------------|
| `~/.config/velo-player/config.toml`    | Spotify Client ID and OAuth redirect port               |
| `~/.cache/velo-player/credentials`     | Cached login credentials (so you don't re-auth each run) |
| `~/.cache/velo-player/oauth_token.json`| Cached Web API OAuth token (search, recents art)        |
| `~/.cache/velo-player/volume`          | Last-used volume level                                  |
| `~/.cache/velo-player/audio`           | On-disk audio cache (capped at 2 GiB) to avoid re-downloading recently played tracks |
| `~/.cache/velo-player/recent_tracks.json` | Your Recents sidebar history                         |

Delete `~/.cache/velo-player/credentials` and `~/.cache/velo-player/oauth_token.json`
to log out / switch accounts.

## Architecture

- **UI** — GTK4 + libadwaita on the main thread (`src/window.rs`, `src/ui/`).
- **Spotify controller** — a dedicated OS thread running a Tokio runtime
  (`src/spotify/controller.rs`), owning the `librespot` `Session` and
  `Player`. The UI and controller communicate over async channels
  (`Command`s in, `Event`s out) so GTK never blocks on network I/O.
- **Playback** — `librespot` handles the Spotify Connect session, audio
  decoding, and output via `rodio`.
- **Browsing/search** — the Spotify Web API (search, track/album art) is
  called directly with `ureq`, authenticated via a token cached separately
  from the librespot session.

## Limitations

- No Spotify Connect remote control: Velo Player doesn't appear as a "device"
  that other Spotify clients can cast to, and it won't take over playback
  already running on another device.
- No playlist or library browsing (Spotify's Web API restricts these for
  apps in Development Mode) — search and Recents are the way to find and
  replay tracks.
- No queue reordering or lyrics.
- Audio output uses whatever ALSA considers the default device.

## Security & privacy

- Login uses Spotify's standard OAuth PKCE flow in your system browser — your
  Spotify password is never seen by Velo Player.
- Cached credentials, tokens, recents, and the audio cache are stored only
  under `~/.cache/velo-player`, never transmitted anywhere besides Spotify's
  own servers.
- The only network requests Velo Player makes are to Spotify's accounts,
  audio streaming, and Web API endpoints.
- "Check for Updates" runs `git fetch`/`git rev-parse` against the repo's
  configured remote to compare commit hashes — no other network requests are
  made until you choose "Update Now". The actual update (`git pull`, build,
  install) only escalates privileges for the final file-copy step, via
  `pkexec`/`sudo`.
