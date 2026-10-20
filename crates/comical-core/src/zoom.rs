//! Zoom and fit calculations.
//!
//! Geometry uses physical pixels. Scale factors are `f32` to match the
//! behaviour contract; output dimensions use truncation and are at least one
//! pixel.

use crate::ViewMode;

/// Minimum custom zoom percentage.
pub const MIN_CUSTOM_PERCENT: u32 = 1;
/// Maximum custom zoom percentage.
pub const MAX_CUSTOM_PERCENT: u32 = 200;

/// A width and height in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Size {
    /// Horizontal size.
    pub width: u32,
    /// Vertical size.
    pub height: u32,
}

impl Size {
    /// Creates a size.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// How a page is scaled to the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZoomMode {
    /// Fit the whole page inside the usable area.
    Fit,
    /// Fill the usable width, adding a vertical scrollbar when needed.
    Width,
    /// Fill the usable height, adding a horizontal scrollbar when needed.
    Height,
    /// Draw at 1:1.
    Full,
    /// Draw at a percentage of the original size.
    Custom,
}

/// Inputs that affect scaling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoomOptions {
    /// One-page or two-page layout.
    pub mode: ViewMode,
    /// Scrollbar thickness reserved when scrolling is needed.
    pub scrollbar: u32,
    /// Selected zoom mode.
    pub zoom: ZoomMode,
    /// Percentage for [`ZoomMode::Custom`], clamped to 1 through 200.
    pub custom_percent: u32,
    /// When true, fit, width and height modes leave a non-oversized page at
    /// 1:1.
    pub fit_only_if_oversized: bool,
}

impl ZoomOptions {
    /// Creates options with the usual defaults.
    pub const fn new(mode: ViewMode, zoom: ZoomMode) -> Self {
        Self {
            mode,
            scrollbar: 0,
            zoom,
            custom_percent: 100,
            fit_only_if_oversized: false,
        }
    }
}

/// Result of a zoom calculation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoomResult {
    /// Floating-point scale factor.
    pub scale: f32,
    /// Truncated target width in pixels, at least one.
    pub width: u32,
    /// Truncated target height in pixels, at least one.
    pub height: u32,
}

/// Computes scale and target size without neighbor pages.
pub fn compute_zoom(canvas: Size, page: Size, options: ZoomOptions) -> ZoomResult {
    compute_zoom_with_neighbors(canvas, page, options, &[])
}

/// Computes scale and target size, checking adjacent pages for fit-width and
/// fit-height scrollbar consistency.
///
/// `neighbors` contains the other pages that may share a two-page layout. The
/// caller supplies only the pages that are relevant; one or two are typical.
/// Neighbor checks are ignored in one-page mode.
pub fn compute_zoom_with_neighbors(
    canvas: Size,
    page: Size,
    options: ZoomOptions,
    neighbors: &[Size],
) -> ZoomResult {
    if options.fit_only_if_oversized && !is_oversized(canvas, page, options) {
        return result(page, 1.0);
    }

    let scale = match options.zoom {
        ZoomMode::Fit => fit_scale(canvas, page, options),
        ZoomMode::Width => fit_width_scale(canvas, page, options, neighbors),
        ZoomMode::Height => fit_height_scale(canvas, page, options, neighbors),
        ZoomMode::Full => 1.0,
        ZoomMode::Custom => custom_scale(options.custom_percent),
    };
    result(page, scale)
}

/// Tests whether a page is larger than the usable area for its zoom mode.
///
/// Full and custom zoom are always oversized. The two-page width half applies
/// to portrait pages, including square pages; square pages still use the full
/// canvas width when scaling.
pub fn is_oversized(canvas: Size, page: Size, options: ZoomOptions) -> bool {
    let (usable_width, usable_height) = match options.zoom {
        ZoomMode::Fit => (oversize_width(canvas.width, page, options), canvas.height),
        ZoomMode::Width => (
            oversize_width(
                canvas.width.saturating_sub(options.scrollbar),
                page,
                options,
            ),
            canvas.height,
        ),
        ZoomMode::Height => (
            // The only dimension halved in two-page mode is width, including
            // for height fit. The 5.4 table's "or half" placement is treated
            // as this width half.
            oversize_width(canvas.width, page, options),
            canvas.height.saturating_sub(options.scrollbar),
        ),
        ZoomMode::Full | ZoomMode::Custom => return true,
    };

    page.width > usable_width || page.height > usable_height
}

fn fit_scale(canvas: Size, page: Size, options: ZoomOptions) -> f32 {
    let usable_width = fit_usable_width(canvas.width, page, options);
    if canvas.width as f32 / canvas.height as f32 > page.width as f32 / page.height as f32 {
        canvas.height as f32 / page.height as f32
    } else {
        usable_width as f32 / page.width as f32
    }
}

