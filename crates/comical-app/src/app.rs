//! Application state and book loading for the comical viewer.
//!
//! [`App`] holds everything the UI needs: the currently open book, the
//! [`SpreadState`] navigation model, decoded-page and texture caches, and view
//! settings (zoom, pan, fullscreen).

use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use comical_archive::{ComicArchive, DEFAULT_MAX_ENTRY_BYTES};
use comical_core::{
    Direction, MAX_CUSTOM_PERCENT, MIN_CUSTOM_PERCENT, PageEntry, PageOrientation, PagePart,
    PageRef, Rotation, Size, Spread, SpreadState, ViewMode, ZoomMode, ZoomOptions,
};
use comical_image::{Filter, RgbImage, decode, probe_size, resize, rotate as image_rotate};
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use eframe::egui::{ColorImage, Context, TextureHandle, TextureOptions};
use lru::LruCache;

use crate::error::AppError;
use crate::settings::Settings;

/// Number of decoded pages and uploaded textures kept in memory.
const CACHE_PAGES: usize = 10;

/// Longest side of an uploaded page texture, in pixels.
///
/// Keeping textures independent of the on-screen size makes window resizing
/// free: the GPU scales the cached texture instead of asking us to decode and
/// resample the page again for every new window size. Most comic pages are
/// below this limit and are uploaded at their natural size.
const TEXTURE_MAX_SIDE: u32 = 4096;

/// Books whose last page is kept when `remember_page` is on.
const MAX_REMEMBERED_BOOKS: usize = 200;

/// Number of thumbnail requests that may wait for the thumbnail worker.
const THUMB_QUEUE: usize = 32;

/// Identity of one uploaded texture.
///
/// The size is deliberately absent. A texture is decoded at its natural (or
/// capped) size once, and the GPU scales it for whatever size the current
/// window and zoom need.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TextureKey {
    page: usize,
    part: PagePart,
    rotation: Rotation,
    filter: Filter,
}

/// Work sent to the background decode thread.
#[derive(Debug, Clone)]
struct DecodeRequest {
    key: TextureKey,
    generation: u64,
}

/// Completed work returned from the background decode thread.
struct DecodeResult {
    key: TextureKey,
    generation: u64,
    image: ColorImage,
    name: String,
    /// An encrypted entry needed a password the prompt has not asked for yet.
    password_required: bool,
}

/// Work sent to the background thumbnail thread.
#[derive(Debug, Clone, Copy)]
struct ThumbRequest {
    page: usize,
}

/// Completed thumbnail, or `None` when the page could not be decoded.
struct ThumbResult {
    page: usize,
    image: Option<ColorImage>,
    name: String,
}

/// A currently open book and its per-book caches.
struct OpenedBook {
    /// Page entries in book order.
    pages: Vec<PageEntry>,
    /// The archive handle (keeps it alive). Wrapped in Mutex because
    /// `read_page` requires `&mut self` but we share it with the decode worker.
    archive: Arc<Mutex<Box<dyn ComicArchive + Send>>>,
    /// Natural full-page size, filled on the first successful decode.
    page_sizes: Vec<Option<Size>>,
    /// Per-page error message, so a broken page is not retried every frame.
    page_errors: Vec<Option<String>>,
    /// Decoded pages at natural size, keyed by page index.
    natural: LruCache<usize, RgbImage>,
    /// Uploaded page textures, keyed by page, part and rotation.
    textures: LruCache<TextureKey, TextureHandle>,
    /// Sends decode requests to the background worker.
    decode_tx: Sender<DecodeRequest>,
    /// Receives completed decodes from the background worker.
    decode_rx: Receiver<DecodeResult>,
    /// Keys currently being decoded, to avoid duplicate requests.
    pending: std::collections::HashSet<TextureKey>,
    /// Generation used to discard decode requests made obsolete by navigation.
    generation: Arc<AtomicU64>,
    /// One-pixel black texture stretched under a page that is still decoding.
    placeholder: Option<TextureHandle>,
    /// Uploaded thumbnails for the browser panel.
    thumb_cache: crate::browser::ThumbnailCache,
    /// Pages whose thumbnail is being decoded.
    thumb_pending: std::collections::HashSet<usize>,
    /// Sends thumbnail requests to the background thumbnail worker.
    thumb_tx: Sender<ThumbRequest>,
    /// Receives completed thumbnails from the background worker.
    thumb_rx: Receiver<ThumbResult>,
    /// Resampling filter for pages that have to be scaled.
    filter: Filter,
    /// Set when a background decode reported an encrypted entry that still
    /// needs a password.
    password_required: bool,
}

impl OpenedBook {
    fn new(archive: Box<dyn ComicArchive + Send>, filter: Filter) -> Self {
        let pages = archive.pages().to_vec();
        let count = pages.len();
        let (decode_tx, worker_rx) = unbounded::<DecodeRequest>();
        let (worker_tx, decode_rx) = unbounded::<DecodeResult>();
        let generation = Arc::new(AtomicU64::new(0));
        let archive = Arc::new(Mutex::new(archive));

        let worker_archive = Arc::clone(&archive);
        let worker_generation = Arc::clone(&generation);
        thread::Builder::new()
            .name("comical-decode".to_owned())
            .spawn(move || {
                while let Ok(req) = worker_rx.recv() {
                    // A page turn makes older requests useless. Skipping them
                    // here keeps the visible page from waiting behind a queue
                    // of stale decodes.
                    if req.generation != worker_generation.load(Ordering::Relaxed) {
                        continue;
                    }
                    let result = decode_request(&worker_archive, &req);
                    if worker_tx.send(result).is_err() {
                        break;
                    }
                }
            })
            .expect("failed to spawn decode worker");

        let (thumb_tx, thumb_worker_rx) = bounded::<ThumbRequest>(THUMB_QUEUE);
        let (thumb_worker_tx, thumb_rx) = unbounded::<ThumbResult>();
        let thumb_archive = Arc::clone(&archive);
        thread::Builder::new()
            .name("comical-thumb".to_owned())
            .spawn(move || {
                while let Ok(req) = thumb_worker_rx.recv() {
                    let result = decode_thumbnail(&thumb_archive, req.page);
                    if thumb_worker_tx.send(result).is_err() {
                        break;
                    }
                }
            })
            .expect("failed to spawn thumbnail worker");

        Self {
            pages,
            archive,
            page_sizes: vec![None; count],
            page_errors: vec![None; count],
            natural: LruCache::new(cache_capacity()),
            textures: LruCache::new(cache_capacity()),
            decode_tx,
            decode_rx,
            pending: std::collections::HashSet::new(),
            generation,
            placeholder: None,
            thumb_cache: crate::browser::ThumbnailCache::new(),
            thumb_pending: std::collections::HashSet::new(),
            thumb_tx,
            thumb_rx,
            filter,
            password_required: false,
        }
    }

    /// Discards decode work for pages that are no longer visible.
    fn cancel_pending(&mut self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.pending.clear();
    }

