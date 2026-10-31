//! Canvas drawing: black background, centered page image(s), pan support,
//! error display, and the "Open a book" button when no book is loaded.

use comical_core::{
    Direction, PagePart, PageRef, Size, Spread, ViewMode, compute_zoom_with_neighbors,
};
use eframe::egui::{
    Align2, Color32, Context, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, Vec2,
};

use crate::App;
use crate::app::split_size;

/// Draw the whole window contents.
pub fn draw(ctx: &Context, app: &mut App) {
    eframe::egui::CentralPanel::default()
        .frame(eframe::egui::Frame::NONE.fill(Color32::BLACK))
        .show(ctx, |ui| {
            if app.has_book() {
                draw_book(ctx, ui, app);
            } else {
                draw_welcome(ui, app);
            }
        });
}

fn draw_welcome(ui: &mut eframe::egui::Ui, app: &mut App) {
    ui.vertical_centered(|ui| {
        ui.add_space(48.0);
        ui.label(
            eframe::egui::RichText::new("comical")
                .size(32.0)
                .strong()
                .color(Color32::WHITE),
        );
        ui.add_space(12.0);
        if ui.button("Open a book").clicked() {
            app.open_file_dialog();
        }
        if let Some(error) = app.error() {
            ui.add_space(16.0);
            ui.label(
                eframe::egui::RichText::new(format!("Error: {error}")).color(Color32::LIGHT_RED),
            );
        }
        ui.add_space(12.0);
        ui.label(
            eframe::egui::RichText::new(
                "Arrows turn pages (Shift+arrows scroll), PgUp/PgDn turn spreads, D toggles LTR/RTL, 1/2 change view, F11 fullscreen",
            )
            .small()
            .color(Color32::GRAY),
        );
    });
}