#[derive(Debug, Clone, Copy)]
struct FitWidthBase {
    scale: f32,
    height_fit: bool,
    scrollbar: bool,
}

fn fit_width_base(canvas: Size, page: Size, options: ZoomOptions) -> FitWidthBase {
    let usable_width = fit_usable_width(canvas.width, page, options);
    let width_scale = usable_width as f32 / page.width as f32;
    if width_scale * page.height as f32 <= canvas.height as f32 {
        return FitWidthBase {
            scale: width_scale,
            height_fit: false,
            scrollbar: false,
        };
    }

    let height_scale = canvas.height as f32 / page.height as f32;
    let scrollbar_width = fit_width_scrollbar_width(canvas.width, page, options);
    if height_scale * page.width as f32 > scrollbar_width as f32 {
        FitWidthBase {
            scale: height_scale,
            height_fit: true,
            scrollbar: false,
        }
    } else {
        FitWidthBase {
            scale: scrollbar_width as f32 / page.width as f32,
            height_fit: false,
            scrollbar: true,
        }
    }
}

fn fit_width_scale(canvas: Size, page: Size, options: ZoomOptions, neighbors: &[Size]) -> f32 {
    let base = fit_width_base(canvas, page, options);
    if base.height_fit
        && options.mode == ViewMode::TwoPage
        && neighbors
            .iter()
            .any(|neighbor| fit_width_base(canvas, *neighbor, options).scrollbar)
    {
        return fit_width_scrollbar_width(canvas.width, page, options) as f32 / page.width as f32;
    }
    base.scale
}

#[derive(Debug, Clone, Copy)]
struct FitHeightBase {
    scale: f32,
    width_fit: bool,
    scrollbar: bool,
}

fn fit_height_base(canvas: Size, page: Size, options: ZoomOptions) -> FitHeightBase {
    let height_scale = canvas.height as f32 / page.height as f32;
    let usable_width = fit_usable_width(canvas.width, page, options);
    if height_scale * page.width as f32 <= usable_width as f32 {
        return FitHeightBase {
            scale: height_scale,
            width_fit: false,
            scrollbar: false,
        };
    }

    let width_scale = usable_width as f32 / page.width as f32;
    let scrollbar_height = canvas.height.saturating_sub(options.scrollbar);
    if width_scale * page.height as f32 > scrollbar_height as f32 {
        FitHeightBase {
            scale: width_scale,
            width_fit: true,
            scrollbar: false,
        }
    } else {
        FitHeightBase {
            scale: scrollbar_height as f32 / page.height as f32,
            width_fit: false,
            scrollbar: true,
        }
    }
}

fn fit_height_scale(canvas: Size, page: Size, options: ZoomOptions, neighbors: &[Size]) -> f32 {
    let base = fit_height_base(canvas, page, options);
    if base.width_fit
        && options.mode == ViewMode::TwoPage
        && neighbors
            .iter()
            .any(|neighbor| fit_height_base(canvas, *neighbor, options).scrollbar)
    {
        return canvas.height.saturating_sub(options.scrollbar) as f32 / page.height as f32;
    }
    base.scale
}

fn fit_usable_width(canvas_width: u32, page: Size, options: ZoomOptions) -> u32 {
    if options.mode == ViewMode::OnePage || is_wide(page) {
        canvas_width
    } else {
        canvas_width / 2
    }
}

fn fit_width_scrollbar_width(canvas_width: u32, page: Size, options: ZoomOptions) -> u32 {
    let available = canvas_width.saturating_sub(options.scrollbar);
    if options.mode == ViewMode::TwoPage && is_portrait(page) {
        available / 2
    } else {
        available
    }
}

fn oversize_width(canvas_width: u32, page: Size, options: ZoomOptions) -> u32 {
    if options.mode == ViewMode::TwoPage && is_portrait(page) {
        canvas_width / 2
    } else {
        canvas_width
    }
}

fn custom_scale(percent: u32) -> f32 {
    percent.clamp(MIN_CUSTOM_PERCENT, MAX_CUSTOM_PERCENT) as f32 / 100.0
}

fn result(page: Size, scale: f32) -> ZoomResult {
    ZoomResult {
        scale,
        width: scaled_dimension(page.width, scale),
        height: scaled_dimension(page.height, scale),
    }
}

fn scaled_dimension(original: u32, scale: f32) -> u32 {
    ((original as f32 * scale) as u32).max(1)
}

fn is_wide(page: Size) -> bool {
    page.width >= page.height
}

fn is_portrait(page: Size) -> bool {
    page.height >= page.width
}