    /// Returns the natural full-page size, reading only the image header.
    ///
    /// The full image is decoded later on the background worker. Parsing the
    /// header is fast enough to lay out the first spread without blocking the
    /// UI on a full-resolution decode.
    fn natural_size(&mut self, page: usize) -> Result<Size, AppError> {
        if let Some(Some(size)) = self.page_sizes.get(page) {
            return Ok(*size);
        }

        let bytes = self.read_page_bytes(page)?;
        let (width, height) = match probe_size(&bytes) {
            Ok(size) => size,
            Err(error) => {
                let message = error.to_string();
                self.record_page_error(page, message.clone());
                return Err(AppError::Page { page, message });
            }
        };
        let size = Size::new(width, height);
        if let Some(slot) = self.page_sizes.get_mut(page) {
            *slot = Some(size);
        }
        Ok(size)
    }

    /// Reads one page's encoded bytes, recording a per-page error on failure.
    fn read_page_bytes(&mut self, page: usize) -> Result<Vec<u8>, AppError> {
        if let Some(Some(message)) = self.page_errors.get(page) {
            return Err(AppError::Page {
                page,
                message: message.clone(),
            });
        }

        let mut guard = self.archive.lock().unwrap();
        match guard.read_page(page, DEFAULT_MAX_ENTRY_BYTES) {
            Ok(bytes) => Ok(bytes),
            Err(error) => {
                let message = error.to_string();
                drop(guard);
                self.record_page_error(page, message.clone());
                Err(AppError::Page { page, message })
            }
        }
    }

    /// Returns the oriented size after applying rotation.
    fn oriented_size(&mut self, page: usize, rotation: Rotation) -> Result<Size, AppError> {
        let natural = self.natural_size(page)?;
        if rotation.swaps_dimensions() {
            Ok(Size::new(natural.height, natural.width))
        } else {
            Ok(natural)
        }
    }

    /// Returns the decoded page at natural size, reading it on first use.
    fn natural(&mut self, page: usize) -> Result<&RgbImage, AppError> {
        if self.natural.contains(&page) {
            return Ok(self
                .natural
                .get(&page)
                .expect("page was found in the natural cache"));
        }

        let bytes = self.read_page_bytes(page)?;
        let image = match decode(&bytes) {
            Ok(image) => image,
            Err(error) => {
                let message = error.to_string();
                self.record_page_error(page, message.clone());
                return Err(AppError::Page { page, message });
            }
        };

        self.natural.put(page, image);
        self.natural.get(&page).ok_or_else(|| AppError::Page {
            page,
            message: "decoded page was not cached".to_owned(),
        })
    }

    fn record_page_error(&mut self, page: usize, message: String) {
        if let Some(slot) = self.page_errors.get_mut(page) {
            *slot = Some(message);
        }
    }

    /// Returns an uploaded texture for one visible part of a page.
    ///
    /// Cache hits are instant. Misses queue a background decode and return a
    /// one-pixel placeholder, stretched to the page rectangle, so the UI
    /// thread never blocks on I/O, decode or resampling.
    fn texture(
        &mut self,
        ctx: &Context,
        page: usize,
        part: PagePart,
        rotation: Rotation,
    ) -> Result<TextureHandle, AppError> {
        let key = TextureKey {
            page,
            part,
            rotation,
            filter: self.filter,
        };
        if let Some(handle) = self.textures.get(&key) {
            return Ok(handle.clone());
        }
        if let Some(Some(message)) = self.page_errors.get(page) {
            return Err(AppError::Page {
                page,
                message: message.clone(),
            });
        }

        if !self.pending.contains(&key) {
            let request = DecodeRequest {
                key,
                generation: self.generation.load(Ordering::Relaxed),
            };
            if self.decode_tx.send(request).is_ok() {
                self.pending.insert(key);
            }
        }

        Ok(self.placeholder_texture(ctx))
    }

    /// Returns a cached thumbnail, queueing a background decode on a miss.
    fn thumbnail(&mut self, page: usize) -> Option<TextureHandle> {
        if let Some(handle) = self.thumb_cache.get(page) {
            return Some(handle);
        }

        if !self.thumb_pending.contains(&page) {
            match self.thumb_tx.try_send(ThumbRequest { page }) {
                Ok(()) => {
                    self.thumb_pending.insert(page);
                }
                Err(_) => {
                    // The queue is full. The next frame will try again once a
                    // slot frees up.
                }
            }
        }
        None
    }

    /// Whether any background decode or thumbnail is still outstanding.
    fn has_pending(&self) -> bool {
        !self.pending.is_empty() || !self.thumb_pending.is_empty()
    }

    /// Returns the one-pixel black placeholder used while a page decodes.
    fn placeholder_texture(&mut self, ctx: &Context) -> TextureHandle {
        if let Some(handle) = &self.placeholder {
            return handle.clone();
        }
        let image = ColorImage::from_rgb([1, 1], &[0, 0, 0]);
        let handle = ctx.load_texture("placeholder-black", image, TextureOptions::LINEAR);
        self.placeholder = Some(handle.clone());
        handle
    }

    /// Drain completed background decodes and upload them as GPU textures.
    fn drain_decoded(&mut self, ctx: &Context) -> bool {
        let mut uploaded = false;
        while let Ok(result) = self.decode_rx.try_recv() {
            self.pending.remove(&result.key);
            if result.generation != self.generation.load(Ordering::Relaxed) {
                // A page turn made this result obsolete. Uploading it would
                // stall the frame for a page the reader has already left.
                continue;
            }
            if result.password_required {
                self.password_required = true;
            }
            let handle = ctx.load_texture(result.name, result.image, TextureOptions::LINEAR);
            self.textures.put(result.key, handle);
            uploaded = true;
        }
        while let Ok(result) = self.thumb_rx.try_recv() {
            self.thumb_pending.remove(&result.page);
            let image = result.image.unwrap_or_else(|| {
                // A failed thumbnail still needs a cache entry, otherwise the
                // browser would request the same broken page every frame.
                ColorImage::from_rgb([1, 1], &[24, 24, 24])
            });
            let handle = ctx.load_texture(result.name, image, TextureOptions::LINEAR);
            self.thumb_cache.put(result.page, handle);
            uploaded = true;
        }
        uploaded
    }
}

/// Background worker function: reads archive, decodes, rotates and crops.
///
/// The result is capped at [`TEXTURE_MAX_SIDE`] but never scaled to the current
/// window size, so window resizes reuse the same texture.
fn decode_request(
    archive: &Arc<Mutex<Box<dyn ComicArchive + Send>>>,
    req: &DecodeRequest,
) -> DecodeResult {
    let name = format!(
        "page-{}-{:?}-{:?}",
        req.key.page, req.key.part, req.key.rotation
    );

    let bytes = match archive
        .lock()
        .unwrap()
        .read_page(req.key.page, DEFAULT_MAX_ENTRY_BYTES)
    {
        Ok(bytes) => bytes,
        Err(error) => {
            // Encrypted entries list without a password but fail at read
            // time; report that so the app can offer the password prompt.
            return placeholder_result(req, name, error.is_password_required());
        }
    };

    let decoded = match decode(&bytes) {
        Ok(image) => image,
        Err(_) => return placeholder_result(req, name, false),
    };

    let rotated = if req.key.rotation != Rotation::North {
        image_rotate(&decoded, rotation_to_image(req.key.rotation))
    } else {
        decoded
    };

    let image = if req.key.part == PagePart::Full {
        rotated
    } else {
        crop_part(&rotated, req.key.part).unwrap_or(rotated)
    };

    let (width, height) = capped_dimensions(image.width(), image.height());
    let image = if width == image.width() && height == image.height() {
        image
    } else {
        resize(&image, width, height, req.key.filter).unwrap_or(image)
    };

    let size = [image.width() as usize, image.height() as usize];
    DecodeResult {
        key: req.key,
        generation: req.generation,
        image: ColorImage::from_rgb(size, image.pixels()),
        name,
        password_required: false,
    }
}

