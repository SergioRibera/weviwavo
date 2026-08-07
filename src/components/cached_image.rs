use std::path::PathBuf;
use std::sync::OnceLock;

use bytes::Bytes;
use freya::prelude::*;
use reqwest::Url;

static HTTP: OnceLock<reqwest::Client> = OnceLock::new();

fn http() -> &'static reqwest::Client {
    HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default()
    })
}

/// `~/.cache/weviwavo/thumbs/<sha1_of_url>`
pub(crate) fn thumb_path(url: &str) -> PathBuf {
    let hash = sha1_smol::Sha1::from(url.as_bytes()).hexdigest();
    crate::utils::cache_dir(&["thumbs", &hash])
}

/// Synchronous version for use inside iterators/non-component contexts.
///
/// - Disk hit  → `Some(ImageSource::Path(...))` immediately (no network)
/// - Disk miss → `Some(ImageSource::Uri(...))` (Freya fetches from network as before)
/// - Empty/invalid URL → `None`
///
/// Does NOT download or save to disk. Use alongside [`use_cached_thumbnail`] in
/// components that do: thumbnails saved there will be found here next session.
pub fn cached_image_source(url: &str) -> Option<ImageSource> {
    if url.is_empty() {
        return None;
    }
    let path = thumb_path(url);
    if path.exists() {
        Some(ImageSource::Path(path))
    } else {
        Url::parse(url).ok().map(ImageSource::Uri)
    }
}

/// Hook that returns a cached `ImageSource` for a thumbnail URL.
///
/// - Disk hit  → `Some(ImageSource::Path(...))` on first render (no network)
/// - Disk miss → `None` until the download completes, then `Some(ImageSource::Bytes(...))`
///   and the bytes are persisted for future sessions
/// - Empty URL → always `None`
pub fn use_cached_thumbnail(url: &str) -> Option<ImageSource> {
    let url = url.to_owned();
    let mut source = use_state(|| Option::<ImageSource>::None);

    use_side_effect_with_deps(&url, move |url| {
        if url.is_empty() {
            return;
        }

        let url = url.clone();
        let path = thumb_path(&url);

        spawn_forever(async move {
            // Fast path: already on disk from a previous session.
            let on_disk = tokio::task::spawn_blocking({
                let p = path.clone();
                move || p.exists()
            })
            .await
            .unwrap_or(false);

            if on_disk {
                source.set(Some(ImageSource::Path(path)));
                return;
            }

            // Download.
            let bytes: Bytes = match async {
                http()
                    .get(&url)
                    .send()
                    .await?
                    .error_for_status()?
                    .bytes()
                    .await
            }
            .await
            {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(url, error = %e, "thumbnail download failed");
                    return;
                }
            };

            // Persist to disk in background.
            let to_write = bytes.clone();
            let write_path = path.clone();
            tokio::task::spawn_blocking(move || {
                if let Some(parent) = write_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(e) = std::fs::write(&write_path, &to_write) {
                    tracing::warn!(error = %e, "failed to persist thumbnail");
                }
            });

            source.set(Some((url, bytes).into()));
        });
    });

    source.read().clone()
}
