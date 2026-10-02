//! The Bakery: a canvas of the user's reference images, stored in the linked repo so agents
//! can read them.
//!
//! - Images: `docs/bakery/NN-<slug-of-label>.<ext>` (`NN` zero-padded, never reused).
//! - Layout: `.foundry/bakery.json`.
//! - `docs/bakery/manifest.md`: generated for agents; rewritten only when its content changes,
//!   and never by `move_item`.
//!
//! Every write goes through `core::repo_write`. Repos are found by project id in
//! `core::repos::RepoIndex`, which the projects feature keeps current.

pub mod commands;
mod images;
mod manifest;

use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::core::paths::{self, PathKind};
use crate::core::repos::RepoIndex;
use crate::core::slug::slugify;
use crate::core::{ids, repo_write, AppError, ErrorCode};

pub use images::{MAX_EDGE, MAX_INPUT_BYTES};

const LAYOUT_FILE: &str = ".foundry/bakery.json";
const IMAGE_DIR: &str = "docs/bakery";
const MANIFEST_FILE: &str = "docs/bakery/manifest.md";
const LAYOUT_VERSION: u32 = 1;
/// Default for `maxImages` in `bakery.json` (no command changes it yet).
pub const DEFAULT_MAX_IMAGES: u32 = 40;
const MAX_LABEL_CHARS: usize = 100;
/// Long labels still give readable file names.
const MAX_SLUG_CHARS: usize = 60;
/// A new card's long edge on the canvas, in canvas units.
const CARD_EDGE: f64 = 240.0;
/// Offset between cards dropped together, or placed with no drop point.
const CASCADE: f64 = 24.0;

/// A canvas coordinate or size. Always finite (commands reject NaN and infinities), so it
/// exports as a plain TS `number`; a bare `f64` would export as `number | null`.
// i32 stands in for the exported type only, as with EventMeta.ts.
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd, Serialize, Deserialize, Type)]
#[serde(transparent)]
#[specta(transparent)]
pub struct Coord(#[specta(type = i32)] pub f64);

/// One image on the canvas, as stored in `bakery.json` and sent to the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BakeryItem {
    pub id: String,
    /// File name inside `docs/bakery/`, e.g. `03-red-door.png`.
    pub file: String,
    pub label: String,
    /// What an agent should take from this image.
    pub take: String,
    /// What an agent should ignore in it.
    pub skip: String,
    pub tags: Vec<String>,
    /// Which agents or tasks it applies to.
    pub applies_to: Vec<String>,
    pub x: Coord,
    pub y: Coord,
    pub w: Coord,
    pub h: Coord,
    pub z: i32,
}

/// The whole Bakery, returned by every command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BakeryState {
    /// Ordered by `NN`.
    pub items: Vec<BakeryItem>,
    /// Images with no label or no Take note: agents can't act on them.
    pub unlabeled_count: u32,
    pub max_images: u32,
    /// Absolute path of `docs/bakery/`; an item's image is at `<dir>/<file>`.
    pub dir: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BakeryAddResult {
    /// Ids of the new items, in input order.
    pub added: Vec<String>,
    pub bakery: BakeryState,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
pub struct DropPoint {
    pub x: Coord,
    pub y: Coord,
}

/// Fields to change; `null` leaves a field as it is.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BakeryPatch {
    pub label: Option<String>,
    pub take: Option<String>,
    pub skip: Option<String>,
    pub tags: Option<Vec<String>>,
    pub applies_to: Option<Vec<String>>,
}