fn placeholder_result(req: &DecodeRequest, name: String, password_required: bool) -> DecodeResult {
    DecodeResult {
        key: req.key,
        generation: req.generation,
        image: ColorImage::from_rgb([1, 1], &[0, 0, 0]),
        name,
        password_required,
    }
}

/// Scales a dimension down only when it exceeds [`TEXTURE_MAX_SIDE`].
fn capped_dimensions(width: u32, height: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= TEXTURE_MAX_SIDE {
        return (width, height);
    }
    let scale = f64::from(TEXTURE_MAX_SIDE) / f64::from(longest);
    (
        ((f64::from(width) * scale).round() as u32).max(1),
        ((f64::from(height) * scale).round() as u32).max(1),
    )
}

/// Background worker function: reads one page and builds an 80 px thumbnail.
fn decode_thumbnail(
    archive: &Arc<Mutex<Box<dyn ComicArchive + Send>>>,
    page: usize,
) -> ThumbResult {
    let name = format!("thumb-{page}");
    let bytes = match archive
        .lock()
        .unwrap()
        .read_page(page, DEFAULT_MAX_ENTRY_BYTES)
    {
        Ok(bytes) => bytes,
        Err(_) => {
            return ThumbResult {
                page,
                image: None,
                name,
            };
        }
    };
    let decoded = match decode(&bytes) {
        Ok(image) => image,
        Err(_) => {
            return ThumbResult {
                page,
                image: None,
                name,
            };
        }
    };

    let (width, height) = crate::browser::thumbnail_dimensions(decoded.width(), decoded.height());
    let thumb = resize(&decoded, width, height, Filter::Box).unwrap_or(decoded);
    let size = [thumb.width() as usize, thumb.height() as usize];
    ThumbResult {
        page,
        image: Some(ColorImage::from_rgb(size, thumb.pixels())),
        name,
    }
}

/// Converts core Rotation to image Rotation.
fn rotation_to_image(rotation: Rotation) -> comical_image::Rotation {
    match rotation {
        Rotation::North => comical_image::Rotation::None,
        Rotation::East => comical_image::Rotation::Cw90,
        Rotation::South => comical_image::Rotation::Cw180,
        Rotation::West => comical_image::Rotation::Cw270,
    }
}

/// The top-level application state for the comical viewer.
pub struct App {
    /// The currently open book, if any.
    book: Option<OpenedBook>,
    /// The navigation model for the current book.
    spread: Option<SpreadState>,
    /// Orientation of every page, refined as pages are decoded.
    orientations: Vec<PageOrientation>,
    /// Per-page rotation state.
    rotations: Vec<Rotation>,
    /// Pages whose orientation changed since the last spread rebuild.
    changed_pages: Vec<usize>,
    /// Zoom and fit settings; `mode` always mirrors the spread state.
    zoom: ZoomOptions,
    /// Scrollbar thickness in logical points. Converted to physical pixels
    /// before passing to zoom calculations.
    scrollbar_points: f32,
    /// Pan offset in canvas points.
    pan: (f32, f32),
    /// Whether the window is fullscreen.
    fullscreen: bool,
    /// Reading direction for two-page spreads.
    direction: Direction,
    /// Persistent application settings.
    settings: Settings,
    /// Last error message, drawn in the canvas.
    error: Option<String>,
    /// Window title, normally the file name.
    title: String,
    /// Set when the title must be sent to the viewport.
    title_dirty: bool,
    /// Whether the settings window is currently open.
    settings_open: bool,
    /// Canonical identity of the open book, used by `remember_page`.
    book_key: String,
    /// Book whose open attempt asked for a password, if any.
    pending_password_path: Option<String>,
    /// Whether the password prompt window is currently open.
    password_prompt_open: bool,
}

impl App {
    /// Creates an empty application, loading persisted settings from disk.
    pub fn new() -> Self {
        Self::with_settings(Settings::load())
    }

    /// Creates an empty application with explicit settings.
    ///
    /// `new` loads these from disk; tests and embedders can pass their own so
    /// a developer's saved settings never leak into a run.
    pub fn with_settings(settings: Settings) -> Self {
        let mut zoom = ZoomOptions::new(settings.mode, settings.zoom);
        zoom.custom_percent = settings.zoom_percent;
        zoom.fit_only_if_oversized = settings.fit_only_oversize;
        Self {
            book: None,
            spread: None,
            orientations: Vec::new(),
            rotations: Vec::new(),
            changed_pages: Vec::new(),
            zoom,
            scrollbar_points: 16.0,
            pan: (0.0, 0.0),
            fullscreen: false,
            direction: settings.direction,
            settings,
            error: None,
            title: "comical".to_owned(),
            title_dirty: true,
            settings_open: false,
            book_key: String::new(),
            pending_password_path: None,
            password_prompt_open: false,
        }
    }

    /// Returns a reference to the current settings.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Syncs mutable app state back into the settings struct.
    fn sync_settings(&mut self) {
        self.settings.zoom = self.zoom.zoom;
        self.settings.zoom_percent = self.zoom.custom_percent;
        self.settings.mode = self.zoom.mode;
        self.settings.fit_only_oversize = self.zoom.fit_only_if_oversized;
        self.settings.direction = self.direction;
    }

    /// Open the book at `path` and prepare the navigation model.
    ///
    /// This is the entry point from the command line and from the open dialog.
    /// On failure the previously open book is left untouched.
    ///
    /// An encrypted archive returns
    /// [`AppError::OpenBook`] with `ArchiveError::PasswordRequired`; the app
    /// then opens the password prompt for `path`.
    pub fn open_book(&mut self, path: &str) -> Result<(), AppError> {
        let options =
            comical_archive::OpenOptions::new().with_natural_sort(self.settings.natural_sort);
        let archive = match comical_archive::open_with_options(path, options) {
            Ok(archive) => archive,
            Err(error) => {
                if error.is_password_required() {
                    self.pending_password_path = Some(path.to_owned());
                }
                return Err(AppError::OpenBook(error));
            }
        };
        self.install_book(path, archive)
    }

