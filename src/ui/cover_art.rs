use std::io::Read;
use std::time::Duration;

use anyhow::{anyhow, Result};

/// Fetches `url` on a worker thread and, once the bytes are in, sets them as
/// `picture`'s paintable on the GTK main thread. Errors (including a missing
/// `url`) are silently ignored — the widget just keeps whatever placeholder
/// background its CSS class provides.
pub fn set_picture_from_url(picture: &gtk4::Picture, url: Option<String>) {
    // Clear any previous art immediately so stale covers don't linger
    // while the new one (if any) loads.
    picture.set_paintable(gtk4::gdk::Paintable::NONE);

    let Some(url) = url else { return };

    let (tx, rx) = async_channel::bounded::<Vec<u8>>(1);

    std::thread::spawn(move || {
        if let Ok(bytes) = fetch_image_bytes(&url) {
            let _ = tx.send_blocking(bytes);
        }
    });

    let picture = picture.clone();
    glib::MainContext::default().spawn_local(async move {
        if let Ok(bytes) = rx.recv().await {
            if let Ok(texture) = gtk4::gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes)) {
                picture.set_paintable(Some(&texture));
            }
        }
    });
}

fn fetch_image_bytes(url: &str) -> Result<Vec<u8>> {
    let response = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build()
        .get(url)
        .call()
        .map_err(|e| anyhow!("failed to fetch {url}: {e}"))?;

    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| anyhow!("failed to read {url}: {e}"))?;
    Ok(bytes)
}