fn draw_book(ctx: &Context, ui: &mut eframe::egui::Ui, app: &mut App) {
    let Some(spread) = app.current_spread() else {
        return;
    };

    // Webtoon mode uses a completely different rendering path: continuous
    // vertical scroll through all pages instead of discrete spreads.
    if app.zoom().mode == ViewMode::Webtoon {
        draw_webtoon(ctx, ui, app);
        return;
    }

    let mut spread = spread;
    let canvas_rect = ui.max_rect();
    let ppp = ctx.pixels_per_point().max(f32::EPSILON);
    app.update_scrollbar(ppp);
    let canvas = Size::new(
        (canvas_rect.width() * ppp).round().max(1.0) as u32,
        (canvas_rect.height() * ppp).round().max(1.0) as u32,
    );
    let options = app.zoom();

    // First pass: learn the natural size of every visible page before drawing.
    // A page first seen as landscape changes the spread, so re-resolve the refs
    // and fill in sizes for any page the new spread added. This keeps a split
    // landscape page from flashing as a full portrait page for one frame.
    let direction = app.direction();
    let mut refs = visible_refs(&spread, direction);
    let mut natural = page_sizes(app, &refs);
    if app.refresh_spread()
        && let Some(updated) = app.current_spread()
    {
        spread = updated;
        let updated_refs = visible_refs(&spread, direction);
        natural = updated_refs
            .iter()
            .map(|page_ref| {
                size_for_page(&refs, &natural, page_ref.page)
                    .unwrap_or_else(|| app.page_size(page_ref.page).ok())
            })
            .collect();
        refs = updated_refs;
    }

    // Scale each page as a whole, then divide a split landscape page at the
    // midpoint of the scaled dimensions.
    let mut targets: Vec<Size> = Vec::with_capacity(refs.len());
    let mut handles: Vec<Option<TextureHandle>> = Vec::with_capacity(refs.len());
    let mut errors: Vec<Option<String>> = Vec::with_capacity(refs.len());
    for (index, page_ref) in refs.iter().enumerate() {
        let target = match natural[index] {
            Some(size) => {
                let neighbors: Vec<Size> = natural
                    .iter()
                    .enumerate()
                    .filter(|(other, _)| *other != index)
                    .filter_map(|(_, size)| *size)
                    .collect();
                let zoom = compute_zoom_with_neighbors(canvas, size, options, &neighbors);
                split_size(Size::new(zoom.width, zoom.height), page_ref.part)
            }
            None => Size::new((canvas.width / 2).max(1), (canvas.height / 2).max(1)),
        };
        targets.push(target);

        let fetch_part = effective_part(page_ref.part, direction);
        match app.texture(ctx, page_ref.page, fetch_part) {
            Ok(handle) => {
                handles.push(Some(handle));
                errors.push(None);
            }
            Err(error) => {
                handles.push(None);
                errors.push(Some(error.to_string()));
            }
        }
    }

    // Drag anywhere on the canvas to pan.
    let response = ui.allocate_rect(canvas_rect, Sense::drag());
    if response.dragged() {
        let delta = response.drag_delta();
        let (x, y) = app.pan();
        app.set_pan((x + delta.x, y + delta.y));
    }

    let (content_left, content_right, content_height) =
        content_bounds(&targets, ppp, canvas_rect.width());
    let content_top = if content_height > canvas_rect.height() {
        0.0
    } else {
        (canvas_rect.height() - content_height) / 2.0
    };
    let content = Rect::from_min_max(
        Pos2::new(content_left, content_top),
        Pos2::new(content_right, content_top + content_height),
    );
    let pan = clamp_pan(app.pan(), canvas_rect.size(), content);
    app.set_pan(pan);

    let painter = ui.painter().with_clip_rect(canvas_rect);
    let mut x = content_left + pan.0;
    for (index, target) in targets.iter().enumerate() {
        let width = target.width as f32 / ppp;
        let height = target.height as f32 / ppp;
        let page_rect = Rect::from_min_size(
            canvas_rect.min + Vec2::new(x, content_top + pan.1),
            Vec2::new(width, height),
        );

        if let Some(handle) = &handles[index] {
            painter.image(handle.id(), page_rect, uv_full(), Color32::WHITE);
        } else {
            // Keep placeholder text inside this page even if layout overflows.
            let page_painter = painter.with_clip_rect(page_rect.intersect(canvas_rect));
            page_painter.rect_filled(page_rect, 0.0, Color32::from_gray(18));
            page_painter.rect_stroke(
                page_rect,
                0.0,
                Stroke::new(1.0_f32, Color32::DARK_GRAY),
                StrokeKind::Inside,
            );
            if let Some(message) = &errors[index] {
                paint_placeholder(&page_painter, page_rect, message);
            }
        }

        x += width;
    }

    // Draw scrollbar indicators when content overflows the viewport.
    let sb_thickness = options.scrollbar as f32 / ppp;
    let overflow_x = content.width() > canvas_rect.width();
    let overflow_y = content.height() > canvas_rect.height();

    if overflow_y {
        // Vertical scrollbar on the right edge.
        let track_x = canvas_rect.right() - sb_thickness;
        let track = Rect::from_min_max(
            Pos2::new(track_x, canvas_rect.top()),
            Pos2::new(canvas_rect.right(), canvas_rect.bottom()),
        );
        painter.rect_filled(track, 0.0, Color32::from_gray(32));

        // Thumb position based on pan offset relative to scrollable range.
        let scrollable = content.height() - canvas_rect.height();
        let view_ratio = canvas_rect.height() / content.height();
        let thumb_height = (canvas_rect.height() * view_ratio).max(20.0);
        let scroll_frac = if scrollable > 0.0 {
            (-pan.1 - content.top()) / scrollable
        } else {
            0.0
        };
        let thumb_y = canvas_rect.top() + scroll_frac * (canvas_rect.height() - thumb_height);
        let thumb = Rect::from_min_max(
            Pos2::new(track_x, thumb_y),
            Pos2::new(canvas_rect.right(), thumb_y + thumb_height),
        );
        painter.rect_filled(thumb, 0.0, Color32::from_gray(96));
    }

    if overflow_x {
        // Horizontal scrollbar on the bottom edge.
        let track_y = canvas_rect.bottom() - sb_thickness;
        let track = Rect::from_min_max(
            Pos2::new(canvas_rect.left(), track_y),
            Pos2::new(canvas_rect.right(), canvas_rect.bottom()),
        );
        painter.rect_filled(track, 0.0, Color32::from_gray(32));

        // Thumb position based on pan offset relative to scrollable range.
        let scrollable = content.width() - canvas_rect.width();
        let view_ratio = canvas_rect.width() / content.width();
        let thumb_width = (canvas_rect.width() * view_ratio).max(20.0);
        let scroll_frac = if scrollable > 0.0 {
            (-pan.0 - content.left()) / scrollable
        } else {
            0.0
        };
        let thumb_x = canvas_rect.left() + scroll_frac * (canvas_rect.width() - thumb_width);
        let thumb = Rect::from_min_max(
            Pos2::new(thumb_x, track_y),
            Pos2::new(thumb_x + thumb_width, canvas_rect.bottom()),
        );
        painter.rect_filled(thumb, 0.0, Color32::from_gray(96));
    }

    // Draw page labels: separate labels for two distinct pages, one centered
    // label for single-page or split-landscape mode. Labels are positioned
    // inside the content area to avoid overlapping scrollbar indicators.
    let label_y = if overflow_y {
        // Keep labels above the horizontal scrollbar track.
        canvas_rect.bottom() - sb_thickness - 18.0
    } else {
        canvas_rect.bottom() - 18.0
    };
    let total = app.page_count();
    let is_two_distinct_pages = spread
        .right
        .is_some_and(|right| right.page != spread.left.page);

    if is_two_distinct_pages {
        // Two distinct pages: draw separate labels near the bottom of each page area.
        // In screen order, refs[0] is the left page and refs[1] is the right page.
        let left_page_num = refs[0].page + 1;
        let right_page_num = refs[1].page + 1;

        // Left label: bottom-left of the left page area, inset from edges.
        let left_label_x = if overflow_x {
            // Stay inside the content area, away from the vertical scrollbar.
            content_left + pan.0 + 12.0
        } else {
            content_left + pan.0 + 12.0
        };
        painter.text(
            Pos2::new(left_label_x, label_y),
            Align2::LEFT_BOTTOM,
            format!("{left_page_num} of {total}"),
            FontId::proportional(13.0),
            Color32::from_gray(160),
        );

        // Right label: bottom-right of the right page area, inset from edges.
        let right_label_x = if overflow_x {
            // Stay inside the content area, away from the vertical scrollbar.
            content_right + pan.0 - 12.0
        } else {
            content_right + pan.0 - 12.0
        };
        painter.text(
            Pos2::new(right_label_x, label_y),
            Align2::RIGHT_BOTTOM,
            format!("{right_page_num} of {total}"),
            FontId::proportional(13.0),
            Color32::from_gray(160),
        );
    } else {
        // Single page or split landscape: one centered label at the bottom.
        let page_num = spread.left.page + 1;
        painter.text(
            Pos2::new(canvas_rect.center().x, label_y),
            Align2::CENTER_BOTTOM,
            format!("{page_num} of {total}"),
            FontId::proportional(13.0),
            Color32::from_gray(160),
        );
    }

    if let Some(error) = app.error() {
        painter.text(
            canvas_rect.left_top() + Vec2::new(12.0, 12.0),
            Align2::LEFT_TOP,
            format!("Error: {error}"),
            FontId::proportional(14.0),
            Color32::LIGHT_RED,
        );
    }
}