    /// Installs a freshly opened archive as the open book and navigates.
    fn install_book(
        &mut self,
        path: &str,
        archive: Box<dyn comical_archive::ComicArchive + Send>,
    ) -> Result<(), AppError> {
        let count = archive.pages().len();
        let orientations = vec![PageOrientation::Portrait; count];
        let rotations = vec![Rotation::North; count];
        let spread = SpreadState::new(orientations.clone(), self.zoom.mode)?;
        let book = OpenedBook::new(archive, self.settings.filter);

        self.book = Some(book);
        self.orientations = orientations;
        self.rotations = rotations;
        self.changed_pages.clear();
        self.spread = Some(spread);
        self.pan = (0.0, 0.0);
        self.error = None;
        self.title = title_for(path);
        self.title_dirty = true;
        self.book_key = book_key(path);
        if self.settings.remember_page
            && let Some(&page) = self.settings.last_pages.get(&self.book_key)
        {
            self.go_to_page(page.min(count.saturating_sub(1)));
        }
        Ok(())
    }

    /// Answers a pending password prompt and retries the open.
    ///
    /// Returns `true` when the retry installed a book. A wrong password keeps
    /// the prompt open for another attempt; the error is only surfaced when the
    /// prompt is dismissed.
    pub fn submit_password(&mut self, password: &str) -> bool {
        let Some(path) = self.pending_password_path.clone() else {
            return false;
        };
        let options = comical_archive::OpenOptions::new()
            .with_password(Some(password))
            .with_natural_sort(self.settings.natural_sort);
        let archive = match comical_archive::open_with_options(&path, options) {
            Ok(archive) => archive,
            Err(error) => {
                if error.is_bad_password() {
                    self.error = Some(error.to_string());
                }
                return false;
            }
        };
        if self.install_book(&path, archive).is_ok() {
            self.pending_password_path = None;
            self.password_prompt_open = false;
            return true;
        }
        false
    }

    /// Cancels the password prompt and keeps the previous book open.
    pub fn cancel_password_prompt(&mut self) {
        self.pending_password_path = None;
        self.password_prompt_open = false;
    }

    /// Show a native file-open dialog and open the selected book.
    ///
    /// Called from the UI thread only, when the user presses the open button.
    pub fn open_file_dialog(&mut self) {
        let mut dialog =
            rfd::FileDialog::new().add_filter("Comics", &["zip", "cbz", "rar", "cbr", "7z", "cb7"]);
        if !self.settings.last_directory.is_empty() {
            dialog = dialog.set_directory(&self.settings.last_directory);
        }
        if let Some(path) = dialog.pick_file() {
            if let Some(parent) = path.parent() {
                self.settings.last_directory = parent.to_string_lossy().into_owned();
            }
            let path = path.to_string_lossy().into_owned();
            if let Err(error) = self.open_book(&path) {
                self.show_error(error.to_string());
            }
        }
    }

