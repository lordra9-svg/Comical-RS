//! Per-page rotation state for the viewer.
//!
//! Rotation is per page and not persisted. Four states represent 0, 90, 180 and
//! 270 degrees clockwise respectively.

/// A clockwise rotation applied to a page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Rotation {
    /// No rotation (0 degrees).
    #[default]
    North,
    /// 90 degrees clockwise.
    East,
    /// 180 degrees.
    South,
    /// 270 degrees clockwise (90 degrees counter-clockwise).
    West,
}

impl Rotation {
    /// Returns the next clockwise rotation.
    pub const fn clockwise(self) -> Self {
        match self {
            Self::North => Self::East,
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::North,
        }
    }

    /// Returns the next counter-clockwise rotation.
    pub const fn counter_clockwise(self) -> Self {
        match self {
            Self::North => Self::West,
            Self::West => Self::South,
            Self::South => Self::East,
            Self::East => Self::North,
        }
    }

    /// Returns true if this rotation swaps width and height.
    pub const fn swaps_dimensions(self) -> bool {
        matches!(self, Self::East | Self::West)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clockwise_cycles_through_all_states() {
        let mut r = Rotation::North;
        r = r.clockwise();
        assert_eq!(r, Rotation::East);
        r = r.clockwise();
        assert_eq!(r, Rotation::South);
        r = r.clockwise();
        assert_eq!(r, Rotation::West);
        r = r.clockwise();
        assert_eq!(r, Rotation::North);
    }

    #[test]
    fn counter_clockwise_cycles_through_all_states() {
        let mut r = Rotation::North;
        r = r.counter_clockwise();
        assert_eq!(r, Rotation::West);
        r = r.counter_clockwise();
        assert_eq!(r, Rotation::South);
        r = r.counter_clockwise();
        assert_eq!(r, Rotation::East);
        r = r.counter_clockwise();
        assert_eq!(r, Rotation::North);
    }

    #[test]
    fn swaps_dimensions_for_east_and_west_only() {
        assert!(!Rotation::North.swaps_dimensions());
        assert!(Rotation::East.swaps_dimensions());
        assert!(!Rotation::South.swaps_dimensions());
        assert!(Rotation::West.swaps_dimensions());
    }

    #[test]
    fn default_is_north() {
        assert_eq!(Rotation::default(), Rotation::North);
    }
}