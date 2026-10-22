//! Reading direction for two-page spreads.

/// The visual reading direction of a two-page spread.
///
/// `Ltr` (left-to-right) places book-order left on the screen left, which is
/// the default for Western comics. `Rtl` (right-to-left) swaps the screen
/// sides so book-order left appears on the screen right, matching manga and
/// other right-bound publications.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Book-order left page on the screen left (Western default).
    #[default]
    Ltr,
    /// Book-order left page on the screen right (manga / right-bound).
    Rtl,
}

impl Direction {
    /// Returns the opposite direction.
    pub fn toggled(self) -> Self {
        match self {
            Self::Ltr => Self::Rtl,
            Self::Rtl => Self::Ltr,
        }
    }

    /// Whether this is right-to-left.
    pub fn is_rtl(self) -> bool {
        matches!(self, Self::Rtl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_ltr() {
        assert_eq!(Direction::default(), Direction::Ltr);
    }

    #[test]
    fn toggle_swaps_direction() {
        assert_eq!(Direction::Ltr.toggled(), Direction::Rtl);
        assert_eq!(Direction::Rtl.toggled(), Direction::Ltr);
    }

    #[test]
    fn is_rtl_matches_variant() {
        assert!(!Direction::Ltr.is_rtl());
        assert!(Direction::Rtl.is_rtl());
    }
}