/// Gap between pages in webtoon mode, in logical points.
const WEBTOON_GAP_PTS: f32 = 4.0;

/// Layout entry for one page in the webtoon strip.
struct WebtoonPageLayout {
    /// Book-order page index.
    page: usize,
    /// Y offset of this page's top edge within the strip content, in points.
    y_offset: f32,
    /// Scaled width in points.
    width: f32,
    /// Scaled height in points.
    height: f32,
}

/// Continuous vertical scroll renderer for webtoon mode.
///
/// All pages are laid out in a single vertical strip, each scaled to fit the
/// viewport width. Only pages whose rects intersect the visible viewport are
/// decoded and uploaded, keeping memory bounded regardless of book length.
fn draw_webtoon(ctx: &Context, ui: &mut eframe::egui::Ui, app: &mut App) {
    let canvas_rect = ui.max_rect();
    let ppp = ctx.pixels_per_point().max(f32::EPSILON);
    app.update_scrollbar(ppp);
    let options = app.zoom();
    let total = app.page_count();
    if total == 0 {
        return;
    }

    let gap = WEBTOON_GAP_PTS;
    let viewport_width_pts = canvas_rect.width();

    // Build the strip layout: compute y_offset, width and height for every page.
    // Pages that haven't been probed yet get a placeholder aspect ratio of 2:3
    // (portrait comic standard) so the strip has a stable shape before decode.
    let mut layouts: Vec<WebtoonPageLayout> = Vec::with_capacity(total);
    let mut cursor_y: f32 = 0.0;
    for page in 0..total {
        let natural = app.page_size(page).ok();
        let (w_pts, h_pts) = match natural {
            Some(size) => {
                let scale = viewport_width_pts / (size.width as f32 / ppp);
                let w = viewport_width_pts;
                let h = (size.height as f32 / ppp) * scale;
                (w, h)
            }
            None => {
                // Placeholder: assume 2:3 portrait until decoded.
                let w = viewport_width_pts;
                let h = w * 1.5;
                (w, h)
            }
        };
        layouts.push(WebtoonPageLayout {
            page,
            y_offset: cursor_y,
            width: w_pts,
            height: h_pts,
        });
        cursor_y += h_pts + gap;
    }
    let strip_height = if layouts.is_empty() {
        0.0
    } else {
        cursor_y - gap
    };

    // Drag to pan vertically. Horizontal pan is clamped to zero since the
    // strip always fits the viewport width.
    let response = ui.allocate_rect(canvas_rect, Sense::drag());
    if response.dragged() {
        let delta = response.drag_delta();
        let (_, y) = app.pan();
        app.set_pan((0.0, y + delta.y));
    }

    // Clamp vertical pan so the strip never scrolls past its bounds.
    let max_scroll = (strip_height - canvas_rect.height()).max(0.0);
    let (_, raw_y) = app.pan();
    let clamped_y = raw_y.clamp(-max_scroll, 0.0);
    if clamped_y != raw_y {
        app.set_pan((0.0, clamped_y));
    }
    let pan_y = clamped_y;

    // Determine which pages are visible (or within a margin for prefetch).
    let margin = canvas_rect.height() * 0.5;
    let _vis_top = -pan_y - margin;
    let _vis_bottom = -pan_y + canvas_rect.height() + margin;

    let painter = ui.painter().with_clip_rect(canvas_rect);
    let center_x = canvas_rect.left() + viewport_width_pts / 2.0;

    // Track the page closest to the top of the viewport for thumbnail sync.
    let mut closest_page: Option<usize> = None;
    let mut closest_dist = f32::MAX;

    for layout in &layouts {
        let page_top = layout.y_offset + pan_y;
        let page_bottom = page_top + layout.height;

        // Skip pages entirely outside the visible region plus margin.
        if page_bottom < canvas_rect.top() - margin || page_top > canvas_rect.bottom() + margin {
            continue;
        }

        let page_rect = Rect::from_min_size(
            Pos2::new(center_x - layout.width / 2.0, canvas_rect.top() + page_top),
            Vec2::new(layout.width, layout.height),
        );

        // Request texture upload for visible pages. The LRU cache and background
        // decoder handle eviction and async decode transparently.
        match app.texture(ctx, layout.page, PagePart::Full) {
            Ok(handle) => {
                painter.image(handle.id(), page_rect, uv_full(), Color32::WHITE);
            }
            Err(_) => {
                let clipped = page_rect.intersect(canvas_rect);
                if clipped.width() > 0.0 && clipped.height() > 0.0 {
                    let page_painter = painter.with_clip_rect(clipped);
                    page_painter.rect_filled(page_rect, 0.0, Color32::from_gray(18));
                    page_painter.rect_stroke(
                        page_rect,
                        0.0,
                        Stroke::new(1.0_f32, Color32::DARK_GRAY),
                        StrokeKind::Inside,
                    );
                }
            }
        }

        // Track which page is nearest the viewport top for current-page display.
        let dist = (layout.y_offset + pan_y).abs();
        if dist < closest_dist {
            closest_dist = dist;
            closest_page = Some(layout.page);
        }
    }

    // Sync the spread state to the page nearest the viewport top so the
    // thumbnail browser highlights the correct row and page labels update.
    if let Some(page) = closest_page {
        let current = app.current_spread().map(|s| s.left.page).unwrap_or(0);
        if page != current {
            app.sync_webtoon_page(page);
        }
    }

    // Vertical scrollbar indicator.
    let sb_thickness = options.scrollbar as f32 / ppp;
    if strip_height > canvas_rect.height() {
        let track_x = canvas_rect.right() - sb_thickness;
        let track = Rect::from_min_max(
            Pos2::new(track_x, canvas_rect.top()),
            Pos2::new(canvas_rect.right(), canvas_rect.bottom()),
        );
        painter.rect_filled(track, 0.0, Color32::from_gray(32));

        let scrollable = strip_height - canvas_rect.height();
        let view_ratio = canvas_rect.height() / strip_height;
        let thumb_height = (canvas_rect.height() * view_ratio).max(20.0);
        let scroll_frac = if scrollable > 0.0 {
            (-pan_y) / scrollable
        } else {
            0.0
        };
        let thumb_y = canvas_rect.top() + scroll_frac * (canvas_rect.height() - thumb_height);
        let thumb = Rect::from_min_max(
            Pos2::new(track_x, thumb_y),
            Pos2::new(canvas_rect.right(), thumb_y + thumb_height),
        );
        painter.rect_filled(thumb, 0.0, Color32::from_gray(96));
    }

    // Page label at the bottom center.
    let label_page = closest_page.unwrap_or(0) + 1;
    let label_y = canvas_rect.bottom() - 18.0;
    painter.text(
        Pos2::new(canvas_rect.center().x, label_y),
        Align2::CENTER_BOTTOM,
        format!("{label_page} of {total}"),
        FontId::proportional(13.0),
        Color32::from_gray(160),
    );

    if let Some(error) = app.error() {
        painter.text(
            canvas_rect.left_top() + Vec2::new(12.0, 12.0),
            Align2::LEFT_TOP,
            format!("Error: {error}"),
            FontId::proportional(14.0),
            Color32::LIGHT_RED,
        );
    }
}

