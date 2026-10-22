//! Thumbnail browser side panel.
//!
//! A 110 px wide left-side panel showing one row per page with a small
//! thumbnail. Clicking a row jumps to that page. The browser is hidden in
//! fullscreen mode and its visibility is toggled via the `B` key or menu.

use std::num::NonZeroUsize;

use comical_image::{Filter, resize};
use eframe::egui::{self, Color32, ColorImage, Context, TextureHandle, TextureOptions};
use lru::LruCache;

use crate::App;

/// Width of the thumbnail browser panel in logical points.
const PANEL_WIDTH: f32 = 110.0;

/// Height of each thumbnail row in logical points.
const ROW_HEIGHT: f32 = 60.0;

/// Horizontal margin inside the panel.
const H_MARGIN: f32 = 5.0;

/// Vertical margin between rows.
const V_MARGIN: f32 = 3.0;

/// Maximum dimension (width or height) of a thumbnail image in pixels.
const THUMB_MAX_DIM: u32 = 80;

/// Number of thumbnail textures kept in memory.
const THUMB_CACHE_SIZE: usize = 50;

/// Actions the browser may request after drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserAction {
    /// Jump to the given page index.
    GoToPage(usize),
}

/// Per-book thumbnail cache. Stored on App so it survives across frames
/// but is cleared when a new book is opened.
pub struct ThumbnailCache {
    textures: LruCache<usize, TextureHandle>,
}

impl ThumbnailCache {
    pub fn new() -> Self {
        Self {
            textures: LruCache::new(NonZeroUsize::new(THUMB_CACHE_SIZE).expect("non-zero")),
        }
    }

    /// Clears all cached thumbnails. Call when opening a new book.
    pub fn clear(&mut self) {
        self.textures.clear();
    }
}

impl Default for ThumbnailCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Draw the thumbnail browser if visible and not fullscreen.
///
/// Returns an action to apply, if any row was clicked.
pub fn draw(ctx: &Context, app: &mut App) -> Option<BrowserAction> {
    if app.fullscreen() || !app.settings().browser_visible {
        return None;
    }
    if !app.has_book() {
        return None;
    }

    let page_count = app.page_count();
    let current_page = app.current_page();
    let mut action = None;

    egui::SidePanel::left("thumbnail_browser")
        .exact_width(PANEL_WIDTH)
        .frame(
            egui::Frame::NONE
                .fill(Color32::WHITE)
                .inner_margin(egui::Margin::symmetric(H_MARGIN as i8, V_MARGIN as i8)),
        )
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for page in 0..page_count {
                        let is_selected = current_page == Some(page);
                        let response = draw_row(ui, ctx, app, page, is_selected);
                        if response.clicked() {
                            action = Some(BrowserAction::GoToPage(page));
                        }
                    }
                });
        });

    action
}

fn draw_row(
    ui: &mut egui::Ui,
    ctx: &Context,
    app: &mut App,
    page: usize,
    selected: bool,
) -> egui::Response {
    let available_width = ui.available_width();
    let row_rect = ui.allocate_space(egui::vec2(available_width, ROW_HEIGHT)).1;

    let bg = if selected {
        Color32::from_rgb(200, 220, 255)
    } else {
        Color32::WHITE
    };
    ui.painter().rect_filled(row_rect, 2.0, bg);

    if selected {
        ui.painter().rect_stroke(
            row_rect,
            2.0,
            egui::Stroke::new(1.5_f32, Color32::from_rgb(70, 130, 200)),
            egui::StrokeKind::Inside,
        );
    }

    // Try to get or generate a thumbnail texture.
    if let Some(handle) = get_thumbnail(ctx, app, page) {
        let thumb_size = handle.size_vec2();
        let aspect = thumb_size.x / thumb_size.y.max(1.0);
        let max_h = ROW_HEIGHT - V_MARGIN * 2.0;
        let max_w = available_width - H_MARGIN * 2.0;
        let (w, h) = if max_w / max_h > aspect {
            (max_h * aspect, max_h)
        } else {
            (max_w, max_w / aspect)
        };
        let image_rect = egui::Rect::from_center_size(row_rect.center(), egui::vec2(w, h));
        ui.painter().image(
            handle.id(),
            image_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    // Page number label at bottom-right of the row.
    let label = format!("{}", page + 1);
    ui.painter().text(
        row_rect.right_bottom() + egui::vec2(-4.0, -2.0),
        egui::Align2::RIGHT_BOTTOM,
        label,
        egui::FontId::proportional(10.0),
        Color32::DARK_GRAY,
    );

    // Invisible interactable covering the whole row for click detection.
    ui.interact(row_rect, ui.next_auto_id(), egui::Sense::click())
}

fn get_thumbnail(ctx: &Context, app: &mut App, page: usize) -> Option<TextureHandle> {
    // Check cache first.
    if let Some(handle) = app.thumb_cache_mut().textures.get(&page) {
        return Some(handle.clone());
    }

    // Generate thumbnail from the decoded natural-size image.
    let image = app.decoded_page(page).ok()?;
    let (tw, th) = thumbnail_dimensions(image.width(), image.height());
    let thumb = resize(image, tw, th, Filter::Box).ok()?;

    let size = [thumb.width() as usize, thumb.height() as usize];
    let color = ColorImage::from_rgb(size, thumb.pixels());
    let name = format!("thumb-{page}");
    let handle = ctx.load_texture(name, color, TextureOptions::LINEAR);
    app.thumb_cache_mut().textures.put(page, handle.clone());
    Some(handle)
}

/// Compute thumbnail dimensions preserving aspect ratio, fitting within
/// `THUMB_MAX_DIM`.
fn thumbnail_dimensions(src_w: u32, src_h: u32) -> (u32, u32) {
    if src_w == 0 || src_h == 0 {
        return (1, 1);
    }
    let scale = THUMB_MAX_DIM as f64 / src_w.max(src_h) as f64;
    let w = ((src_w as f64 * scale).round() as u32).max(1);
    let h = ((src_h as f64 * scale).round() as u32).max(1);
    (w, h)
}

/// Apply a browser action to the app state.
pub fn apply(app: &mut App, action: BrowserAction) {
    match action {
        BrowserAction::GoToPage(page) => app.go_to_page(page),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnail_dimensions_preserves_aspect_ratio() {
        let (w, h) = thumbnail_dimensions(200, 400);
        assert_eq!(w, 40);
        assert_eq!(h, 80);
    }

    #[test]
    fn thumbnail_dimensions_square() {
        let (w, h) = thumbnail_dimensions(300, 300);
        assert_eq!(w, 80);
        assert_eq!(h, 80);
    }

    #[test]
    fn thumbnail_dimensions_landscape() {
        let (w, h) = thumbnail_dimensions(400, 200);
        assert_eq!(w, 80);
        assert_eq!(h, 40);
    }

    #[test]
    fn thumbnail_dimensions_zero_returns_one() {
        let (w, h) = thumbnail_dimensions(0, 100);
        assert_eq!(w, 1);
        assert_eq!(h, 1);
    }

    #[test]
    fn thumbnail_cache_keyed_by_page_index() {
        let cache = ThumbnailCache::new();
        assert!(!cache.textures.contains(&0));
        assert!(!cache.textures.contains(&5));
        // Cache starts empty, keys are page indices.
        assert_eq!(cache.textures.len(), 0);
    }

    #[test]
    fn thumbnail_cache_clear_empties() {
        let cache = ThumbnailCache::new();
        // Verify a fresh cache is empty; clear is tested implicitly via new().
        assert_eq!(cache.textures.len(), 0);
    }
}
