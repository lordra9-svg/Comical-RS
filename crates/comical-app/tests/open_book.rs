//! Integration test: opening a real directory book decodes its first page.

use comical_app::App;
use comical_core::Size;

/// A valid 4x3 RGB PNG, four columns by three rows of opaque pixels.
const PAGE_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x03, 0x08, 0x02, 0x00, 0x00, 0x00, 0x3b, 0x96, 0x39,
    0x91, 0x00, 0x00, 0x00, 0x30, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x60, 0x60, 0x38, 0xa1,
    0xc1, 0x70, 0x22, 0x80, 0xe1, 0x44, 0x05, 0xc3, 0x09, 0x06, 0x2e, 0x9b, 0x13, 0x46, 0x36, 0x27,
    0xa2, 0x6c, 0x4e, 0x34, 0xd9, 0x9c, 0x60, 0x10, 0xa9, 0x38, 0x61, 0x53, 0x71, 0x22, 0xa5, 0xe2,
    0x44, 0x4f, 0xc5, 0x09, 0x00, 0x04, 0x4a, 0x0f, 0x79, 0xd3, 0x19, 0xe5, 0xf2, 0x00, 0x00, 0x00,
    0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

#[test]
fn opening_a_book_decodes_page_zero_at_its_natural_size() {
    let directory = tempfile::tempdir().expect("temp directory");
    std::fs::write(directory.path().join("page0.png"), PAGE_PNG).expect("write png page");
    // A non-image file must be ignored by the page list.
    std::fs::write(directory.path().join("notes.txt"), b"not a page").expect("write notes");
    let path = directory.path().to_str().expect("temp path is utf-8");

    let mut app = App::new();
    app.open_book(path).expect("open book");

    assert!(app.has_book());
    assert_eq!(app.page_count(), 1);
    assert_eq!(app.page_size(0).expect("decode page 0"), Size::new(4, 3));
}

#[test]
fn natural_sort_setting_reorders_pages_on_open() {
    let directory = tempfile::tempdir().expect("temp directory");
    for name in ["page10.png", "page2.png", "page1.png"] {
        std::fs::write(directory.path().join(name), PAGE_PNG).expect("write png page");
    }
    let path = directory.path().to_str().expect("temp path is utf-8");

    let mut byte_order = App::with_settings(comical_app::Settings::default());
    byte_order.open_book(path).expect("open book");
    let names: Vec<&str> = (0..byte_order.page_count())
        .map(|page| byte_order.page_name(page).expect("page name"))
        .collect();
    assert_eq!(names, vec!["page1.png", "page10.png", "page2.png"]);

    let mut natural = App::with_settings(comical_app::Settings {
        natural_sort: true,
        ..Default::default()
    });
    natural.open_book(path).expect("open book");
    let names: Vec<&str> = (0..natural.page_count())
        .map(|page| natural.page_name(page).expect("page name"))
        .collect();
    assert_eq!(names, vec!["page1.png", "page2.png", "page10.png"]);
}

#[test]
fn remember_page_restores_the_last_page_of_a_book() {
    let directory = tempfile::tempdir().expect("temp directory");
    for name in ["page0.png", "page1.png", "page2.png", "page3.png"] {
        std::fs::write(directory.path().join(name), PAGE_PNG).expect("write png page");
    }
    let path = directory.path().to_str().expect("temp path is utf-8");

    let mut app = App::with_settings(comical_app::Settings {
        remember_page: true,
        mode: comical_core::ViewMode::OnePage,
        ..Default::default()
    });
    app.open_book(path).expect("open book");
    app.go_to_page(2);
    assert_eq!(app.current_page(), Some(2));

    // Reopening the same book starts on the remembered page.
    app.open_book(path).expect("reopen book");
    assert_eq!(app.current_page(), Some(2));

    // Without the setting the book always starts at page 0.
    let mut fresh = App::with_settings(comical_app::Settings {
        mode: comical_core::ViewMode::OnePage,
        ..Default::default()
    });
    fresh.open_book(path).expect("open book");
    assert_eq!(fresh.current_page(), Some(0));
}

#[test]
fn filter_setting_is_stored_and_updated() {
    let directory = tempfile::tempdir().expect("temp directory");
    std::fs::write(directory.path().join("page0.png"), PAGE_PNG).expect("write png page");
    let path = directory.path().to_str().expect("temp path is utf-8");

    let mut app = App::with_settings(comical_app::Settings {
        filter: comical_image::Filter::Lanczos3,
        ..Default::default()
    });
    app.open_book(path).expect("open book");
    assert_eq!(app.settings().filter, comical_image::Filter::Lanczos3);
    app.set_filter(comical_image::Filter::Box);
    assert_eq!(app.settings().filter, comical_image::Filter::Box);
}