/// Page references of the visible spread in screen order.
///
/// In RTL mode the book-order left page is drawn on the screen right, so the
/// screen-left slot gets the book-order right page (if any).
fn visible_refs(spread: &Spread, direction: Direction) -> Vec<PageRef> {
    match spread.right {
        Some(right) if direction.is_rtl() => vec![right, spread.left],
        Some(right) => vec![spread.left, right],
        None => vec![spread.left],
    }
}

/// The pixel half to fetch for a given book-order part and direction.
///
/// In RTL mode a split landscape page must show its right pixel half on the
/// screen left (where the book-order left page now sits), so the part is
/// swapped before cropping.
fn effective_part(part: PagePart, direction: Direction) -> PagePart {
    if !direction.is_rtl() || part == PagePart::Full {
        return part;
    }
    match part {
        PagePart::Left => PagePart::Right,
        PagePart::Right => PagePart::Left,
        PagePart::Full => PagePart::Full,
    }
}

/// Natural sizes for `refs`, with `None` for pages that failed to decode.
fn page_sizes(app: &mut App, refs: &[PageRef]) -> Vec<Option<Size>> {
    refs.iter()
        .map(|page_ref| app.page_size(page_ref.page).ok())
        .collect()
}

/// A previously fetched size for `page`, if `refs` already contained it.
fn size_for_page(refs: &[PageRef], natural: &[Option<Size>], page: usize) -> Option<Option<Size>> {
    refs.iter()
        .position(|page_ref| page_ref.page == page)
        .and_then(|index| natural.get(index).copied())
}

