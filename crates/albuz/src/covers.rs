//! Album covers: downloaded once and kept on disk, decoded off the main thread, and the
//! recently shown ones kept in memory.

use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
    rc::Rc,
    sync::OnceLock,
    time::{Duration, SystemTime},
};

use relm4::gtk::{self, gdk, glib, prelude::*};
use tokio::sync::Semaphore;

/// How many covers download at once.
const MAX_DOWNLOADS: usize = 6;

/// How long a cover stays on disk.
const MAX_AGE: Duration = Duration::from_hours(30 * 24);

/// How many decoded covers stay in memory.
const MEMORY_CAPACITY: usize = 200;

/// The cover at `url`, if it's in memory.
pub fn cached(url: &str) -> Option<gdk::Texture> {
    MEMORY.with_borrow_mut(|memory| memory.get(url))
}

/// The cover at `url`, from memory, from disk or downloaded.
///
/// Call it from the main thread.
pub async fn load(url: String) -> Option<gdk::Texture> {
    if let Some(texture) = cached(&url) {
        return Some(texture);
    }
    let fetched_url = url.clone();
    let texture = relm4::spawn(async move { fetch(&fetched_url).await })
        .await
        .ok()??;
    MEMORY.with_borrow_mut(|memory| memory.insert(url, texture.clone()));
    Some(texture)
}

struct Fetcher {
    http: reqwest::Client,
    dir: PathBuf,
    downloads: Semaphore,
}

static FETCHER: OnceLock<Fetcher> = OnceLock::new();

async fn fetch(url: &str) -> Option<gdk::Texture> {
    let fetcher = FETCHER.get_or_init(|| Fetcher {
        http: crate::account::http_client(),
        dir: cache_dir(),
        downloads: Semaphore::new(MAX_DOWNLOADS),
    });
    let path = file_name(url).map(|name| fetcher.dir.join(name));

    let cached = match &path {
        Some(path) => tokio::fs::read(path).await.ok(),
        None => None,
    };
    let bytes = if let Some(bytes) = cached {
        bytes
    } else {
        let bytes = download(fetcher, url).await?;
        if let Some(path) = &path
            && let Err(err) = save(path, &bytes).await
        {
            tracing::debug!("cannot cache cover {}: {err}", path.display());
        }
        bytes
    };

    tokio::task::spawn_blocking(move || gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes)))
        .await
        .ok()?
        .inspect_err(|err| tracing::debug!("cannot decode cover {url}: {err}"))
        .ok()
}

async fn download(fetcher: &Fetcher, url: &str) -> Option<Vec<u8>> {
    let _permit = fetcher.downloads.acquire().await.ok()?;
    let response = fetcher
        .http
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status);
    match response {
        Ok(response) => response.bytes().await.ok().map(Vec::from),
        Err(err) => {
            tracing::debug!("cannot download cover {url}: {err}");
            None
        }
    }
}

/// Write the file whole or not at all, so a crash never leaves a truncated cover behind.
async fn save(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let partial = path.with_extension("part");
    tokio::fs::write(&partial, bytes).await?;
    tokio::fs::rename(&partial, path).await
}

/// Delete the covers cached [`MAX_AGE`] ago or more, in the background.
///
/// Covers still in use are downloaded again; the others no longer take space.
pub fn prune_cache() {
    relm4::spawn_blocking(|| {
        let dir = cache_dir();
        match prune(&dir, MAX_AGE, SystemTime::now()) {
            Ok(0) => {}
            Ok(removed) => tracing::debug!("removed {removed} old covers from {}", dir.display()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => tracing::warn!("cannot clean up the cover cache: {err}"),
        }
    });
}

/// Delete the files in `dir` written `max_age` before `now` or earlier, and return how many.
///
/// Half-written `.part` files are deleted the same way, which leaves alone those still being
/// written and removes those a crash left behind.
fn prune(dir: &Path, max_age: Duration, now: SystemTime) -> std::io::Result<usize> {
    let mut removed = 0;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        // Files written in the future (a clock that moved back) count as new.
        let age = metadata
            .modified()
            .map(|written| now.duration_since(written).unwrap_or_default())?;
        if metadata.is_file() && age >= max_age {
            match std::fs::remove_file(entry.path()) {
                Ok(()) => removed += 1,
                // Someone else removed it already.
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(err),
            }
        }
    }
    Ok(removed)
}

fn cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .filter(|dir| !dir.is_empty())
        .map_or_else(
            || PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache"),
            PathBuf::from,
        )
        .join("albuz")
        .join("covers")
}

/// The cache file for a cover URL such as
/// `https://static.qobuz.com/images/covers/a3/t2/dpqdxzmrwt2a3_230.jpg`: `a3-t2-dpqdxzmrwt2a3_230.jpg`.
///
/// Qobuz never changes the image behind a URL, so the URL is enough to name it.
fn file_name(url: &str) -> Option<String> {
    let path = url.split_once("/images/covers/")?.1;
    let safe = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | '-');
    (!path.is_empty() && path.chars().all(safe) && !path.contains(".."))
        .then(|| path.replace('/', "-"))
}

/// Shows covers in a scrolled list or grid, loading only those on or near the screen.
///
/// List and grid views keep widgets for many more items than they show (a `GridView` keeps
/// up to 30 rows of its maximum column count), and all of them are bound and mapped, so
/// loading every bound cover would fetch and decode hundreds at once. Only the tiles on
/// screen are laid out though, so on every scroll or resize this checks where each tile is:
/// laid-out covers near the viewport load, and the others let go of their texture, which
/// keeps memory bounded however long the list.
#[derive(Clone, Default)]
pub struct VisibleCovers(Rc<RefCell<VisibleCoversInner>>);