    /// The error currently shown in the canvas, if any.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Replace the visible error message.
    pub fn show_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }

    /// Clear the visible error message.
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Whether a book is open.
    pub fn has_book(&self) -> bool {
        self.book.is_some()
    }

    /// The current spread, in book order.
    pub fn current_spread(&self) -> Option<Spread> {
        self.spread.as_ref().map(SpreadState::spread)
    }

    /// The zero-based index of the left page in the current spread.
    pub fn current_page(&self) -> Option<usize> {
        self.spread.as_ref().map(SpreadState::current_page)
    }

    /// Number of pages in the open book.
    pub fn page_count(&self) -> usize {
        self.book.as_ref().map_or(0, |book| book.pages.len())
    }

    /// Name of a page in book order, if the index is valid.
    pub fn page_name(&self, page: usize) -> Option<&str> {
        self.book
            .as_ref()?
            .pages
            .get(page)
            .map(|entry| entry.name.as_str())
    }

    /// Returns a reference to the decoded natural-size image for a page.
    ///
    /// Used by the thumbnail browser to generate small previews.
    pub fn decoded_page(&mut self, page: usize) -> Result<&RgbImage, AppError> {
        let book = self.book.as_mut().ok_or(AppError::NoBook)?;
        book.natural(page)
    }

    /// Discards background decode work for pages that are no longer visible.
    fn invalidate_visible(&mut self) {
        if let Some(book) = self.book.as_mut() {
            book.cancel_pending();
        }
    }

    /// Jumps to a specific page index.
    pub fn go_to_page(&mut self, page: usize) {
        if let Some(spread) = self.spread.as_mut() {
            spread.go_to_page(page);
        }
        self.pan = (0.0, 0.0);
        self.invalidate_visible();
        self.remember_current_page();
    }

    /// Returns a cached thumbnail, queueing a background decode on a miss.
    pub fn thumbnail(&mut self, page: usize) -> Option<TextureHandle> {
        self.book.as_mut()?.thumbnail(page)
    }

    /// The oriented full-page size, after rotation.
    ///
    /// The first call reads only the image header. Learning the orientation may
    /// mark the spread for a rebuild on the next frame.
    pub fn page_size(&mut self, page: usize) -> Result<Size, AppError> {
        let rotation = self.rotations.get(page).copied().unwrap_or_default();
        let oriented = {
            let book = self.book.as_mut().ok_or(AppError::NoBook)?;
            book.oriented_size(page, rotation)?
        };
        self.learn_orientation(page, oriented);
        Ok(oriented)
    }

    /// Returns the rotation for a page.
    pub fn page_rotation(&self, page: usize) -> Rotation {
        self.rotations.get(page).copied().unwrap_or_default()
    }

    /// Returns a cached texture for one visible page part.
    pub fn texture(
        &mut self,
        ctx: &Context,
        page: usize,
        part: PagePart,
    ) -> Result<TextureHandle, AppError> {
        let rotation = self.rotations.get(page).copied().unwrap_or_default();
        let book = self.book.as_mut().ok_or(AppError::NoBook)?;
        book.texture(ctx, page, part, rotation)
    }

    fn learn_orientation(&mut self, page: usize, size: Size) {
        let orientation = if size.width > size.height {
            PageOrientation::Landscape
        } else {
            PageOrientation::Portrait
        };
        let changed =
            matches!(self.orientations.get(page), Some(current) if *current != orientation);
        if changed {
            self.orientations[page] = orientation;
            self.changed_pages.push(page);
        }
    }

    /// Rotates the left page of the current spread counter-clockwise.
    pub fn rotate_left_page_ccw(&mut self) {
        let Some(spread) = self.spread.as_ref() else {
            return;
        };
        let page = spread.spread().left.page;
        if let Some(rotation) = self.rotations.get_mut(page) {
            *rotation = rotation.counter_clockwise();
            self.changed_pages.push(page);
        }
        self.invalidate_visible();
    }

    /// Rotates the left page of the current spread clockwise.
    pub fn rotate_left_page_cw(&mut self) {
        let Some(spread) = self.spread.as_ref() else {
            return;
        };
        let page = spread.spread().left.page;
        if let Some(rotation) = self.rotations.get_mut(page) {
            *rotation = rotation.clockwise();
            self.changed_pages.push(page);
        }
        self.invalidate_visible();
    }

    /// Rotates the right page of the current spread counter-clockwise.
    ///
    /// Only applies when there is a separate right page (two-page portrait spread).
    pub fn rotate_right_page_ccw(&mut self) {
        let Some(spread) = self.spread.as_ref() else {
            return;
        };
        let Some(right) = spread.spread().right else {
            return;
        };
        // Only rotate right page if it's a different page from left
        if right.page == spread.spread().left.page {
            return;
        }
        if let Some(rotation) = self.rotations.get_mut(right.page) {
            *rotation = rotation.counter_clockwise();
            self.changed_pages.push(right.page);
        }
        self.invalidate_visible();
    }

    /// Rotates the right page of the current spread clockwise.
    ///
    /// Only applies when there is a separate right page (two-page portrait spread).
    pub fn rotate_right_page_cw(&mut self) {
        let Some(spread) = self.spread.as_ref() else {
            return;
        };
        let Some(right) = spread.spread().right else {
            return;
        };
        // Only rotate right page if it's a different page from left
        if right.page == spread.spread().left.page {
            return;
        }
        if let Some(rotation) = self.rotations.get_mut(right.page) {
            *rotation = rotation.clockwise();
            self.changed_pages.push(right.page);
        }
        self.invalidate_visible();
    }

    /// Rebuilds the spread if a newly decoded page changed its orientation.
    ///
    /// A page that becomes landscape while it occupies the left slot has to
    /// restart the spread as its two halves. A landscape page in the right slot
    /// only needs its part changed to the left half, which keeps the left page
    /// visible. When neither applies, the current spread is preserved.
    ///
    /// Returns true when the state changed and a repaint is needed.
    pub fn refresh_spread(&mut self) -> bool {
        if self.changed_pages.is_empty() {
            return false;
        }
        let changed = std::mem::take(&mut self.changed_pages);
        let Some(state) = &self.spread else {
            return false;
        };
        let spread = state.spread();
        let mode = state.mode();

        let left_changed = spread.left.part == PagePart::Full
            && changed.contains(&spread.left.page)
            && self
                .orientations
                .get(spread.left.page)
                .is_some_and(|orientation| orientation.is_landscape());
        if left_changed {
            if let Ok(mut state) = SpreadState::new(self.orientations.clone(), mode) {
                state.go_to_page(spread.left.page);
                self.spread = Some(state);
                self.invalidate_visible();
                return true;
            }
            return false;
        }

        let mut updated = spread;
        if let Some(right) = updated.right
            && right.part == PagePart::Full
            && changed.contains(&right.page)
            && self
                .orientations
                .get(right.page)
                .is_some_and(|orientation| orientation.is_landscape())
        {
            updated.right = Some(PageRef::new(right.page, PagePart::Left));
        }

        match SpreadState::with_spread(self.orientations.clone(), mode, updated) {
            Ok(state) => {
                self.spread = Some(state);
                self.invalidate_visible();
                true
            }
            Err(_) => false,
        }
    }

    /// The current zoom options.
    pub fn zoom(&self) -> ZoomOptions {
        self.zoom
    }

    /// Current pan offset in canvas points.
    pub fn pan(&self) -> (f32, f32) {
        self.pan
    }

    /// Replace the pan offset.
    pub fn set_pan(&mut self, pan: (f32, f32)) {
        self.pan = pan;
    }

    /// Update the zoom scrollbar thickness from the current pixels-per-point.
    ///
    /// Call once per frame before zoom calculations so the reserved space
    /// matches the display density.
    pub fn update_scrollbar(&mut self, pixels_per_point: f32) {
        let physical = (self.scrollbar_points * pixels_per_point).round().max(1.0) as u32;
        self.zoom.scrollbar = physical;
    }

    /// Scroll by a delta in canvas points.
    pub fn scroll(&mut self, delta: (f32, f32)) {
        self.pan = (self.pan.0 + delta.0, self.pan.1 + delta.1);
    }

    /// Whether the window is fullscreen.
    pub fn fullscreen(&self) -> bool {
        self.fullscreen
    }

    /// Set the desired fullscreen state.
    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
    }

    /// Toggle the desired fullscreen state.
    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
    }

    /// The current reading direction.
    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// Set the reading direction.
    pub fn set_direction(&mut self, direction: Direction) {
        self.direction = direction;
        self.sync_settings();
    }

    /// Toggle between LTR and RTL reading direction.
    pub fn toggle_direction(&mut self) {
        self.direction = self.direction.toggled();
        self.sync_settings();
    }

    /// Set whether fit modes leave a page that already fits at 1:1.
    pub fn set_fit_only_oversize(&mut self, enabled: bool) {
        self.zoom.fit_only_if_oversized = enabled;
        self.sync_settings();
    }

    /// Toggle toolbar visibility.
    pub fn toggle_toolbar(&mut self) {
        self.settings.toolbar_visible = !self.settings.toolbar_visible;
        self.sync_settings();
    }

    /// Toggle the settings window.
    pub fn toggle_settings(&mut self) {
        self.settings_open = !self.settings_open;
    }

    /// Whether the settings window is open.
    pub fn settings_open(&self) -> bool {
        self.settings_open
    }

    /// Draws the password prompt while an open attempt needs one.
    ///
    /// Returns `true` while the prompt is open, so the frame can be scheduled.
    pub fn draw_password_prompt(&mut self, ctx: &Context) -> bool {
        if self.pending_password_path.is_none() {
            return false;
        }
        let mut open = self.password_prompt_open;
        let response = egui::Window::new("Password required")
            .id(egui::Id::new("comical-password-prompt"))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                let Some(path) = self.pending_password_path.clone() else {
                    return;
                };
                ui.label(format!("This archive is encrypted: {}", path));
                let mut password = String::new();
                ui.add(
                    egui::TextEdit::singleline(&mut password)
                        .password(true)
                        .hint_text("Password")
                        .desired_width(280.0),
                );
                if ui.button("Open").clicked() {
                    let password = std::mem::take(&mut password);
                    self.submit_password(&password);
                }
                if ui.button("Cancel").clicked() {
                    self.cancel_password_prompt();
                }
            });
        let open_flag = response.is_some();
        if !self.pending_password_path.is_some() {
            // A submit or cancel cleared the prompt during this frame.
            self.password_prompt_open = false;
        } else {
            self.password_prompt_open = open_flag;
        }
        if !self.password_prompt_open {
            self.pending_password_path = None;
        }
        self.pending_password_path.is_some() && self.password_prompt_open
    }

    /// Draw the settings window if open.
    pub fn draw_settings(&mut self, ctx: &Context) {
        if !self.settings_open {
            return;
        }
        let mut open = self.settings_open;
        eframe::egui::Window::new("Settings")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Reading");
                ui.horizontal(|ui| {
                    ui.label("Direction:");
                    let dir_label = if self.direction().is_rtl() {
                        "RTL"
                    } else {
                        "LTR"
                    };
                    if ui.button(dir_label).clicked() {
                        self.toggle_direction();
                    }
                });
                ui.separator();
                ui.heading("View");
                ui.horizontal(|ui| {
                    ui.label("Layout:");
                    let mut mode = self.zoom.mode;
                    ui.radio_value(&mut mode, ViewMode::OnePage, "One page");
                    ui.radio_value(&mut mode, ViewMode::TwoPage, "Two pages");
                    if mode != self.zoom.mode {
                        self.set_view_mode(mode);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Zoom:");
                    let zoom_label = match self.zoom.zoom {
                        ZoomMode::Fit => "Fit",
                        ZoomMode::Width => "Width",
                        ZoomMode::Height => "Height",
                        ZoomMode::Full => "Full",
                        ZoomMode::Custom => "Custom",
                    };
                    if ui.button(zoom_label).clicked() {
                        self.cycle_zoom();
                    }
                });
                if self.zoom.zoom == ZoomMode::Custom {
                    ui.horizontal(|ui| {
                        ui.label("Percent:");
                        if ui.button("-").clicked() {
                            self.adjust_custom_percent(-10);
                        }
                        ui.label(format!("{}%", self.zoom.custom_percent));
                        if ui.button("+").clicked() {
                            self.adjust_custom_percent(10);
                        }
                    });
                }
                let mut fit_only = self.zoom.fit_only_if_oversized;
                if ui
                    .checkbox(&mut fit_only, "Only fit pages larger than the window")
                    .changed()
                {
                    self.set_fit_only_oversize(fit_only);
                }
                ui.separator();
                ui.heading("Pages");
                ui.horizontal(|ui| {
                    ui.label("Filter:");
                    let mut filter = self.settings.filter;
                    eframe::egui::ComboBox::from_id_salt("settings-filter")
                        .selected_text(filter_name(filter))
                        .show_ui(ui, |ui| {
                            for candidate in [
                                Filter::Box,
                                Filter::Bilinear,
                                Filter::Bicubic,
                                Filter::BSpline,
                                Filter::CatmullRom,
                                Filter::Lanczos3,
                            ] {
                                ui.selectable_value(&mut filter, candidate, filter_name(candidate));
                            }
                        });
                    if filter != self.settings.filter {
                        self.set_filter(filter);
                    }
                });
                ui.checkbox(
                    &mut self.settings.natural_sort,
                    "Natural page order (applies on next open)",
                );
                ui.checkbox(&mut self.settings.remember_page, "Remember the last page");
                ui.separator();
                ui.heading("Interface");
                ui.checkbox(&mut self.settings.toolbar_visible, "Toolbar (T)");
                ui.checkbox(&mut self.settings.browser_visible, "Page browser (B)");
            });
        if !open {
            self.settings_open = false;
        }
    }

    /// Stores the current page for `remember_page`.
    ///
    /// The per-book map is bounded so a reader who opens thousands of books
    /// does not grow the settings file without limit.
    fn remember_current_page(&mut self) {
        if !self.settings.remember_page || self.book_key.is_empty() {
            return;
        }
        let Some(page) = self.current_page() else {
            return;
        };
        self.settings.last_pages.insert(self.book_key.clone(), page);
        while self.settings.last_pages.len() > MAX_REMEMBERED_BOOKS {
            let Some(oldest) = self.settings.last_pages.keys().next().cloned() else {
                break;
            };
            self.settings.last_pages.remove(&oldest);
        }
    }

    /// Selects the resampling filter used for scaled pages.
    ///
    /// Cached textures are keyed by filter, so clearing them makes the next
    /// frame decode with the new filter instead of showing stale pixels.
    pub fn set_filter(&mut self, filter: Filter) {
        if self.settings.filter == filter {
            return;
        }
        self.settings.filter = filter;
        if let Some(book) = self.book.as_mut() {
            book.filter = filter;
            book.textures.clear();
            book.pending.clear();
            book.cancel_pending();
        }
        self.sync_settings();
    }

    /// Toggle thumbnail browser visibility.
    pub fn toggle_browser(&mut self) {
        self.settings.browser_visible = !self.settings.browser_visible;
        self.sync_settings();
    }

    /// The desired window title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Moves to the next spread.
    pub fn next_turn(&mut self) {
        if let Some(spread) = self.spread.as_mut() {
            spread.next_turn();
        }
        self.pan = (0.0, 0.0);
        self.invalidate_visible();
        self.remember_current_page();
    }

    /// Moves to the previous spread.
    pub fn prev_turn(&mut self) {
        if let Some(spread) = self.spread.as_mut() {
            spread.prev_turn();
        }
        self.pan = (0.0, 0.0);
        self.invalidate_visible();
        self.remember_current_page();
    }

    /// Moves forward by one page.
    pub fn next_page(&mut self) {
        if let Some(spread) = self.spread.as_mut() {
            spread.next_page();
        }
        self.pan = (0.0, 0.0);
        self.invalidate_visible();
        self.remember_current_page();
    }

    /// Moves backward by one page.
    pub fn prev_page(&mut self) {
        if let Some(spread) = self.spread.as_mut() {
            spread.prev_page();
        }
        self.pan = (0.0, 0.0);
        self.invalidate_visible();
        self.remember_current_page();
    }

    /// Moves to the first spread.
    pub fn first(&mut self) {
        if let Some(spread) = self.spread.as_mut() {
            spread.first();
        }
        self.pan = (0.0, 0.0);
        self.invalidate_visible();
        self.remember_current_page();
    }

    /// Moves to the last spread.
    pub fn last(&mut self) {
        if let Some(spread) = self.spread.as_mut() {
            spread.last();
        }
        self.pan = (0.0, 0.0);
        self.invalidate_visible();
        self.remember_current_page();
    }

    /// Switches between one-page and two-page layout.
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        if let Some(spread) = self.spread.as_mut() {
            spread.set_mode(mode);
        }
        self.zoom.mode = mode;
        self.pan = (0.0, 0.0);
        self.invalidate_visible();
        self.sync_settings();
    }

    /// Selects a zoom mode.
    pub fn set_zoom_mode(&mut self, mode: ZoomMode) {
        self.zoom.zoom = mode;
        self.pan = (0.0, 0.0);
        self.sync_settings();
    }

    /// Cycles Fit, Width, Height, Full, Custom, Fit.
    pub fn cycle_zoom(&mut self) {
        self.zoom.zoom = match self.zoom.zoom {
            ZoomMode::Fit => ZoomMode::Width,
            ZoomMode::Width => ZoomMode::Height,
            ZoomMode::Height => ZoomMode::Full,
            ZoomMode::Full => ZoomMode::Custom,
            ZoomMode::Custom => ZoomMode::Fit,
        };
        self.pan = (0.0, 0.0);
        self.sync_settings();
    }

    /// Changes the custom percentage and selects custom zoom.
    pub fn adjust_custom_percent(&mut self, delta: i32) {
        let percent = i64::from(self.zoom.custom_percent) + i64::from(delta);
        self.set_custom_percent(clamp_percent(percent));
    }

    /// Sets the custom percentage and selects custom zoom.
    pub fn set_custom_percent(&mut self, percent: u32) {
        self.zoom.custom_percent = percent.clamp(MIN_CUSTOM_PERCENT, MAX_CUSTOM_PERCENT);
        self.zoom.zoom = ZoomMode::Custom;
        self.pan = (0.0, 0.0);
        self.sync_settings();
    }

    /// Drain completed background decodes and upload them as GPU textures.
    fn drain_decoded(&mut self, ctx: &Context) -> bool {
        let mut uploaded = false;
        if let Some(book) = self.book.as_mut() {
            uploaded = book.drain_decoded(ctx);
            if book.password_required {
                // An encrypted entry needed a password the prompt has not asked
                // for yet; offer the prompt instead of a permanent placeholder.
                book.password_required = false;
                let path = self.book_key.clone();
                if !path.is_empty() {
                    self.pending_password_path = Some(path);
                }
            }
        }
        uploaded
    }

    fn has_pending_work(&self) -> bool {
        self.book.as_ref().is_some_and(OpenedBook::has_pending)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if self.title_dirty {
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Title(self.title.clone()));
            self.title_dirty = false;
        }

        let consumed = crate::nav::handle_input(ctx, self);

        // Draw toolbar and browser panels before the central canvas.
        if let Some(action) = crate::toolbar::draw(ctx, self) {
            crate::toolbar::apply(self, action);
        }
        if let Some(action) = crate::browser::draw(ctx, self) {
            crate::browser::apply(self, action);
        }

        // Upload any textures decoded in the background since last frame.
        let decoded = self.drain_decoded(ctx);

        crate::view::draw(ctx, self);
        if self.draw_password_prompt(ctx) {
            ctx.request_repaint();
        }
        self.draw_settings(ctx);

        // Background results arrive without an input event. Keep a frame
        // scheduled while work is outstanding, otherwise a page queued during
        // this frame would sit invisible until the user moved the mouse.
        if decoded || self.refresh_spread() || consumed || self.settings_open {
            ctx.request_repaint();
        } else if self.has_pending_work() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.remember_current_page();
        self.sync_settings();
        self.settings.save();
    }
}