/// Draws a per-page error message inside `rect`.
///
/// The text is wrapped to the rect width and skipped when the rect is too
/// small or the wrapped text still does not fit, so it never spills onto a
/// neighbouring page.
fn paint_placeholder(painter: &eframe::egui::Painter, rect: Rect, message: &str) {
    let inner = rect.shrink(6.0);
    if inner.width() < 24.0 || inner.height() < 16.0 {
        return;
    }
    let galley = painter.layout(
        message.to_owned(),
        FontId::proportional(14.0),
        Color32::LIGHT_RED,
        inner.width(),
    );
    let size = galley.size();
    if size.x > inner.width() + 0.5 || size.y > inner.height() {
        return;
    }
    painter.galley(inner.center() - size / 2.0, galley, Color32::LIGHT_RED);
}

fn uv_full() -> Rect {
    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0))
}

/// Horizontal content extent and height for the visible pages.
///
/// A two-page spread meets at the canvas center, as the behavior spec
/// requires. A single page is centered.
fn content_bounds(targets: &[Size], ppp: f32, canvas_width: f32) -> (f32, f32, f32) {
    let width_of = |target: &Size| target.width as f32 / ppp;
    let height = targets
        .iter()
        .map(|target| target.height as f32 / ppp)
        .fold(0.0_f32, f32::max);

    if let [left, right, ..] = targets {
        let left_width = width_of(left);
        let right_width = width_of(right);
        (
            canvas_width / 2.0 - left_width,
            canvas_width / 2.0 + right_width,
            height,
        )
    } else {
        let width = targets.first().map_or(0.0, width_of);
        (
            canvas_width / 2.0 - width / 2.0,
            canvas_width / 2.0 + width / 2.0,
            height,
        )
    }
}

