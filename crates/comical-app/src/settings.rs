//! Persistent application settings stored as versioned TOML.
//!
//! Settings are loaded from `<config_dir>/comical/settings.toml` on startup
//! and saved atomically on exit. A missing or corrupt file falls back to
//! defaults without crashing.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use comical_core::{Direction, ViewMode, ZoomMode};
use comical_image::Filter;

/// Current settings schema version. Bump when adding migration logic.
const SETTINGS_VERSION: u32 = 1;

/// Persistent application settings matching spec section 10.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Schema version for future migration.
    #[serde(default = "default_version")]
    pub version: u32,

    /// Decoded-page cache size in bytes.
    #[serde(default = "default_cache_bytes")]
    pub cache_bytes: u64,

    /// Budget for uploaded page textures, in bytes.
    #[serde(default = "default_resample_bytes")]
    pub resample_bytes: u64,

    /// How pages are scaled to the canvas.
    #[serde(default = "default_zoom", with = "zoom_mode_serde")]
    pub zoom: ZoomMode,

    /// Custom zoom percentage (1–200).
    #[serde(default = "default_zoom_percent")]
    pub zoom_percent: u32,

    /// Only fit pages larger than the viewport.
    #[serde(default)]
    pub fit_only_oversize: bool,

    /// Resampling filter.
    #[serde(default = "default_filter", with = "filter_serde")]
    pub filter: Filter,

    /// One-page or two-page layout.
    #[serde(default = "default_mode", with = "view_mode_serde")]
    pub mode: ViewMode,

    /// Reading direction.
    #[serde(default = "default_direction", with = "direction_serde")]
    pub direction: Direction,

    /// Window width in logical points.
    #[serde(default = "default_window_width")]
    pub window_width: u32,

    /// Window height in logical points.
    #[serde(default = "default_window_height")]
    pub window_height: u32,

    /// Window X position.
    #[serde(default = "default_window_x")]
    pub window_x: i32,

    /// Window Y position.
    #[serde(default = "default_window_y")]
    pub window_y: i32,

    /// Whether the toolbar is visible.
    #[serde(default = "default_true")]
    pub toolbar_visible: bool,

    /// Whether the thumbnail browser is visible.
    #[serde(default = "default_true")]
    pub browser_visible: bool,

    /// Last directory opened by the file dialog.
    #[serde(default)]
    pub last_directory: String,

    /// Use natural (numeric-aware) sort order.
    #[serde(default)]
    pub natural_sort: bool,

    /// Respect EXIF orientation tags.
    #[serde(default)]
    pub exif_orientation: bool,

    /// Remember the last page read per book.
    #[serde(default)]
    pub remember_page: bool,

    /// Last page read per book, keyed by canonical path.
    ///
    /// Only maintained when `remember_page` is on. Bounded so the settings
    /// file cannot grow without limit.
    #[serde(default)]
    pub last_pages: BTreeMap<String, usize>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            cache_bytes: default_cache_bytes(),
            resample_bytes: default_resample_bytes(),
            zoom: ZoomMode::Width,
            zoom_percent: default_zoom_percent(),
            fit_only_oversize: false,
            filter: Filter::CatmullRom,
            mode: ViewMode::TwoPage,
            direction: Direction::Ltr,
            window_width: default_window_width(),
            window_height: default_window_height(),
            window_x: default_window_x(),
            window_y: default_window_y(),
            toolbar_visible: true,
            browser_visible: true,
            last_directory: String::new(),
            natural_sort: false,
            exif_orientation: false,
            remember_page: false,
            last_pages: BTreeMap::new(),
        }
    }
}

impl Settings {
    /// Load settings from the platform config directory.
    ///
    /// Returns defaults if the file is missing or cannot be parsed.
    pub fn load() -> Self {
        let Some(path) = settings_path() else {
            eprintln!("comical: could not determine config directory, using defaults");
            return Self::default();
        };
        match fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str::<Settings>(&contents) {
                Ok(settings) => settings,
                Err(error) => {
                    eprintln!(
                        "comical: failed to parse {}: {error}, using defaults",
                        path.display()
                    );
                    Self::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                eprintln!(
                    "comical: failed to read {}: {error}, using defaults",
                    path.display()
                );
                Self::default()
            }
        }
    }

