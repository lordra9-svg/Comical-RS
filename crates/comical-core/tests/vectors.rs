//! Contract tests loaded from `tests/vectors`.

use std::fs;
use std::path::PathBuf;

use comical_core::{
    PageOrientation, PagePart, PageRef, Size, Spread, SpreadState, ViewMode, ZoomMode, ZoomOptions,
    compute_zoom_with_neighbors,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct SpreadFile {
    cases: Vec<SpreadCase>,
}

#[derive(Deserialize)]
struct SpreadCase {
    name: String,
    pages: Vec<String>,
    start: Option<SpreadJson>,
    op: OperationJson,
    expected: SpreadJson,
}

#[derive(Deserialize)]
struct SpreadJson {
    left: PageRefJson,
    right: Option<PageRefJson>,
}

#[derive(Deserialize)]
struct PageRefJson {
    page: usize,
    part: String,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OperationJson {
    First,
    Last,
    NextTurn,
    PrevTurn,
    NextPage,
    PrevPage,
    Goto { page: usize },
}

#[derive(Deserialize)]
struct ZoomFile {
    cases: Vec<ZoomCase>,
}

#[derive(Deserialize)]
struct ZoomCase {
    name: String,
    canvas: SizeJson,
    scrollbar: u32,
    mode: String,
    page: SizeJson,
    zoom: ZoomJson,
    zoom_percent: u32,
    fit_only_oversize: bool,
    #[serde(default)]
    neighbors: Vec<SizeJson>,
    expected: ZoomExpected,
}

#[derive(Deserialize)]
struct SizeJson {
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ZoomJson {
    Fit,
    Width,
    Height,
    Full,
    Custom,
}

#[derive(Deserialize)]
struct ZoomExpected {
    scale: f64,
    width: u32,
    height: u32,
}

#[test]
fn spread_vectors_match() {
    let path = vector_path("spreads.json");
    let file: SpreadFile =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).expect("spreads.json parses");

    for case in file.cases {
        let pages: Vec<PageOrientation> = case
            .pages
            .iter()
            .map(|value| match value.as_str() {
                "portrait" => PageOrientation::Portrait,
                "landscape" => PageOrientation::Landscape,
                other => panic!("{}: unknown orientation {other}", case.name),
            })
            .collect();

        let mut state = match case.start {
            Some(start) => {
                SpreadState::with_spread(pages, ViewMode::TwoPage, spread_from_json(start)).unwrap()
            }
            None => SpreadState::new(pages, ViewMode::TwoPage).unwrap(),
        };

        match case.op {
            OperationJson::First => state.first(),
            OperationJson::Last => state.last(),
            OperationJson::NextTurn => state.next_turn(),
            OperationJson::PrevTurn => state.prev_turn(),
            OperationJson::NextPage => state.next_page(),
            OperationJson::PrevPage => state.prev_page(),
            OperationJson::Goto { page } => state.go_to_page(page),
        }

        assert_eq!(
            state.spread(),
            spread_from_json(case.expected),
            "spread vector {}",
            case.name
        );
    }
}

#[test]
fn zoom_vectors_match() {
    let path = vector_path("zoom.json");
    let file: ZoomFile =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).expect("zoom.json parses");

    for case in file.cases {
        let canvas = Size::new(case.canvas.width, case.canvas.height);
        let page = Size::new(case.page.width, case.page.height);
        let options = ZoomOptions {
            mode: match case.mode.as_str() {
                "one_page" => ViewMode::OnePage,
                "two_page" => ViewMode::TwoPage,
                other => panic!("{}: unknown mode {other}", case.name),
            },
            scrollbar: case.scrollbar,
            zoom: match case.zoom {
                ZoomJson::Fit => ZoomMode::Fit,
                ZoomJson::Width => ZoomMode::Width,
                ZoomJson::Height => ZoomMode::Height,
                ZoomJson::Full => ZoomMode::Full,
                ZoomJson::Custom => ZoomMode::Custom,
            },
            custom_percent: case.zoom_percent,
            fit_only_if_oversized: case.fit_only_oversize,
        };

        let neighbors: Vec<Size> = case
            .neighbors
            .iter()
            .map(|neighbor| Size::new(neighbor.width, neighbor.height))
            .collect();
        let result = compute_zoom_with_neighbors(canvas, page, options, &neighbors);
        assert!(
            (f64::from(result.scale) - case.expected.scale).abs() <= 1e-6,
            "zoom vector {}: scale {} != {}",
            case.name,
            result.scale,
            case.expected.scale
        );
        assert_eq!(
            result.width, case.expected.width,
            "zoom vector {}: width",
            case.name
        );
        assert_eq!(
            result.height, case.expected.height,
            "zoom vector {}: height",
            case.name
        );
    }
}

fn spread_from_json(value: SpreadJson) -> Spread {
    Spread {
        left: page_ref_from_json(value.left),
        right: value.right.map(page_ref_from_json),
    }
}

fn page_ref_from_json(value: PageRefJson) -> PageRef {
    PageRef::new(
        value.page,
        match value.part.as_str() {
            "full" => PagePart::Full,
            "left" => PagePart::Left,
            "right" => PagePart::Right,
            other => panic!("unknown part {other}"),
        },
    )
}

fn vector_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/vectors")
        .join(name)
}