/// Clamp a pan offset so the content never leaves a gap at the canvas edge.
fn clamp_pan(pan: (f32, f32), canvas: Vec2, content: Rect) -> (f32, f32) {
    let x = if content.width() > canvas.x {
        pan.0.clamp(canvas.x - content.right(), -content.left())
    } else {
        0.0
    };
    let y = if content.height() > canvas.y {
        pan.1.clamp(canvas.y - content.bottom(), -content.top())
    } else {
        0.0
    };
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(width: u32, height: u32) -> Size {
        Size::new(width, height)
    }

    #[test]
    fn two_page_content_meets_at_the_center() {
        let (left, right, height) =
            content_bounds(&[target(200, 300), target(200, 300)], 1.0, 800.0);
        assert_eq!(left, 200.0);
        assert_eq!(right, 600.0);
        assert_eq!(height, 300.0);
    }

    #[test]
    fn pan_is_clamped_to_the_content_edges() {
        let canvas = Vec2::new(800.0, 600.0);
        let content = Rect::from_min_max(Pos2::new(-200.0, 0.0), Pos2::new(1000.0, 900.0));
        let (x, y) = clamp_pan((5000.0, 5000.0), canvas, content);
        assert_eq!(x, 200.0);
        assert_eq!(y, 0.0);

        let (x, y) = clamp_pan((-5000.0, -5000.0), canvas, content);
        assert_eq!(x, -200.0);
        assert_eq!(y, -300.0);
    }

    #[test]
    fn fitting_content_ignores_pan() {
        let canvas = Vec2::new(800.0, 600.0);
        let content = Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(700.0, 500.0));
        assert_eq!(clamp_pan((50.0, 50.0), canvas, content), (0.0, 0.0));
    }

    #[test]
    fn visible_refs_ltr_keeps_book_order() {
        use comical_core::{PageRef, Spread};
        let spread = Spread {
            left: PageRef {
                page: 0,
                part: PagePart::Full,
            },
            right: Some(PageRef {
                page: 1,
                part: PagePart::Full,
            }),
        };
        let refs = visible_refs(&spread, Direction::Ltr);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].page, 0);
        assert_eq!(refs[1].page, 1);
    }

    #[test]
    fn visible_refs_rtl_swaps_screen_order() {
        use comical_core::{PageRef, Spread};
        let spread = Spread {
            left: PageRef {
                page: 0,
                part: PagePart::Full,
            },
            right: Some(PageRef {
                page: 1,
                part: PagePart::Full,
            }),
        };
        let refs = visible_refs(&spread, Direction::Rtl);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].page, 1);
        assert_eq!(refs[1].page, 0);
    }

    #[test]
    fn visible_refs_single_page_unchanged_by_direction() {
        use comical_core::{PageRef, Spread};
        let spread = Spread {
            left: PageRef {
                page: 3,
                part: PagePart::Full,
            },
            right: None,
        };
        let ltr = visible_refs(&spread, Direction::Ltr);
        let rtl = visible_refs(&spread, Direction::Rtl);
        assert_eq!(ltr, rtl);
        assert_eq!(ltr.len(), 1);
        assert_eq!(ltr[0].page, 3);
    }

    #[test]
    fn effective_part_ltr_preserves_parts() {
        assert_eq!(
            effective_part(PagePart::Left, Direction::Ltr),
            PagePart::Left
        );
        assert_eq!(
            effective_part(PagePart::Right, Direction::Ltr),
            PagePart::Right
        );
        assert_eq!(
            effective_part(PagePart::Full, Direction::Ltr),
            PagePart::Full
        );
    }

    #[test]
    fn effective_part_rtl_swaps_left_and_right() {
        assert_eq!(
            effective_part(PagePart::Left, Direction::Rtl),
            PagePart::Right
        );
        assert_eq!(
            effective_part(PagePart::Right, Direction::Rtl),
            PagePart::Left
        );
        assert_eq!(
            effective_part(PagePart::Full, Direction::Rtl),
            PagePart::Full
        );
    }

    /// Determines whether a spread should show two separate labels or one centered label.
    /// Returns true if the spread has two distinct pages (different page indices).
    fn is_two_distinct_pages(spread: &Spread) -> bool {
        spread
            .right
            .is_some_and(|right| right.page != spread.left.page)
    }

    #[test]
    fn two_distinct_pages_shows_separate_labels() {
        use comical_core::{PageRef, Spread};
        let spread = Spread {
            left: PageRef {
                page: 0,
                part: PagePart::Full,
            },
            right: Some(PageRef {
                page: 1,
                part: PagePart::Full,
            }),
        };
        assert!(is_two_distinct_pages(&spread));
    }

    #[test]
    fn single_page_shows_centered_label() {
        use comical_core::{PageRef, Spread};
        let spread = Spread {
            left: PageRef {
                page: 3,
                part: PagePart::Full,
            },
            right: None,
        };
        assert!(!is_two_distinct_pages(&spread));
    }

    #[test]
    fn split_landscape_shows_centered_label() {
        use comical_core::{PageRef, Spread};
        // Split landscape: same page index, different parts
        let spread = Spread {
            left: PageRef {
                page: 2,
                part: PagePart::Left,
            },
            right: Some(PageRef {
                page: 2,
                part: PagePart::Right,
            }),
        };
        assert!(!is_two_distinct_pages(&spread));
    }

    #[test]
    fn visible_refs_screen_order_determines_label_positions_ltr() {
        use comical_core::{PageRef, Spread};
        // In LTR, book-order left (page 0) is on screen left, book-order right (page 1) is on screen right.
        // So screen-left label shows page 0, screen-right label shows page 1.
        let spread = Spread {
            left: PageRef {
                page: 0,
                part: PagePart::Full,
            },
            right: Some(PageRef {
                page: 1,
                part: PagePart::Full,
            }),
        };
        let refs = visible_refs(&spread, Direction::Ltr);
        assert_eq!(refs[0].page, 0); // screen left
        assert_eq!(refs[1].page, 1); // screen right
    }

    #[test]
    fn visible_refs_screen_order_determines_label_positions_rtl() {
        use comical_core::{PageRef, Spread};
        // In RTL, book-order left (page 0) is on screen RIGHT, book-order right (page 1) is on screen LEFT.
        // So screen-left label shows page 1, screen-right label shows page 0.
        let spread = Spread {
            left: PageRef {
                page: 0,
                part: PagePart::Full,
            },
            right: Some(PageRef {
                page: 1,
                part: PagePart::Full,
            }),
        };
        let refs = visible_refs(&spread, Direction::Rtl);
        assert_eq!(refs[0].page, 1); // screen left (book-order right)
        assert_eq!(refs[1].page, 0); // screen right (book-order left)
    }
}