/// `.foundry/bakery.json`. Read through [`StoredLayout`] so hand-edited or older files with
/// missing fields still load.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Layout {
    version: u32,
    /// The next `NN`. Only ever grows, so numbers are never reused.
    next_number: u32,
    max_images: u32,
    items: Vec<BakeryItem>,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            version: LAYOUT_VERSION,
            next_number: 1,
            max_images: DEFAULT_MAX_IMAGES,
            items: Vec::new(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredLayout {
    #[serde(default)]
    next_number: u32,
    #[serde(default)]
    max_images: u32,
    #[serde(default)]
    items: Vec<StoredItem>,
}

/// A [`BakeryItem`] as read from disk: only `id` and `file` are required. (Defaults live here,
/// not on `BakeryItem`, so the exported type keeps every field required.)
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredItem {
    id: String,
    file: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    take: String,
    #[serde(default)]
    skip: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    applies_to: Vec<String>,
    #[serde(default)]
    x: Coord,
    #[serde(default)]
    y: Coord,
    #[serde(default = "default_edge")]
    w: Coord,
    #[serde(default = "default_edge")]
    h: Coord,
    #[serde(default)]
    z: i32,
}

fn default_edge() -> Coord {
    Coord(CARD_EDGE)
}

impl From<StoredItem> for BakeryItem {
    fn from(item: StoredItem) -> Self {
        Self {
            id: item.id,
            file: item.file,
            label: item.label,
            take: item.take,
            skip: item.skip,
            tags: item.tags,
            applies_to: item.applies_to,
            x: item.x,
            y: item.y,
            w: item.w,
            h: item.h,
            z: item.z,
        }
    }
}

/// A repo's `docs/bakery/` folder, built per component (no mixed separators on Windows).
pub fn image_dir(repo: &Path) -> PathBuf {
    repo.join("docs").join("bakery")
}

/// The `NN` a file name starts with (`u32::MAX` if none, so it sorts last).
pub(crate) fn file_number(file: &str) -> u32 {
    let digits: String = file.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().unwrap_or(u32::MAX)
}

pub(crate) fn is_unlabeled(item: &BakeryItem) -> bool {
    item.label.trim().is_empty() || item.take.trim().is_empty()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// A bare file name inside `docs/bakery/`; anything else (separators, `..`, a path) is
/// refused, so a hand-edited `bakery.json` can't point outside the folder.
fn image_rel(file: &str) -> Result<String, AppError> {
    let path = Path::new(file);
    let mut parts = path.components();
    let single = matches!(
        (parts.next(), parts.next()),
        (Some(Component::Normal(_)), None)
    );
    if !single || file.contains(['/', '\\', ':']) || file == "manifest.md" {
        return Err(AppError::path_escape(format!(
            "not a plain image file name: {file}"
        )));
    }
    Ok(format!("{IMAGE_DIR}/{file}"))
}

/// `NN-<slug>.<ext>`, `NN.<ext>` without a label, `NN-<slug>-<k>.<ext>` on a collision.
fn file_name(number: u32, label: &str, ext: &str, attempt: u32) -> String {
    let slug: String = slugify(label).chars().take(MAX_SLUG_CHARS).collect();
    let slug = slug.trim_end_matches('-');
    let mut name = format!("{number:02}");
    if !slug.is_empty() {
        name.push('-');
        name.push_str(slug);
    }
    if attempt > 1 {
        name.push_str(&format!("-{attempt}"));
    }
    format!("{name}.{ext}")
}

fn extension(file: &str) -> &str {
    Path::new(file)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("png")
}

fn clean_list(values: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for value in values {
        let value = value.trim().to_string();
        if !value.is_empty() && !out.contains(&value) {
            out.push(value);
        }
    }
    out
}

fn finite(name: &str, value: Coord) -> Result<Coord, AppError> {
    if value.0.is_finite() {
        Ok(value)
    } else {
        Err(AppError::invalid_input(format!(
            "{name} must be a finite number"
        )))
    }
}

fn positive(name: &str, value: Coord) -> Result<Coord, AppError> {
    if finite(name, value)?.0 > 0.0 {
        Ok(value)
    } else {
        Err(AppError::invalid_input(format!(
            "{name} must be greater than 0"
        )))
    }
}

/// A plain file name for a pasted image: no folders, no `..`.
fn check_paste_name(name: &str) -> Result<(), AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::invalid_input("pasted image needs a name"));
    }
    let rel = paths::check(Path::new(name), PathKind::Relative)?;
    if rel.components().count() != 1 || name.contains(['/', '\\', ':']) {
        return Err(AppError::path_escape(format!(
            "pasted image name must be a plain file name: {name}"
        )));
    }
    Ok(())
}

/// Called with a project's canonical `docs/bakery/` folder whenever a command touches it,
/// so the app can let the UI display its images (Tauri's asset protocol scope).
pub type DirHook = Box<dyn Fn(&Path) + Send + Sync>;