/// Human-readable name for a resampling filter.
fn filter_name(filter: Filter) -> &'static str {
    match filter {
        Filter::Box => "Box",
        Filter::Bilinear => "Bilinear",
        Filter::Bicubic => "Bicubic (Mitchell)",
        Filter::BSpline => "B-spline",
        Filter::CatmullRom => "Catmull-Rom",
        Filter::Lanczos3 => "Lanczos3",
    }
}

/// Stable identity for a book path.
///
/// Canonicalizing collapses relative paths and symlinks so the same book maps
/// to one entry. Falls back to the path as given when the filesystem cannot
/// resolve it.
fn book_key(path: &str) -> String {
    std::fs::canonicalize(path)
        .map(|canonical| canonical.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_owned())
}

fn cache_capacity() -> NonZeroUsize {
    NonZeroUsize::new(CACHE_PAGES).expect("CACHE_PAGES is non-zero")
}

fn clamp_percent(percent: i64) -> u32 {
    let low = i64::from(MIN_CUSTOM_PERCENT);
    let high = i64::from(MAX_CUSTOM_PERCENT);
    percent.clamp(low, high) as u32
}

/// Visible size of one part of a page. The midpoint split gives the leftover
/// pixel column to the right half.
pub(crate) fn split_size(size: Size, part: PagePart) -> Size {
    let split = size.width / 2;
    if split == 0 {
        return size;
    }
    match part {
        PagePart::Full => size,
        PagePart::Left => Size::new(split, size.height),
        PagePart::Right => Size::new(size.width - split, size.height),
    }
}

