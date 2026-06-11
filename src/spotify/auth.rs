use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use librespot::core::authentication::Credentials;
use librespot::core::cache::Cache;
use librespot::core::session::Session;
use librespot::oauth::OAuthClientBuilder;
use serde::{Deserialize, Serialize};

/// Scopes requested during the OAuth login. `streaming` is required for
/// playback; the rest are used by [`crate::spotify::api`] to browse the
/// user's library.
const SCOPES: &[&str] = &[
    "streaming",
    "user-read-playback-state",
    "user-modify-playback-state",
    "user-read-currently-playing",
    "playlist-read-private",
    "playlist-read-collaborative",
    "user-library-read",
];

const CONFIG_TEMPLATE: &str = r#"# Velo Player configuration
#
# 1. Go to https://developer.spotify.com/dashboard and create an app.
# 2. Open "Settings" on the app and add this exact Redirect URI:
#      http://127.0.0.1:8978/login
# 3. Paste the app's Client ID below.
client_id = ""

# Local port used to receive the OAuth redirect. If you change this, update
# the Redirect URI above to match.
redirect_port = 8978
"#;

/// XDG-ish locations Velo Player reads its config from and writes its
/// session cache to.
pub struct Paths {
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl Paths {
    pub fn discover() -> Self {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("velo-player");
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("velo-player");
        Self { config_dir, cache_dir }
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub client_id: String,
    #[serde(default = "default_redirect_port")]
    pub redirect_port: u16,
}

fn default_redirect_port() -> u16 {
    8978
}

impl Config {
    /// Loads `config.toml`, writing a template (with setup instructions) if
    /// it doesn't exist yet. Returns `Ok(None)` if the config still needs a
    /// `client_id` filled in, along with the path the user should edit.
    pub fn load_or_template(paths: &Paths) -> Result<Option<Config>> {
        let path = paths.config_file();

        if !path.exists() {
            std::fs::create_dir_all(&paths.config_dir)
                .with_context(|| format!("creating {}", paths.config_dir.display()))?;
            std::fs::write(&path, CONFIG_TEMPLATE)
                .with_context(|| format!("writing {}", path.display()))?;
            return Ok(None);
        }

        let data = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let config: Config =
            toml::from_str(&data).with_context(|| format!("parsing {}", path.display()))?;

        if config.client_id.trim().is_empty() {
            return Ok(None);
        }

        Ok(Some(config))
    }
}

/// Opens the on-disk cache used for persisted login credentials, mixer
/// volume, and downloaded audio chunks.
pub fn open_cache(paths: &Paths) -> Result<Cache> {
    let credentials = paths.cache_dir.join("credentials");
    let volume = paths.cache_dir.join("volume");
    let audio = paths.cache_dir.join("audio");

    // 2 GiB on-disk audio cache to avoid re-downloading recently played tracks.
    const AUDIO_CACHE_LIMIT: u64 = 2 * 1024 * 1024 * 1024;

    Cache::new(Some(credentials), Some(volume), Some(audio), Some(AUDIO_CACHE_LIMIT))
        .map_err(|e| anyhow!("failed to open cache: {e}"))
}

/// Connects `session`, reusing cached credentials if present and falling
/// back to an interactive OAuth login (opens the system's web browser).
pub async fn connect(session: &Session, cache: &Cache, config: &Config) -> Result<()> {
    if let Some(creds) = cache.credentials() {
        if session.connect(creds, true).await.is_ok() {
            return Ok(());
        }
    }

    let creds = oauth_login(config).await?;
    session
        .connect(creds, true)
        .await
        .map_err(|e| anyhow!("failed to connect to Spotify: {e}"))
}

/// Runs the OAuth authorization-code-with-PKCE flow, opening the user's
/// browser and listening on `redirect_port` for the callback.
async fn oauth_login(config: &Config) -> Result<Credentials> {
    let client_id = config.client_id.clone();
    let redirect_uri = format!("http://127.0.0.1:{}/login", config.redirect_port);

    // The OAuth client blocks the calling thread until the browser redirects
    // back, so it must run on a blocking-friendly thread.
    let token = tokio::task::spawn_blocking(move || {
        let client = OAuthClientBuilder::new(&client_id, &redirect_uri, SCOPES.to_vec())
            .open_in_browser()
            .build()
            .map_err(|e| anyhow!("failed to start OAuth flow: {e}"))?;

        client
            .get_access_token()
            .map_err(|e| anyhow!("OAuth login failed: {e}"))
    })
    .await
    .context("OAuth login task panicked")??;

    Ok(Credentials::with_access_token(token.access_token))
}