/// Owns Bakery writes for every project. Share it as `Arc<BakeryStore>`.
pub struct BakeryStore {
    repos: Arc<RepoIndex>,
    on_dir: Option<DirHook>,
    /// One lock per project, so concurrent adds get distinct numbers.
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl BakeryStore {
    pub fn new(repos: Arc<RepoIndex>) -> Self {
        Self {
            repos,
            on_dir: None,
            locks: Mutex::new(HashMap::new()),
        }
    }

    /// Sets the hook called with each project's `docs/bakery/` folder (see [`DirHook`]).
    pub fn with_dir_hook(mut self, hook: impl Fn(&Path) + Send + Sync + 'static) -> Self {
        self.on_dir = Some(Box::new(hook));
        self
    }

    fn project_lock(&self, project_id: &str) -> Result<Arc<Mutex<()>>, AppError> {
        let mut locks = self
            .locks
            .lock()
            .map_err(|_| AppError::internal("bakery locks are poisoned"))?;
        Ok(Arc::clone(locks.entry(project_id.to_string()).or_default()))
    }

    /// Runs `f` with the project's repo, holding its lock.
    fn with_repo<T>(
        &self,
        project_id: &str,
        f: impl FnOnce(&Path) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let lock = self.project_lock(project_id)?;
        let _guard = lock
            .lock()
            .map_err(|_| AppError::internal("bakery lock is poisoned"))?;
        let repo = self.repos.repo(project_id)?;
        let result = f(&repo)?;
        if let Some(hook) = &self.on_dir {
            // Canonical, because the asset scope compares against canonicalized requests.
            // Missing until the first image is added; nothing to show before that.
            if let Ok(dir) = image_dir(&repo).canonicalize() {
                hook(&dir);
            }
        }
        Ok(result)
    }

    pub fn list(&self, project_id: &str) -> Result<BakeryState, AppError> {
        self.with_repo(project_id, |repo| Ok(state(repo, &load(repo)?)))
    }

    /// Adds dropped files. All or nothing: if any file is refused, nothing is added.
    pub fn add_from_paths(
        &self,
        project_id: &str,
        paths: &[String],
        drop_point: Option<DropPoint>,
    ) -> Result<BakeryAddResult, AppError> {
        if paths.is_empty() {
            return Err(AppError::invalid_input("no files to add"));
        }
        // Decode and resize before taking the lock: it's the slow part.
        let processed = paths
            .iter()
            .map(|path| read_input(path).and_then(|bytes| images::process(&bytes)))
            .collect::<Result<Vec<_>, _>>()?;
        self.add(project_id, processed, drop_point)
    }

    /// Adds a pasted image. `name` must be a plain file name; the format comes from the bytes.
    pub fn add_from_bytes(
        &self,
        project_id: &str,
        name: &str,
        bytes: &[u8],
    ) -> Result<BakeryAddResult, AppError> {
        check_paste_name(name)?;
        let processed = images::process(bytes)?;
        self.add(project_id, vec![processed], None)
    }

    fn add(
        &self,
        project_id: &str,
        processed: Vec<images::Processed>,
        drop_point: Option<DropPoint>,
    ) -> Result<BakeryAddResult, AppError> {
        if let Some(point) = drop_point {
            finite("dropPoint.x", point.x)?;
            finite("dropPoint.y", point.y)?;
        }
        self.with_repo(project_id, |repo| {
            let mut layout = load(repo)?;
            let wanted = layout.items.len() + processed.len();
            if wanted > layout.max_images as usize {
                return Err(AppError::new(
                    ErrorCode::BakeryFull,
                    format!(
                        "the Bakery holds at most {} images ({} now)",
                        layout.max_images,
                        layout.items.len()
                    ),
                ));
            }

            let start = drop_point.unwrap_or_else(|| {
                let step = (layout.items.len() % 10) as f64 * CASCADE;
                DropPoint {
                    x: Coord(step),
                    y: Coord(step),
                }
            });
            let top = layout.items.iter().map(|item| item.z).max().unwrap_or(0);
            let mut written: Vec<String> = Vec::new();
            let mut added = Vec::new();
            for (k, image) in processed.iter().enumerate() {
                let file = match write_new_image(repo, &mut layout, image) {
                    Ok(file) => file,
                    Err(err) => {
                        discard(repo, &written);
                        return Err(err);
                    }
                };
                written.push(file.clone());
                let scale = CARD_EDGE / f64::from(image.width.max(image.height).max(1));
                let offset = k as f64 * CASCADE;
                let item = BakeryItem {
                    id: ids::new_id(),
                    file,
                    label: String::new(),
                    take: String::new(),
                    skip: String::new(),
                    tags: Vec::new(),
                    applies_to: Vec::new(),
                    x: Coord(start.x.0 + offset),
                    y: Coord(start.y.0 + offset),
                    w: Coord((f64::from(image.width) * scale).round().max(1.0)),
                    h: Coord((f64::from(image.height) * scale).round().max(1.0)),
                    z: top.saturating_add(i32::try_from(k + 1).unwrap_or(i32::MAX)),
                };
                added.push(item.id.clone());
                layout.items.push(item);
            }

            if let Err(err) = save(repo, &layout).and_then(|()| write_manifest(repo, &layout)) {
                discard(repo, &written);
                return Err(err);
            }
            tracing::info!(project_id, added = added.len(), "bakery images added");
            Ok(BakeryAddResult {
                added,
                bakery: state(repo, &layout),
            })
        })
    }

    /// Changes notes; a label change renames the file (`-2`, `-3`… on a name collision).
    pub fn update(
        &self,
        project_id: &str,
        id: &str,
        patch: BakeryPatch,
    ) -> Result<BakeryState, AppError> {
        if let Some(label) = &patch.label {
            if label.trim().chars().count() > MAX_LABEL_CHARS {
                return Err(AppError::invalid_input(format!(
                    "label must be {MAX_LABEL_CHARS} characters or fewer"
                )));
            }
        }
        self.with_repo(project_id, |repo| {
            let mut layout = load(repo)?;
            let index = find(&layout, id)?;
            let old_file = layout.items[index].file.clone();
            let item = &mut layout.items[index];
            if let Some(label) = patch.label {
                item.label = label.trim().to_string();
            }
            if let Some(take) = patch.take {
                item.take = take.trim().to_string();
            }
            if let Some(skip) = patch.skip {
                item.skip = skip.trim().to_string();
            }
            if let Some(tags) = patch.tags {
                item.tags = clean_list(tags);
            }
            if let Some(applies_to) = patch.applies_to {
                item.applies_to = clean_list(applies_to);
            }

            let number = file_number(&old_file);
            let ext = extension(&old_file).to_string();
            let label = item.label.clone();
            let renamed = if file_name(number, &label, &ext, 1) == old_file {
                None
            } else {
                Some(rename_image(repo, &old_file, number, &label, &ext)?)
            };
            if let Some(new_file) = &renamed {
                layout.items[index].file = new_file.clone();
            }

            if let Err(err) = save(repo, &layout) {
                if let Some(new_file) = &renamed {
                    // Put the file back so the old layout still matches the disk.
                    let _ =
                        repo_write::rename_new(repo, &image_rel(new_file)?, &image_rel(&old_file)?);
                }
                return Err(err);
            }
            write_manifest(repo, &layout)?;
            tracing::info!(
                project_id,
                id,
                renamed = renamed.is_some(),
                "bakery item updated"
            );
            Ok(state(repo, &layout))
        })
    }

    /// Moves, resizes or restacks a card. Layout only: never touches `manifest.md`.
    #[allow(clippy::too_many_arguments)]
    pub fn move_item(
        &self,
        project_id: &str,
        id: &str,
        x: Coord,
        y: Coord,
        w: Option<Coord>,
        h: Option<Coord>,
        z: Option<i32>,
    ) -> Result<BakeryState, AppError> {
        let x = finite("x", x)?;
        let y = finite("y", y)?;
        let w = w.map(|w| positive("w", w)).transpose()?;
        let h = h.map(|h| positive("h", h)).transpose()?;
        self.with_repo(project_id, |repo| {
            let mut layout = load(repo)?;
            let index = find(&layout, id)?;
            let item = &mut layout.items[index];
            item.x = x;
            item.y = y;
            if let Some(w) = w {
                item.w = w;
            }
            if let Some(h) = h {
                item.h = h;
            }
            if let Some(z) = z {
                item.z = z;
            }
            save(repo, &layout)?;
            tracing::debug!(project_id, id, "bakery item moved");
            Ok(state(repo, &layout))
        })
    }

    /// Removes the item and deletes its image. Deletes only inside `docs/bakery/`.
    pub fn remove(&self, project_id: &str, id: &str) -> Result<BakeryState, AppError> {
        self.with_repo(project_id, |repo| {
            let mut layout = load(repo)?;
            let index = find(&layout, id)?;
            // Check before changing anything, so a bad name in bakery.json removes nothing.
            let rel = image_rel(&layout.items[index].file)?;
            layout.items.remove(index);
            save(repo, &layout)?;
            write_manifest(repo, &layout)?;
            repo_write::remove(repo, &rel)?;
            tracing::info!(project_id, id, "bakery item removed");
            Ok(state(repo, &layout))
        })
    }
}

fn find(layout: &Layout, id: &str) -> Result<usize, AppError> {
    layout
        .items
        .iter()
        .position(|item| item.id == id)
        .ok_or_else(|| AppError::not_found(format!("no Bakery image with id {id}")))
}

fn read_input(path: &str) -> Result<Vec<u8>, AppError> {
    let path = paths::check(Path::new(path), PathKind::Absolute)?;
    let meta = fs::metadata(&path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => {
            AppError::not_found(format!("file does not exist: {}", path.display()))
        }
        _ => err.into(),
    })?;
    if !meta.is_file() {
        return Err(AppError::invalid_path(format!(
            "not a file: {}",
            path.display()
        )));
    }
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    if meta.len() > MAX_INPUT_BYTES {
        return Err(images::too_large(&name));
    }
    Ok(fs::read(&path)?)
}

