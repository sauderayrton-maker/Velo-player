use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use librespot::core::authentication::Credentials;
use librespot::core::cache::Cache;
use librespot::core::session::Session;
use librespot::oauth::{OAuthClientBuilder, OAuthToken};
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
#[derive(Clone)]
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

    fn oauth_token_file(&self) -> PathBuf {
        self.cache_dir.join("oauth_token.json")
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

/// An OAuth access token cached on disk, reused across runs and across the
/// librespot session and Web API calls alike. librespot's own keymaster
/// token endpoint (`session.token_provider()`) doesn't accept the access
/// tokens issued for custom OAuth client IDs, so Velo Player keeps its own
/// copy of the token returned by `librespot-oauth` and refreshes it as
/// needed instead.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct StoredToken {
    access_token: String,
    refresh_token: String,
    /// Unix timestamp (seconds) after which `access_token` should be refreshed.
    expires_at: u64,
}

impl StoredToken {
    fn from_oauth_token(token: &OAuthToken) -> Self {
        let remaining = token.expires_at.saturating_duration_since(Instant::now());
        Self {
            access_token: token.access_token.clone(),
            refresh_token: token.refresh_token.clone(),
            expires_at: unix_now() + remaining.as_secs(),
        }
    }

    /// Leaves a 30 second margin so a token doesn't expire mid-request.
    fn is_fresh(&self) -> bool {
        unix_now() + 30 < self.expires_at
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn load_token(paths: &Paths) -> Option<StoredToken> {
    let data = std::fs::read_to_string(paths.oauth_token_file()).ok()?;
    serde_json::from_str(&data).ok()
}

fn save_token(paths: &Paths, token: &StoredToken) -> Result<()> {
    std::fs::create_dir_all(&paths.cache_dir)
        .with_context(|| format!("creating {}", paths.cache_dir.display()))?;
    let data = serde_json::to_string(token).context("serializing OAuth token")?;
    let path = paths.oauth_token_file();
    std::fs::write(&path, data).with_context(|| format!("writing {}", path.display()))
}

/// Connects `session`, reusing a cached OAuth token (refreshing it if it has
/// expired) if present and falling back to an interactive OAuth login
/// (opens the system's web browser) otherwise.
pub async fn connect(session: &Session, paths: &Paths, config: &Config) -> Result<()> {
    if let Some(stored) = load_token(paths) {
        let stored = if stored.is_fresh() {
            Some(stored)
        } else {
            refresh_oauth_token(config, &stored.refresh_token).await.ok()
        };

        if let Some(stored) = stored {
            let creds = Credentials::with_access_token(stored.access_token.clone());
            if session.connect(creds, false).await.is_ok() {
                let _ = save_token(paths, &stored);
                return Ok(());
            }
        }
    }

    let (creds, token) = oauth_login(config).await?;
    session
        .connect(creds, false)
        .await
        .map_err(|e| anyhow!("failed to connect to Spotify: {e}"))?;
    save_token(paths, &token)
}

/// Returns a valid Web API access token, refreshing the cached OAuth token
/// first if it has expired.
pub async fn web_api_token(paths: &Paths, config: &Config) -> Result<String> {
    let stored = load_token(paths).ok_or_else(|| anyhow!("not logged in"))?;

    if stored.is_fresh() {
        return Ok(stored.access_token);
    }

    let refreshed = refresh_oauth_token(config, &stored.refresh_token).await?;
    save_token(paths, &refreshed)?;
    Ok(refreshed.access_token)
}

/// Runs the OAuth authorization-code-with-PKCE flow, opening the user's
/// browser and listening on `redirect_port` for the callback.
async fn oauth_login(config: &Config) -> Result<(Credentials, StoredToken)> {
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

    let stored = StoredToken::from_oauth_token(&token);
    let creds = Credentials::with_access_token(token.access_token);
    Ok((creds, stored))
}

/// Exchanges a refresh token for a new access token, without any browser
/// interaction.
async fn refresh_oauth_token(config: &Config, refresh_token: &str) -> Result<StoredToken> {
    let client_id = config.client_id.clone();
    let redirect_uri = format!("http://127.0.0.1:{}/login", config.redirect_port);
    let refresh_token = refresh_token.to_string();

    let token = tokio::task::spawn_blocking(move || {
        let client = OAuthClientBuilder::new(&client_id, &redirect_uri, SCOPES.to_vec())
            .build()
            .map_err(|e| anyhow!("failed to init OAuth client: {e}"))?;

        client
            .refresh_token(&refresh_token)
            .map_err(|e| anyhow!("OAuth token refresh failed: {e}"))
    })
    .await
    .context("OAuth refresh task panicked")??;

    Ok(StoredToken::from_oauth_token(&token))
}