    /// Save settings atomically to the platform config directory.
    ///
    /// Writes to a temporary file first, then renames it into place.
    /// Creates the config directory if it doesn't exist.
    pub fn save(&self) {
        let Some(path) = settings_path() else {
            eprintln!("comical: could not determine config directory, settings not saved");
            return;
        };
        if let Some(parent) = path.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            eprintln!(
                "comical: failed to create config directory {}: {error}",
                parent.display()
            );
            return;
        }
        let tmp_path = path.with_extension("toml.tmp");
        let serialized = match toml::to_string_pretty(self) {
            Ok(s) => s,
            Err(error) => {
                eprintln!("comical: failed to serialize settings: {error}");
                return;
            }
        };
        let mut file = match fs::File::create(&tmp_path) {
            Ok(f) => f,
            Err(error) => {
                eprintln!(
                    "comical: failed to create temp file {}: {error}",
                    tmp_path.display()
                );
                return;
            }
        };
        if let Err(error) = file.write_all(serialized.as_bytes()) {
            eprintln!(
                "comical: failed to write temp file {}: {error}",
                tmp_path.display()
            );
            let _ = fs::remove_file(&tmp_path);
            return;
        }
        drop(file);
        let rename = fs::rename(&tmp_path, &path);
        #[cfg(windows)]
        let rename = match rename {
            Err(error)
                if error.kind() == std::io::ErrorKind::AlreadyExists
                    || error.kind() == std::io::ErrorKind::PermissionDenied =>
            {
                // Windows cannot atomically replace an existing file. Removing
                // the old settings first is still better than losing the save
                // entirely; the next launch falls back to defaults if the
                // second rename fails.
                fs::remove_file(&path).and_then(|()| fs::rename(&tmp_path, &path))
            }
            other => other,
        };
        if let Err(error) = rename {
            eprintln!(
                "comical: failed to rename {} to {}: {error}",
                tmp_path.display(),
                path.display()
            );
            let _ = fs::remove_file(&tmp_path);
        }
    }
}

/// Returns the path to the settings file.
fn settings_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "comical")
        .map(|dirs| dirs.config_dir().join("settings.toml"))
}

// --- Default value functions for serde ---

fn default_version() -> u32 {
    SETTINGS_VERSION
}

fn default_cache_bytes() -> u64 {
    805_306_368
}

fn default_resample_bytes() -> u64 {
    268_435_456
}

fn default_zoom() -> ZoomMode {
    ZoomMode::Width
}

fn default_zoom_percent() -> u32 {
    100
}

fn default_filter() -> Filter {
    Filter::CatmullRom
}

fn default_mode() -> ViewMode {
    ViewMode::TwoPage
}

fn default_direction() -> Direction {
    Direction::Ltr
}

fn default_window_width() -> u32 {
    600
}

fn default_window_height() -> u32 {
    400
}

fn default_window_x() -> i32 {
    50
}

fn default_window_y() -> i32 {
    50
}

fn default_true() -> bool {
    true
}

// --- Serde modules for enums that don't derive Serialize/Deserialize ---

mod zoom_mode_serde {
    use super::ZoomMode;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &ZoomMode, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match value {
            ZoomMode::Fit => "fit",
            ZoomMode::Width => "width",
            ZoomMode::Height => "height",
            ZoomMode::Full => "full",
            ZoomMode::Custom => "custom",
        };
        serializer.serialize_str(s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ZoomMode, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "fit" => Ok(ZoomMode::Fit),
            "width" => Ok(ZoomMode::Width),
            "height" => Ok(ZoomMode::Height),
            "full" => Ok(ZoomMode::Full),
            "custom" => Ok(ZoomMode::Custom),
            _ => Ok(ZoomMode::Width),
        }
    }
}

mod view_mode_serde {
    use super::ViewMode;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &ViewMode, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match value {
            ViewMode::OnePage => "one_page",
            ViewMode::TwoPage => "two_page",
            ViewMode::Webtoon => "webtoon",
        };
        serializer.serialize_str(s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ViewMode, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "one_page" => Ok(ViewMode::OnePage),
            "two_page" => Ok(ViewMode::TwoPage),
            "webtoon" => Ok(ViewMode::Webtoon),
            _ => Ok(ViewMode::TwoPage),
        }
    }
}

mod direction_serde {
    use super::Direction;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &Direction, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match value {
            Direction::Ltr => "ltr",
            Direction::Rtl => "rtl",
        };
        serializer.serialize_str(s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Direction, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "ltr" => Ok(Direction::Ltr),
            "rtl" => Ok(Direction::Rtl),
            _ => Ok(Direction::Ltr),
        }
    }
}