/// Copies one horizontal half of an image.
fn crop_part(image: &RgbImage, part: PagePart) -> Result<RgbImage, AppError> {
    let width = image.width();
    let height = image.height();
    let split = width / 2;
    if part == PagePart::Full || split == 0 {
        return Ok(image.clone());
    }

    let (x0, part_width) = if part == PagePart::Left {
        (0, split)
    } else {
        (split, width - split)
    };
    let mut pixels = Vec::with_capacity(part_width as usize * height as usize * 3);
    for y in 0..height {
        let start = (y as usize * width as usize + x0 as usize) * 3;
        let end = start + part_width as usize * 3;
        if let Some(row) = image.pixels().get(start..end) {
            pixels.extend_from_slice(row);
        }
    }
    RgbImage::new(part_width, height, pixels).map_err(AppError::from)
}

fn title_for(path: &str) -> String {
    let name = Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_owned());
    format!("{name} - comical")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_pages(pages: Vec<PageOrientation>) -> App {
        let mut app = App::new();
        let count = pages.len();
        app.orientations = pages.clone();
        app.rotations = vec![Rotation::North; count];
        app.spread = Some(SpreadState::new(pages, ViewMode::TwoPage).expect("pages"));
        app
    }

    #[test]
    fn defaults_match_the_behavior_spec() {
        let app = App::new();
        assert_eq!(app.zoom().mode, ViewMode::TwoPage);
        assert_eq!(app.zoom().zoom, ZoomMode::Width);
        assert_eq!(app.zoom().custom_percent, 100);
        assert!(!app.has_book());
    }

    #[test]
    fn encrypted_book_lists_and_reports_the_missing_password() {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join("book.cbz");
        xtask::write_zip(
            &path,
            &[xtask::FixtureEntry::new("secret.png", b"png-bytes")],
            xtask::ZipMethod::Deflate,
            xtask::ZipEncryption::Aes256,
            Some("open-sesame"),
        )
        .expect("write encrypted zip");
        let book = path.to_str().expect("temp path is utf-8");

        // Encrypted entries list without a password but fail at read time, so
        // the open succeeds and the read reports the missing password.
        let mut app = App::with_settings(Settings::default());
        app.open_book(book)
            .expect("encrypted book lists without a password");
        assert!(app.has_book());

        let archive: std::sync::Arc<
            std::sync::Mutex<Box<dyn comical_archive::ComicArchive + Send>>,
        > = std::sync::Arc::new(std::sync::Mutex::new(
            comical_archive::open(book).expect("open encrypted book"),
        ));
        let request = DecodeRequest {
            key: TextureKey {
                page: 0,
                part: PagePart::Full,
                rotation: Rotation::North,
                filter: Filter::CatmullRom,
            },
            generation: 0,
        };
        let result = decode_request(&archive, &request);
        assert!(result.password_required);
    }

    #[test]
    fn password_prompt_can_be_cancelled() {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join("book.cbz");
        xtask::write_zip(
            &path,
            &[xtask::FixtureEntry::new("page.png", b"png-bytes")],
            xtask::ZipMethod::Store,
            xtask::ZipEncryption::None,
            None,
        )
        .expect("write zip");
        let book = path.to_str().expect("temp path is utf-8");

        let mut app = App::with_settings(Settings::default());
        app.pending_password_path = Some(book.to_owned());
        app.password_prompt_open = true;
        app.cancel_password_prompt();
        assert!(app.pending_password_path.is_none());
        assert!(!app.submit_password("open-sesame"));
    }

    #[test]
    fn navigation_moves_between_spreads_and_resets_pan() {
        let mut app = app_with_pages(vec![PageOrientation::Portrait; 4]);
        assert_eq!(app.current_page(), Some(0));

        app.set_pan((12.0, -4.0));
        app.next_turn();
        assert_eq!(app.current_page(), Some(2));
        assert_eq!(app.pan(), (0.0, 0.0));

        app.prev_turn();
        assert_eq!(app.current_page(), Some(0));

        app.next_page();
        assert_eq!(app.current_page(), Some(1));

        app.last();
        assert_eq!(app.current_page(), Some(2));

        app.first();
        assert_eq!(app.current_page(), Some(0));
    }

    #[test]
    fn view_mode_switch_stays_in_sync_with_zoom_mode() {
        let mut app = app_with_pages(vec![PageOrientation::Portrait; 3]);
        app.set_view_mode(ViewMode::OnePage);
        assert_eq!(app.zoom().mode, ViewMode::OnePage);
        assert!(app.current_spread().expect("spread").right.is_none());

        app.set_view_mode(ViewMode::TwoPage);
        assert_eq!(app.zoom().mode, ViewMode::TwoPage);
        assert!(app.current_spread().expect("spread").right.is_some());
    }

    #[test]
    fn custom_percent_is_clamped() {
        let mut app = App::new();
        app.adjust_custom_percent(500);
        assert_eq!(app.zoom().custom_percent, MAX_CUSTOM_PERCENT);
        assert_eq!(app.zoom().zoom, ZoomMode::Custom);

        app.adjust_custom_percent(-1000);
        assert_eq!(app.zoom().custom_percent, MIN_CUSTOM_PERCENT);
    }

    #[test]
    fn a_decoded_landscape_page_rebuilds_as_two_halves() {
        let mut app = app_with_pages(vec![PageOrientation::Portrait; 2]);
        app.learn_orientation(0, Size::new(200, 100));
        assert!(app.refresh_spread());

        let spread = app.current_spread().expect("spread");
        assert_eq!(spread.left.page, 0);
        assert_eq!(spread.left.part, PagePart::Left);
        let right = spread.right.expect("right half");
        assert_eq!(right.page, 0);
        assert_eq!(right.part, PagePart::Right);
    }

    #[test]
    fn a_landscape_right_page_keeps_the_left_page_visible() {
        let pages = vec![
            PageOrientation::Portrait,
            PageOrientation::Landscape,
            PageOrientation::Portrait,
        ];
        let spread = Spread {
            left: PageRef::new(1, PagePart::Right),
            right: Some(PageRef::new(2, PagePart::Full)),
        };
        let mut app = App::new();
        app.orientations = pages.clone();
        app.rotations = vec![Rotation::North; 3];
        app.spread =
            Some(SpreadState::with_spread(pages, ViewMode::TwoPage, spread).expect("spread"));

        app.learn_orientation(2, Size::new(200, 100));
        assert!(app.refresh_spread());

        let spread = app.current_spread().expect("spread");
        assert_eq!(spread.left.page, 1);
        assert_eq!(spread.left.part, PagePart::Right);
        let right = spread.right.expect("right");
        assert_eq!(right.page, 2);
        assert_eq!(right.part, PagePart::Left);
    }

    #[test]
    fn split_size_uses_the_image_midpoint() {
        let size = Size::new(101, 50);
        assert_eq!(split_size(size, PagePart::Left), Size::new(50, 50));
        assert_eq!(split_size(size, PagePart::Right), Size::new(51, 50));
        assert_eq!(split_size(size, PagePart::Full), size);
    }

    #[test]
    fn crop_part_keeps_both_halves() {
        let pixels = vec![
            1, 2, 3, 4, 5, 6, 7, 8, 9, //
            10, 11, 12, 13, 14, 15, 16, 17, 18,
        ];
        let image = RgbImage::new(3, 2, pixels).expect("image");

        let left = crop_part(&image, PagePart::Left).expect("left");
        assert_eq!((left.width(), left.height()), (1, 2));
        assert_eq!(left.pixel(0, 0), Some([1, 2, 3]));
        assert_eq!(left.pixel(0, 1), Some([10, 11, 12]));

        let right = crop_part(&image, PagePart::Right).expect("right");
        assert_eq!((right.width(), right.height()), (2, 2));
        assert_eq!(right.pixel(0, 0), Some([4, 5, 6]));
        assert_eq!(right.pixel(1, 1), Some([16, 17, 18]));
    }

    #[test]
    fn rotation_state_cycles_correctly() {
        let mut app = app_with_pages(vec![PageOrientation::Portrait; 2]);

        // Initial state
        assert_eq!(app.page_rotation(0), Rotation::North);

        // Rotate left page clockwise
        app.rotate_left_page_cw();
        assert_eq!(app.page_rotation(0), Rotation::East);

        // Rotate again
        app.rotate_left_page_cw();
        assert_eq!(app.page_rotation(0), Rotation::South);

        // Rotate counter-clockwise
        app.rotate_left_page_ccw();
        assert_eq!(app.page_rotation(0), Rotation::East);
    }

    #[test]
    fn oriented_size_swaps_dimensions_for_east_west() {
        // Test the Rotation::swaps_dimensions logic
        assert!(!Rotation::North.swaps_dimensions());
        assert!(Rotation::East.swaps_dimensions());
        assert!(!Rotation::South.swaps_dimensions());
        assert!(Rotation::West.swaps_dimensions());
    }

    #[test]
    fn right_page_rotation_only_works_for_separate_pages() {
        let mut app = app_with_pages(vec![PageOrientation::Portrait; 4]);

        // In a two-page portrait spread, right page should be rotatable
        let spread = app.current_spread().expect("spread");
        assert!(spread.right.is_some());
        let right_page = spread.right.unwrap().page;

        app.rotate_right_page_cw();
        assert_eq!(app.page_rotation(right_page), Rotation::East);

        // Left page should still be North
        assert_eq!(app.page_rotation(spread.left.page), Rotation::North);
    }

    #[test]
    fn update_scrollbar_sets_physical_thickness_from_ppp() {
        let mut app = App::new();
        app.update_scrollbar(1.0);
        assert_eq!(app.zoom().scrollbar, 16);

        app.update_scrollbar(2.0);
        assert_eq!(app.zoom().scrollbar, 32);

        app.update_scrollbar(1.5);
        assert_eq!(app.zoom().scrollbar, 24);
    }

    #[test]
    fn scroll_adjusts_pan_offset() {
        let mut app = App::new();
        assert_eq!(app.pan(), (0.0, 0.0));

        app.scroll((5.0, -10.0));
        assert_eq!(app.pan(), (5.0, -10.0));

        app.scroll((-2.0, 3.0));
        assert_eq!(app.pan(), (3.0, -7.0));
    }

    #[test]
    fn all_navigation_methods_reset_pan_to_origin() {
        let mut app = app_with_pages(vec![PageOrientation::Portrait; 6]);

        app.set_pan((50.0, -100.0));
        app.next_turn();
        assert_eq!(app.pan(), (0.0, 0.0));

        app.set_pan((50.0, -100.0));
        app.prev_turn();
        assert_eq!(app.pan(), (0.0, 0.0));

        app.set_pan((50.0, -100.0));
        app.next_page();
        assert_eq!(app.pan(), (0.0, 0.0));

        app.set_pan((50.0, -100.0));
        app.prev_page();
        assert_eq!(app.pan(), (0.0, 0.0));

        app.set_pan((50.0, -100.0));
        app.first();
        assert_eq!(app.pan(), (0.0, 0.0));

        app.set_pan((50.0, -100.0));
        app.last();
        assert_eq!(app.pan(), (0.0, 0.0));
    }
}
