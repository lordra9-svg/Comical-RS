//! Generated zoom cases beyond the seed vectors.

use comical_core::{
    Size, ViewMode, ZoomMode, ZoomOptions, compute_zoom, compute_zoom_with_neighbors,
};

#[test]
fn fit_and_full_for_portrait_landscape_square_tiny_and_huge() {
    check(
        Size::new(600, 400),
        Size::new(1000, 1500),
        options(ViewMode::OnePage, ZoomMode::Fit),
        0.266_666_67,
        266,
        400,
    );
    check(
        Size::new(600, 400),
        Size::new(1500, 1000),
        options(ViewMode::OnePage, ZoomMode::Fit),
        0.4,
        600,
        400,
    );
    check(
        Size::new(600, 400),
        Size::new(1000, 1000),
        options(ViewMode::OnePage, ZoomMode::Fit),
        0.4,
        400,
        400,
    );
    check(
        Size::new(600, 400),
        Size::new(1, 2),
        options(ViewMode::OnePage, ZoomMode::Fit),
        200.0,
        200,
        400,
    );
    check(
        Size::new(600, 400),
        Size::new(10_000, 20_000),
        options(ViewMode::OnePage, ZoomMode::Fit),
        0.02,
        200,
        400,
    );

    for page in [
        Size::new(1000, 1500),
        Size::new(1500, 1000),
        Size::new(1000, 1000),
        Size::new(1, 1),
        Size::new(20_000, 10_000),
    ] {
        check(
            Size::new(600, 400),
            page,
            options(ViewMode::OnePage, ZoomMode::Full),
            1.0,
            page.width,
            page.height,
        );
    }
}

#[test]
fn width_and_height_cover_both_page_layouts_and_orientations() {
    check(
        Size::new(600, 400),
        Size::new(1000, 1500),
        options(ViewMode::TwoPage, ZoomMode::Width),
        0.292,
        292,
        438,
    );
    check(
        Size::new(600, 400),
        Size::new(1500, 1000),
        options(ViewMode::TwoPage, ZoomMode::Width),
        0.4,
        600,
        400,
    );
    check(
        Size::new(600, 400),
        Size::new(1000, 1000),
        options(ViewMode::TwoPage, ZoomMode::Width),
        0.585,
        585,
        585,
    );
    check(
        Size::new(600, 400),
        Size::new(1000, 1500),
        options(ViewMode::TwoPage, ZoomMode::Height),
        0.266_666_67,
        266,
        400,
    );
    check(
        Size::new(600, 400),
        Size::new(1500, 1000),
        options(ViewMode::TwoPage, ZoomMode::Height),
        0.4,
        600,
        400,
    );
    check(
        Size::new(600, 400),
        Size::new(1000, 1000),
        options(ViewMode::TwoPage, ZoomMode::Height),
        0.4,
        400,
        400,
    );
}

#[test]
fn custom_percent_is_a_scale_and_clamps_to_the_configured_range() {
    for (percent, scale, width, height) in [
        (1, 0.01, 10, 15),
        (50, 0.5, 500, 750),
        (200, 2.0, 2000, 3000),
        (0, 0.01, 10, 15),
        (300, 2.0, 2000, 3000),
    ] {
        let options = ZoomOptions {
            mode: ViewMode::OnePage,
            scrollbar: 15,
            zoom: ZoomMode::Custom,
            custom_percent: percent,
            fit_only_if_oversized: false,
        };
        check(
            Size::new(600, 400),
            Size::new(1000, 1500),
            options,
            scale,
            width,
            height,
        );
    }
}

#[test]
fn fit_only_if_oversized_respects_the_square_portrait_quirk() {
    check(
        Size::new(600, 400),
        Size::new(500, 700),
        fit_only(ViewMode::OnePage, ZoomMode::Fit),
        0.571_428_6,
        285,
        400,
    );
    check(
        Size::new(600, 400),
        Size::new(300, 400),
        fit_only(ViewMode::OnePage, ZoomMode::Fit),
        1.0,
        300,
        400,
    );
    check(
        Size::new(600, 400),
        Size::new(350, 350),
        fit_only(ViewMode::TwoPage, ZoomMode::Fit),
        1.142_857_2,
        400,
        400,
    );
    check(
        Size::new(600, 400),
        Size::new(250, 250),
        fit_only(ViewMode::TwoPage, ZoomMode::Fit),
        1.0,
        250,
        250,
    );
}

