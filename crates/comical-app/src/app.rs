//! Application state and book loading for the comical viewer.
//!
//! [`App`] holds everything the UI needs: the currently open book, the
//! [`SpreadState`] navigation model, decoded-page and texture caches, and view
//! settings (zoom, pan, fullscreen).

use std::num::NonZeroUsize;
use std::path::Path;

use comical_archive::{ComicArchive, DEFAULT_MAX_ENTRY_BYTES};
use comical_core::{
    MAX_CUSTOM_PERCENT, MIN_CUSTOM_PERCENT, PageEntry, PageOrientation, PagePart, PageRef, Size,
    Spread, SpreadState, ViewMode, ZoomMode, ZoomOptions,
};
use comical_image::{Filter, RgbImage, decode, resize};
use eframe::egui::{ColorImage, Context, TextureHandle, TextureOptions};
use lru::LruCache;

use crate::error::AppError;

/// Number of decoded pages and uploaded textures kept in memory.
const CACHE_PAGES: usize = 10;

/// Resampling filter used when a page has to be scaled.
const RESAMPLE_FILTER: Filter = Filter::CatmullRom;

/// Identity of one uploaded texture.
///
/// Width and height are the physical target dimensions, so a zoom change
/// produces a new key and a fresh upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TextureKey {
    page: usize,
    part: PagePart,
    width: u32,
    height: u32,
}

/// A currently open book and its per-book caches.
struct OpenedBook {
    /// Page entries in book order.
    pages: Vec<PageEntry>,
    /// The archive handle (keeps it alive).
    archive: Box<dyn ComicArchive>,
    /// Natural full-page size, filled on the first successful decode.
    page_sizes: Vec<Option<Size>>,
    /// Per-page error message, so a broken page is not retried every frame.
    page_errors: Vec<Option<String>>,
    /// Decoded pages at natural size, keyed by page index.
    natural: LruCache<usize, RgbImage>,
    /// Uploaded, already resized textures, keyed by page, part and target size.
    textures: LruCache<TextureKey, TextureHandle>,
}

impl OpenedBook {
    fn new(archive: Box<dyn ComicArchive>) -> Self {
        let pages = archive.pages().to_vec();
        let count = pages.len();
        Self {
            pages,
            archive,
            page_sizes: vec![None; count],
            page_errors: vec![None; count],
            natural: LruCache::new(cache_capacity()),
            textures: LruCache::new(cache_capacity()),
        }
    }

    /// Returns the natural full-page size, decoding the page on first use.
    fn natural_size(&mut self, page: usize) -> Result<Size, AppError> {
        if let Some(Some(size)) = self.page_sizes.get(page) {
            return Ok(*size);
        }

        let size = {
            let image = self.natural(page)?;
            Size::new(image.width(), image.height())
        };
        if let Some(slot) = self.page_sizes.get_mut(page) {
            *slot = Some(size);
        }
        Ok(size)
    }