mod filter_serde {
    use super::Filter;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &Filter, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = match value {
            Filter::Box => "box",
            Filter::Bilinear => "bilinear",
            Filter::Bicubic => "bicubic",
            Filter::BSpline => "bspline",
            Filter::CatmullRom => "catmull_rom",
            Filter::Lanczos3 => "lanczos3",
        };
        serializer.serialize_str(s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Filter, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "box" => Ok(Filter::Box),
            "bilinear" => Ok(Filter::Bilinear),
            "bicubic" => Ok(Filter::Bicubic),
            "bspline" => Ok(Filter::BSpline),
            "catmull_rom" => Ok(Filter::CatmullRom),
            "lanczos3" => Ok(Filter::Lanczos3),
            _ => Ok(Filter::CatmullRom),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec_section_10() {
        let s = Settings::default();
        assert_eq!(s.version, 1);
        assert_eq!(s.cache_bytes, 805_306_368);
        assert_eq!(s.resample_bytes, 268_435_456);
        assert_eq!(s.zoom, ZoomMode::Width);
        assert_eq!(s.zoom_percent, 100);
        assert!(!s.fit_only_oversize);
        assert_eq!(s.filter, Filter::CatmullRom);
        assert_eq!(s.mode, ViewMode::TwoPage);
        assert_eq!(s.direction, Direction::Ltr);
        assert_eq!(s.window_width, 600);
        assert_eq!(s.window_height, 400);
        assert_eq!(s.window_x, 50);
        assert_eq!(s.window_y, 50);
        assert!(s.toolbar_visible);
        assert!(s.browser_visible);
        assert_eq!(s.last_directory, "");
        assert!(!s.natural_sort);
        assert!(!s.exif_orientation);
        assert!(!s.remember_page);
    }

    #[test]
    fn serialization_round_trip_preserves_all_fields() {
        let original = Settings {
            version: 1,
            cache_bytes: 1_000_000,
            resample_bytes: 500_000,
            zoom: ZoomMode::Custom,
            zoom_percent: 150,
            fit_only_oversize: true,
            filter: Filter::Lanczos3,
            mode: ViewMode::OnePage,
            direction: Direction::Rtl,
            window_width: 1920,
            window_height: 1080,
            window_x: 100,
            window_y: 200,
            toolbar_visible: false,
            browser_visible: false,
            last_directory: "/home/user/comics".to_owned(),
            natural_sort: true,
            exif_orientation: true,
            remember_page: true,
            last_pages: BTreeMap::from([("/home/user/comics/a.cbz".to_owned(), 7)]),
        };

        let serialized = toml::to_string_pretty(&original).expect("serialize");
        let restored: Settings = toml::from_str(&serialized).expect("deserialize");

        assert_eq!(restored.version, original.version);
        assert_eq!(restored.cache_bytes, original.cache_bytes);
        assert_eq!(restored.resample_bytes, original.resample_bytes);
        assert_eq!(restored.zoom, original.zoom);
        assert_eq!(restored.zoom_percent, original.zoom_percent);
        assert_eq!(restored.fit_only_oversize, original.fit_only_oversize);
        assert_eq!(restored.filter, original.filter);
        assert_eq!(restored.mode, original.mode);
        assert_eq!(restored.direction, original.direction);
        assert_eq!(restored.window_width, original.window_width);
        assert_eq!(restored.window_height, original.window_height);
        assert_eq!(restored.window_x, original.window_x);
        assert_eq!(restored.window_y, original.window_y);
        assert_eq!(restored.toolbar_visible, original.toolbar_visible);
        assert_eq!(restored.browser_visible, original.browser_visible);
        assert_eq!(restored.last_directory, original.last_directory);
        assert_eq!(restored.natural_sort, original.natural_sort);
        assert_eq!(restored.exif_orientation, original.exif_orientation);
        assert_eq!(restored.remember_page, original.remember_page);
        assert_eq!(restored.last_pages, original.last_pages);
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let result: Result<Settings, _> = toml::from_str("this is not valid toml {{{");
        assert!(result.is_err());
        // The load() method handles this gracefully; here we just verify
        // that parsing garbage fails as expected.
    }

    #[test]
    fn missing_fields_use_defaults() {
        // An empty TOML document should produce defaults via #[serde(default)].
        let settings: Settings = toml::from_str("").expect("empty doc parses");
        let defaults = Settings::default();
        assert_eq!(settings.version, defaults.version);
        assert_eq!(settings.zoom, defaults.zoom);
        assert_eq!(settings.mode, defaults.mode);
        assert_eq!(settings.direction, defaults.direction);
        assert_eq!(settings.filter, defaults.filter);
        assert_eq!(settings.window_width, defaults.window_width);
        assert!(settings.toolbar_visible);
        assert!(settings.browser_visible);
    }

    #[test]
    fn unknown_enum_values_fall_back_to_defaults() {
        let toml_str = r#"
            zoom = "nonexistent"
            mode = "three_page"
            direction = "up"
            filter = "magic"
        "#;
        let settings: Settings = toml::from_str(toml_str).expect("parse with unknown values");
        assert_eq!(settings.zoom, ZoomMode::Width);
        assert_eq!(settings.mode, ViewMode::TwoPage);
        assert_eq!(settings.direction, Direction::Ltr);
        assert_eq!(settings.filter, Filter::CatmullRom);
    }

    #[test]
    fn serialized_toml_contains_expected_keys() {
        let settings = Settings::default();
        let serialized = toml::to_string_pretty(&settings).expect("serialize");
        assert!(serialized.contains("cache_bytes"));
        assert!(serialized.contains("resample_bytes"));
        assert!(serialized.contains("zoom"));
        assert!(serialized.contains("zoom_percent"));
        assert!(serialized.contains("fit_only_oversize"));
        assert!(serialized.contains("filter"));
        assert!(serialized.contains("mode"));
        assert!(serialized.contains("direction"));
        assert!(serialized.contains("window_width"));
        assert!(serialized.contains("window_height"));
        assert!(serialized.contains("toolbar_visible"));
        assert!(serialized.contains("browser_visible"));
        assert!(serialized.contains("last_directory"));
        assert!(serialized.contains("natural_sort"));
        assert!(serialized.contains("exif_orientation"));
        assert!(serialized.contains("remember_page"));
        assert!(serialized.contains("last_pages"));
        assert!(serialized.contains("version"));
    }
}