#[derive(Default)]
struct VisibleCoversInner {
    viewport: Option<gtk::ScrolledWindow>,
    tiles: HashMap<gtk::Picture, Tile>,
    check_scheduled: bool,
}

struct Tile {
    url: String,
    /// Whether the cover is shown or loading.
    loaded: bool,
}

impl VisibleCovers {
    /// Watch `viewport`, the scrolled window around the tiles.
    pub fn attach(&self, viewport: &gtk::ScrolledWindow) {
        self.0.borrow_mut().viewport = Some(viewport.clone());
        let adjustment = viewport.vadjustment();
        // Scrolling.
        let this = self.clone();
        adjustment.connect_value_changed(move |_| this.schedule_check());
        // Resizing, or the content changing.
        let this = self.clone();
        adjustment.connect_changed(move |_| this.schedule_check());
        // Coming back on screen, as when switching tabs: the grid may have changed while
        // hidden, and nothing else would check it again before the next scroll.
        let this = self.clone();
        viewport.connect_map(move |_| this.schedule_check());
    }

    /// Show the cover at `url` in `picture` once it's near the screen.
    pub fn show(&self, picture: &gtk::Picture, url: String) {
        let cached = cached(&url);
        picture.set_paintable(cached.as_ref());
        let tile = Tile {
            url,
            loaded: cached.is_some(),
        };
        self.0.borrow_mut().tiles.insert(picture.clone(), tile);
        self.schedule_check();
    }

    /// Stop showing a cover in `picture`, as its tile was recycled.
    pub fn hide(&self, picture: &gtk::Picture) {
        self.0.borrow_mut().tiles.remove(picture);
        picture.set_paintable(None::<&gdk::Paintable>);
    }

    /// Check the tiles once layout is done, however many changes come before.
    fn schedule_check(&self) {
        if std::mem::replace(&mut self.0.borrow_mut().check_scheduled, true) {
            return;
        }
        let this = self.clone();
        glib::idle_add_local_once(move || this.check());
    }

    fn check(&self) {
        let mut inner = self.0.borrow_mut();
        inner.check_scheduled = false;
        let Some(viewport) = inner.viewport.clone() else {
            return;
        };
        #[expect(clippy::cast_precision_loss, reason = "screen sizes are small")]
        let screen = viewport.height() as f32;
        // Not shown yet, or hidden, as in another tab: its tiles aren't laid out, so they'd
        // all look far away and lose their covers. Showing it checks again.
        if screen <= 0.0 || !viewport.is_mapped() {
            return;
        }

        for (picture, tile) in &mut inner.tiles {
            // Tiles the view keeps around but doesn't lay out have no size.
            let laid_out = picture
                .compute_bounds(&viewport)
                .filter(|bounds| bounds.height() > 0.0);
            let (near, far) = match laid_out {
                Some(bounds) => {
                    let (top, bottom) = (bounds.y(), bounds.y() + bounds.height());
                    (
                        bottom >= -screen && top <= 2.0 * screen,
                        bottom < -3.0 * screen || top > 4.0 * screen,
                    )
                }
                None => (false, true),
            };

            if near && !tile.loaded {
                tile.loaded = true;
                let this = self.clone();
                let picture = picture.clone();
                let url = tile.url.clone();
                relm4::spawn_local(async move {
                    let texture = load(url.clone()).await;
                    // The tile may have been recycled or scrolled away meanwhile.
                    let still_wanted = this
                        .0
                        .borrow()
                        .tiles
                        .get(&picture)
                        .is_some_and(|tile| tile.loaded && tile.url == url);
                    if still_wanted {
                        picture.set_paintable(texture.as_ref());
                    }
                });
            } else if far && tile.loaded {
                tile.loaded = false;
                picture.set_paintable(None::<&gdk::Paintable>);
            }
        }
    }
}

thread_local! {
    static MEMORY: RefCell<Memory> = RefCell::default();
}

/// The covers used most recently.
#[derive(Default)]
struct Memory {
    textures: HashMap<String, (gdk::Texture, u64)>,
    clock: u64,
}

impl Memory {
    fn get(&mut self, url: &str) -> Option<gdk::Texture> {
        self.clock += 1;
        let (texture, used) = self.textures.get_mut(url)?;
        *used = self.clock;
        Some(texture.clone())
    }

    fn insert(&mut self, url: String, texture: gdk::Texture) {
        self.clock += 1;
        self.textures.insert(url, (texture, self.clock));
        if self.textures.len() > MEMORY_CAPACITY
            && let Some(oldest) = self
                .textures
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(url, _)| url.clone())
        {
            self.textures.remove(&oldest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_cache_files_after_the_cover_url() {
        assert_eq!(
            file_name("https://static.qobuz.com/images/covers/a3/t2/dpqdxzmrwt2a3_230.jpg")
                .as_deref(),
            Some("a3-t2-dpqdxzmrwt2a3_230.jpg")
        );
        assert_eq!(
            file_name("https://static.qobuz.com/images/covers/../x.jpg"),
            None
        );
        assert_eq!(file_name("https://example.com/cover.jpg"), None);
    }

    #[test]
    fn prunes_covers_cached_for_too_long() {
        let dir = std::env::temp_dir().join(format!("albuz-prune-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let now = SystemTime::now();
        let day = Duration::from_hours(24);
        let write = |name: &str, age: Duration| {
            let file = std::fs::File::create(dir.join(name)).unwrap();
            file.set_modified(now - age).unwrap();
        };
        write("fresh.jpg", day);
        write("old.jpg", 31 * day);
        write("just-expired.jpg", 30 * day);
        write("crashed.part", 40 * day);

        assert_eq!(prune(&dir, MAX_AGE, now).unwrap(), 3);
        let mut left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        left.sort();
        assert_eq!(left, ["fresh.jpg"]);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