#[test]
fn height_oversize_halves_the_usable_height_for_portrait_pages() {
    // The page fits the width half and the full height, so treating only the
    // width as halved would report it as not oversized and return 1:1.
    check(
        Size::new(600, 400),
        Size::new(250, 300),
        fit_only(ViewMode::TwoPage, ZoomMode::Height),
        1.283_333_3,
        320,
        385,
    );
}

#[test]
fn neighbor_fallback_applies_when_the_current_page_fits_without_scrollbars() {
    let canvas = Size::new(600, 400);

    let width_page = Size::new(300, 400);
    check_with_neighbors(
        canvas,
        width_page,
        options(ViewMode::TwoPage, ZoomMode::Width),
        &[Size::new(1400, 1000)],
        0.973_333_3,
        292,
        389,
    );
    check_with_neighbors(
        canvas,
        width_page,
        options(ViewMode::TwoPage, ZoomMode::Width),
        &[Size::new(300, 400)],
        1.0,
        300,
        400,
    );

    let height_page = Size::new(300, 300);
    check_with_neighbors(
        canvas,
        height_page,
        options(ViewMode::TwoPage, ZoomMode::Height),
        &[Size::new(2000, 1000)],
        1.283_333_3,
        385,
        385,
    );
    check_with_neighbors(
        canvas,
        height_page,
        options(ViewMode::TwoPage, ZoomMode::Height),
        &[Size::new(300, 300)],
        1.333_333_3,
        400,
        400,
    );
}

#[test]
fn fit_width_neighbor_needing_a_scrollbar_changes_the_scale() {
    let canvas = Size::new(600, 400);
    let page = Size::new(1110, 1500);
    let options = options(ViewMode::TwoPage, ZoomMode::Width);
    check(canvas, page, options, 0.266_666_67, 296, 400);

    let landscape_neighbor = Size::new(1400, 1000);
    check_with_neighbors(
        canvas,
        page,
        options,
        &[landscape_neighbor],
        0.263_063_06,
        292,
        394,
    );
}

#[test]
fn fit_height_neighbor_needing_a_scrollbar_changes_the_scale() {
    let canvas = Size::new(600, 400);
    let page = Size::new(1150, 1500);
    let options = options(ViewMode::TwoPage, ZoomMode::Height);
    check(canvas, page, options, 0.260_869_56, 300, 391);

    let landscape_neighbor = Size::new(1600, 1000);
    check_with_neighbors(
        canvas,
        page,
        options,
        &[landscape_neighbor],
        0.256_666_67,
        295,
        385,
    );
}

fn fit_only(mode: ViewMode, zoom: ZoomMode) -> ZoomOptions {
    ZoomOptions {
        fit_only_if_oversized: true,
        ..options(mode, zoom)
    }
}

fn options(mode: ViewMode, zoom: ZoomMode) -> ZoomOptions {
    ZoomOptions {
        mode,
        scrollbar: 15,
        zoom,
        custom_percent: 100,
        fit_only_if_oversized: false,
    }
}

fn check(canvas: Size, page: Size, options: ZoomOptions, scale: f32, width: u32, height: u32) {
    let result = compute_zoom(canvas, page, options);
    assert_close(result.scale, scale);
    assert_eq!(result.width, width);
    assert_eq!(result.height, height);
}

fn check_with_neighbors(
    canvas: Size,
    page: Size,
    options: ZoomOptions,
    neighbors: &[Size],
    scale: f32,
    width: u32,
    height: u32,
) {
    let result = compute_zoom_with_neighbors(canvas, page, options, neighbors);
    assert_close(result.scale, scale);
    assert_eq!(result.width, width);
    assert_eq!(result.height, height);
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 1e-6,
        "scale {actual} is not within 1e-6 of {expected}"
    );
}