    /// Returns the decoded page at natural size, reading it on first use.
    fn natural(&mut self, page: usize) -> Result<&RgbImage, AppError> {
        if self.natural.contains(&page) {
            return Ok(self
                .natural
                .get(&page)
                .expect("page was found in the natural cache"));
        }
        if let Some(Some(message)) = self.page_errors.get(page) {
            return Err(AppError::Page {
                page,
                message: message.clone(),
            });
        }

        let bytes = match self.archive.read_page(page, DEFAULT_MAX_ENTRY_BYTES) {
            Ok(bytes) => bytes,
            Err(error) => {
                let message = error.to_string();
                self.record_page_error(page, message.clone());
                return Err(AppError::Page { page, message });
            }
        };

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
    /// A cache hit performs no archive read, no decode and no resample, which
    /// keeps panning cheap.
    fn texture(
        &mut self,
        ctx: &Context,
        page: usize,
        part: PagePart,
        target: Size,
    ) -> Result<TextureHandle, AppError> {
        let key = TextureKey {
            page,
            part,
            width: target.width,
            height: target.height,
        };
        if let Some(handle) = self.textures.get(&key) {
            return Ok(handle.clone());
        }

        let image = {
            let natural = self.natural(page)?;
            if part == PagePart::Full {
                if natural.width() == target.width && natural.height() == target.height {
                    natural.clone()
                } else {
                    resize(natural, target.width, target.height, RESAMPLE_FILTER)?
                }
            } else {
                let cropped = crop_part(natural, part)?;
                if cropped.width() == target.width && cropped.height() == target.height {
                    cropped
                } else {
                    resize(&cropped, target.width, target.height, RESAMPLE_FILTER)?
                }
            }
        };

        let size = [image.width() as usize, image.height() as usize];
        let color = ColorImage::from_rgb(size, image.pixels());
        let name = format!("page-{page}-{part:?}-{}x{}", target.width, target.height);
        let handle = ctx.load_texture(name, color, TextureOptions::LINEAR);
        self.textures.put(key, handle.clone());
        Ok(handle)
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
    /// Pages whose orientation changed since the last spread rebuild.
    changed_pages: Vec<usize>,
    /// Zoom and fit settings; `mode` always mirrors the spread state.
    zoom: ZoomOptions,
    /// Pan offset in canvas points.
    pan: (f32, f32),
    /// Whether the window is fullscreen.
    fullscreen: bool,
    /// Last error message, drawn in the canvas.
    error: Option<String>,
    /// Window title, normally the file name.
    title: String,
    /// Set when the title must be sent to the viewport.
    title_dirty: bool,
}

impl App {
    /// Creates an empty application with the behavior-spec defaults.
    pub fn new() -> Self {
        Self {
            book: None,
            spread: None,
            orientations: Vec::new(),
            changed_pages: Vec::new(),
            zoom: ZoomOptions::new(ViewMode::TwoPage, ZoomMode::Width),
            pan: (0.0, 0.0),
            fullscreen: false,
            error: None,
            title: "comical".to_owned(),
            title_dirty: true,
        }
    }

    /// Open the book at `path` and prepare the navigation model.
    ///
    /// This is the entry point from the command line and from the open dialog.
    /// On failure the previously open book is left untouched.
    pub fn open_book(&mut self, path: &str) -> Result<(), AppError> {
        let archive = comical_archive::open(path)?;
        let count = archive.pages().len();
        let orientations = vec![PageOrientation::Portrait; count];
        let spread = SpreadState::new(orientations.clone(), self.zoom.mode)?;
        let book = OpenedBook::new(archive);

        self.book = Some(book);
        self.orientations = orientations;
        self.changed_pages.clear();
        self.spread = Some(spread);
        self.pan = (0.0, 0.0);
        self.error = None;
        self.title = title_for(path);
        self.title_dirty = true;
        Ok(())
    }

    /// Show a native file-open dialog and open the selected book.
    ///
    /// Called from the UI thread only, when the user presses the open button.
    pub fn open_file_dialog(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter("Comics", &["zip", "cbz", "rar", "cbr", "7z", "cb7"])
            .pick_file();
        if let Some(path) = picked {
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

    /// The natural full-page size, after rotation.
    ///
    /// The first call for a page decodes it. Learning its orientation may mark
    /// the spread for a rebuild on the next frame.
    pub fn page_size(&mut self, page: usize) -> Result<Size, AppError> {
        let natural = {
            let book = self.book.as_mut().ok_or(AppError::NoBook)?;
            book.natural_size(page)?
        };
        self.learn_orientation(page, natural);
        Ok(natural)
    }

    /// Returns a cached, uploaded texture for the requested target size.
    pub fn texture(
        &mut self,
        ctx: &Context,
        page: usize,
        part: PagePart,
        target: Size,
    ) -> Result<TextureHandle, AppError> {
        let book = self.book.as_mut().ok_or(AppError::NoBook)?;
        book.texture(ctx, page, part, target)
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
    }

    /// Moves to the previous spread.
    pub fn prev_turn(&mut self) {
        if let Some(spread) = self.spread.as_mut() {
            spread.prev_turn();
        }
        self.pan = (0.0, 0.0);
    }

    /// Moves forward by one page.
    pub fn next_page(&mut self) {
        if let Some(spread) = self.spread.as_mut() {
            spread.next_page();
        }
        self.pan = (0.0, 0.0);
    }

    /// Moves backward by one page.
    pub fn prev_page(&mut self) {
        if let Some(spread) = self.spread.as_mut() {
            spread.prev_page();
        }
        self.pan = (0.0, 0.0);
    }

    /// Moves to the first spread.
    pub fn first(&mut self) {
        if let Some(spread) = self.spread.as_mut() {
            spread.first();
        }
        self.pan = (0.0, 0.0);
    }

    /// Moves to the last spread.
    pub fn last(&mut self) {
        if let Some(spread) = self.spread.as_mut() {
            spread.last();
        }
        self.pan = (0.0, 0.0);
    }

    /// Switches between one-page and two-page layout.
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        if let Some(spread) = self.spread.as_mut() {
            spread.set_mode(mode);
        }
        self.zoom.mode = mode;
        self.pan = (0.0, 0.0);
    }

    /// Selects a zoom mode.
    pub fn set_zoom_mode(&mut self, mode: ZoomMode) {
        self.zoom.zoom = mode;
        self.pan = (0.0, 0.0);
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
        crate::view::draw(ctx, self);

        if self.refresh_spread() || consumed {
            ctx.request_repaint();
        }
    }
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
        app.orientations = pages.clone();
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
}