/// Writes a new image under the next free `NN`, never replacing an existing file.
fn write_new_image(
    repo: &Path,
    layout: &mut Layout,
    image: &images::Processed,
) -> Result<String, AppError> {
    // A stray file can hold a number: skip it rather than reuse or replace.
    for _ in 0..1000 {
        let number = layout.next_number;
        layout.next_number = number
            .checked_add(1)
            .ok_or_else(|| AppError::internal("bakery numbers exhausted"))?;
        let file = file_name(number, "", image.ext, 1);
        if repo_write::write_if_missing(repo, &image_rel(&file)?, &image.bytes)? {
            return Ok(file);
        }
    }
    Err(AppError::internal("could not find a free bakery file name"))
}

/// Renames to `NN-<slug>.<ext>`, adding `-2`, `-3`… while the name is taken.
fn rename_image(
    repo: &Path,
    old_file: &str,
    number: u32,
    label: &str,
    ext: &str,
) -> Result<String, AppError> {
    let from = image_rel(old_file)?;
    for attempt in 1..=100 {
        let candidate = file_name(number, label, ext, attempt);
        if candidate == old_file {
            return Ok(candidate);
        }
        if repo_write::rename_new(repo, &from, &image_rel(&candidate)?)? {
            return Ok(candidate);
        }
    }
    Err(AppError::internal(format!(
        "could not find a free name for {old_file}"
    )))
}

fn discard(repo: &Path, files: &[String]) {
    for file in files {
        if let Ok(rel) = image_rel(file) {
            if let Err(err) = repo_write::remove(repo, &rel) {
                tracing::warn!(error = %err, "could not clean up a new bakery image");
            }
        }
    }
}

fn load(repo: &Path) -> Result<Layout, AppError> {
    let Some(text) = repo_write::read_to_string(repo, LAYOUT_FILE)? else {
        return Ok(Layout::default());
    };
    match serde_json::from_str::<StoredLayout>(&text) {
        Ok(stored) => {
            let mut layout = Layout {
                version: LAYOUT_VERSION,
                next_number: stored.next_number,
                max_images: stored.max_images,
                items: stored.items.into_iter().map(BakeryItem::from).collect(),
            };
            // Never hand out a number a file already has, whatever the file says.
            let highest = layout
                .items
                .iter()
                .map(|item| file_number(&item.file))
                .filter(|&n| n != u32::MAX)
                .max()
                .unwrap_or(0);
            layout.next_number = layout.next_number.max(highest.saturating_add(1)).max(1);
            if layout.max_images == 0 {
                layout.max_images = DEFAULT_MAX_IMAGES;
            }
            layout.items.sort_by_key(|item| file_number(&item.file));
            Ok(layout)
        }
        Err(err) => {
            // Kept for recovery, never deleted; start from an empty canvas.
            let aside = format!("{LAYOUT_FILE}.corrupt-{}", now_ms());
            tracing::warn!(error = %err, moved_to = %aside, "unreadable bakery.json; starting fresh");
            repo_write::rename_new(repo, LAYOUT_FILE, &aside)?;
            Ok(Layout::default())
        }
    }
}

fn save(repo: &Path, layout: &Layout) -> Result<(), AppError> {
    let mut json = serde_json::to_string_pretty(layout)
        .map_err(|err| AppError::internal(format!("serialize bakery.json: {err}")))?;
    json.push('\n');
    repo_write::write(repo, LAYOUT_FILE, json)
}

/// Writes the manifest only when its content changed, so unchanged state never touches it.
fn write_manifest(repo: &Path, layout: &Layout) -> Result<(), AppError> {
    let text = manifest::render(&layout.items);
    if repo_write::read_to_string(repo, MANIFEST_FILE)?.as_deref() == Some(text.as_str()) {
        return Ok(());
    }
    repo_write::write(repo, MANIFEST_FILE, text)
}

fn state(repo: &Path, layout: &Layout) -> BakeryState {
    let unlabeled = layout
        .items
        .iter()
        .filter(|item| is_unlabeled(item))
        .count();
    BakeryState {
        items: layout.items.clone(),
        unlabeled_count: u32::try_from(unlabeled).unwrap_or(u32::MAX),
        max_images: layout.max_images,
        dir: PathBuf::from(repo)
            .join(IMAGE_DIR)
            .to_string_lossy()
            .into_owned(),
    }
}

#[cfg(test)]
mod tests;
