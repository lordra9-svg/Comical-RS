//! Page orientation and the two-page spread state machine.
//!
//! The state machine works in book order. Right-to-left reading is a drawing
//! concern and does not change any reference stored here.

use std::error::Error;
use std::fmt;

/// Orientation of a page after rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PageOrientation {
    /// Height is greater than or equal to width.
    Portrait,
    /// Width is greater than height.
    Landscape,
}

impl PageOrientation {
    /// Returns true for pages that are split across two halves.
    pub const fn is_landscape(self) -> bool {
        matches!(self, Self::Landscape)
    }

    /// Returns true for pages that occupy one page slot.
    pub const fn is_portrait(self) -> bool {
        matches!(self, Self::Portrait)
    }
}

/// Which part of a page is shown in one page slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PagePart {
    /// The whole page.
    Full,
    /// The left half of a landscape page.
    Left,
    /// The right half of a landscape page.
    Right,
}

/// A page index together with the part shown in a spread.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageRef {
    /// Book-order page index.
    pub page: usize,
    /// Visible part of that page.
    pub part: PagePart,
}

impl PageRef {
    /// Creates a page reference.
    pub const fn new(page: usize, part: PagePart) -> Self {
        Self { page, part }
    }
}

/// What is visible in a spread, in book order.
///
/// `left` is always present. In one-page mode `right` is always absent. In
/// two-page mode it is absent only for a one-page book; ordinary spreads track
/// the facing page, including the first and last alignment rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spread {
    /// Left page reference in book order.
    pub left: PageRef,
    /// Right page reference in book order, if any.
    pub right: Option<PageRef>,
}

/// How pages are presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewMode {
    /// One page at a time.
    OnePage,
    /// One or two pages at a time, using spread rules.
    TwoPage,
    /// Continuous vertical scroll through all pages (webtoon mode).
    Webtoon,
}

/// Errors from constructing a spread machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpreadError {
    /// A book must contain at least one page.
    EmptyBook,
    /// A restored spread refers outside the book or contradicts the mode.
    InvalidSpread,
}

impl fmt::Display for SpreadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBook => formatter.write_str("book has no pages"),
            Self::InvalidSpread => formatter.write_str("spread is not valid for this book"),
        }
    }
}

impl Error for SpreadError {}

/// Spread state for one book.
///
/// The current page is always the book-order index of the left page. Calling a
/// navigation method replaces the current spread. Operations clamp at the book
/// boundaries instead of returning errors.
#[derive(Debug, Clone)]
pub struct SpreadState {
    pages: Vec<PageOrientation>,
    mode: ViewMode,
    spread: Spread,
}

impl SpreadState {
    /// Creates a state positioned at the first spread.
    pub fn new(pages: Vec<PageOrientation>, mode: ViewMode) -> Result<Self, SpreadError> {
        if pages.is_empty() {
            return Err(SpreadError::EmptyBook);
        }
        let spread = first_spread(&pages, mode);
        Ok(Self {
            pages,
            mode,
            spread,
        })
    }

    /// Restores a state at a known spread.
    ///
    /// This is useful when a spread is reconstructed from settings or a
    /// session. Page references are bounds-checked; the part values are trusted
    /// because split pages legitimately start on either half.
    pub fn with_spread(
        pages: Vec<PageOrientation>,
        mode: ViewMode,
        spread: Spread,
    ) -> Result<Self, SpreadError> {
        if pages.is_empty() {
            return Err(SpreadError::EmptyBook);
        }
        if spread.left.page >= pages.len() {
            return Err(SpreadError::InvalidSpread);
        }
        if let Some(right) = spread.right {
            if right.page >= pages.len() || spread.left.page > right.page {
                return Err(SpreadError::InvalidSpread);
            }
            if mode == ViewMode::OnePage {
                return Err(SpreadError::InvalidSpread);
            }
        }
        if mode == ViewMode::OnePage && spread.left.part != PagePart::Full {
            return Err(SpreadError::InvalidSpread);
        }
        Ok(Self {
            pages,
            mode,
            spread,
        })
    }

    /// Page orientations in book order.
    pub fn pages(&self) -> &[PageOrientation] {
        &self.pages
    }

    /// Current view mode.
    pub const fn mode(&self) -> ViewMode {
        self.mode
    }

    /// Current spread.
    pub const fn spread(&self) -> Spread {
        self.spread
    }

    /// Current page index, which is always the left page.
    pub const fn current_page(&self) -> usize {
        self.spread.left.page
    }

    /// Moves to the first spread.
    pub fn first(&mut self) {
        self.spread = first_spread(&self.pages, self.mode);
    }

    /// Moves to the last spread.
    pub fn last(&mut self) {
        self.spread = last_spread(&self.pages, self.mode);
    }

    /// Advances by one spread.
    pub fn next_turn(&mut self) {
        self.spread = match self.mode {
            ViewMode::OnePage | ViewMode::Webtoon => {
                one_page_spread(next_page(self.current_page(), self.pages.len()))
            }
            ViewMode::TwoPage => self.next_turn_two_page(),
        };
    }

    /// Retreats by one spread.
    pub fn prev_turn(&mut self) {
        self.spread = match self.mode {
            ViewMode::OnePage | ViewMode::Webtoon => {
                one_page_spread(previous_page(self.current_page()))
            }
            ViewMode::TwoPage => self.prev_turn_two_page(),
        };
    }

    /// Moves forward by one page.
    pub fn next_page(&mut self) {
        self.spread = match self.mode {
            ViewMode::OnePage | ViewMode::Webtoon => {
                one_page_spread(next_page(self.current_page(), self.pages.len()))
            }
            ViewMode::TwoPage => self.next_page_two_page(),
        };
    }

    /// Moves backward by one page.
    pub fn prev_page(&mut self) {
        self.spread = match self.mode {
            ViewMode::OnePage | ViewMode::Webtoon => {
                one_page_spread(previous_page(self.current_page()))
            }
            ViewMode::TwoPage => self.prev_page_two_page(),
        };
    }

    /// Goes to a page, clamping the request to the book.
    pub fn go_to_page(&mut self, page: usize) {
        let last = self.pages.len() - 1;
        let page = page.min(last);
        self.spread = match self.mode {
            ViewMode::OnePage | ViewMode::Webtoon => one_page_spread(page),
            ViewMode::TwoPage if page == 0 => first_spread(&self.pages, ViewMode::TwoPage),
            ViewMode::TwoPage if page == last => last_spread(&self.pages, ViewMode::TwoPage),
            ViewMode::TwoPage => start_spread(&self.pages, page),
        };
    }

    /// Switches view mode while keeping the current page.
    pub fn set_mode(&mut self, mode: ViewMode) {
        if mode == self.mode {
            return;
        }
        let page = self.current_page();
        let last = self.pages.len() - 1;
        self.mode = mode;
        self.spread = match mode {
            ViewMode::OnePage | ViewMode::Webtoon => one_page_spread(page),
            ViewMode::TwoPage if page == 0 => first_spread(&self.pages, ViewMode::TwoPage),
            ViewMode::TwoPage if page == last => last_spread(&self.pages, ViewMode::TwoPage),
            ViewMode::TwoPage => start_spread(&self.pages, page),
        };
    }

    /// Cycles through one-page, two-page and webtoon modes.
    pub fn toggle_mode(&mut self) {
        self.set_mode(match self.mode {
            ViewMode::OnePage => ViewMode::TwoPage,
            ViewMode::TwoPage => ViewMode::Webtoon,
            ViewMode::Webtoon => ViewMode::OnePage,
        });
    }

    fn next_turn_two_page(&self) -> Spread {
        let Some(right) = self.spread.right else {
            return last_spread(&self.pages, ViewMode::TwoPage);
        };

        // Boundary pages end the book before the half-page rule applies. The
        // penultimate page still continues when only its left half is shown.
        let last = self.pages.len() - 1;
        if right.page >= last || (right.page + 1 == last && right.part != PagePart::Left) {
            return last_spread(&self.pages, ViewMode::TwoPage);
        }

        // When the right page is only the left half of a landscape page, that
        // page continues on the left as its right half and the next page joins
        // it on the right.
        if right.part == PagePart::Left && self.pages[right.page].is_landscape() {
            return Spread {
                left: PageRef::new(right.page, PagePart::Right),
                right: Some(page_ref(right.page + 1, &self.pages)),
            };
        }

        self.advance_after(right.page)
    }

    fn advance_after(&self, page: usize) -> Spread {
        let next = page + 1;
        if next >= self.pages.len() - 1 {
            last_spread(&self.pages, ViewMode::TwoPage)
        } else {
            start_spread(&self.pages, next)
        }
    }

    fn prev_turn_two_page(&self) -> Spread {
        let left = self.spread.left;
        // The leading edge is checked before the half-page rule, so a right
        // half at page 0 retreats to the first spread rather than reopening the
        // split page.
        if left.page == 0 || (left.page == 1 && left.part != PagePart::Right) {
            return first_spread(&self.pages, ViewMode::TwoPage);
        }

        if left.part == PagePart::Right {
            let previous = left.page - 1;
            return Spread {
                left: previous_page_ref(previous, &self.pages),
                right: Some(PageRef::new(left.page, PagePart::Left)),
            };
        }

        let previous = left.page - 1;
        if self.pages[previous].is_landscape() {
            return Spread {
                left: PageRef::new(previous, PagePart::Left),
                right: Some(PageRef::new(previous, PagePart::Right)),
            };
        }

        match previous.checked_sub(1) {
            Some(before) => Spread {
                left: previous_page_ref(before, &self.pages),
                right: Some(PageRef::new(previous, PagePart::Full)),
            },
            None => first_spread(&self.pages, ViewMode::TwoPage),
        }
    }

    fn next_page_two_page(&self) -> Spread {
        let Some(right) = self.spread.right else {
            return last_spread(&self.pages, ViewMode::TwoPage);
        };
        if right.part == PagePart::Left && self.pages[right.page].is_landscape() {
            return Spread {
                left: PageRef::new(right.page, PagePart::Left),
                right: Some(PageRef::new(right.page, PagePart::Right)),
            };
        }

        let next = right.page + 1;
        if next >= self.pages.len() {
            return last_spread(&self.pages, ViewMode::TwoPage);
        }
        Spread {
            left: right,
            right: Some(page_ref(next, &self.pages)),
        }
    }

    fn prev_page_two_page(&self) -> Spread {
        let left = self.spread.left;
        // Page zero is the left edge of the book. A page step keeps the current
        // spread, even when the visible left page is the right half of page 0.
        if left.page == 0 {
            return self.spread;
        }

        if left.part == PagePart::Right {
            return Spread {
                left: PageRef::new(left.page, PagePart::Left),
                right: Some(PageRef::new(left.page, PagePart::Right)),
            };
        }

        let previous = left.page - 1;
        Spread {
            left: previous_page_ref(previous, &self.pages),
            right: Some(left),
        }
    }
}

fn one_page_spread(page: usize) -> Spread {
    Spread {
        left: PageRef::new(page, PagePart::Full),
        right: None,
    }
}

fn first_spread(pages: &[PageOrientation], mode: ViewMode) -> Spread {
    if mode == ViewMode::OnePage || pages.len() == 1 {
        return one_page_spread(0);
    }
    if pages[0].is_landscape() {
        return Spread {
            left: PageRef::new(0, PagePart::Left),
            right: Some(PageRef::new(0, PagePart::Right)),
        };
    }
    Spread {
        left: PageRef::new(0, PagePart::Full),
        right: Some(page_ref(1, pages)),
    }
}

fn last_spread(pages: &[PageOrientation], mode: ViewMode) -> Spread {
    let last = pages.len() - 1;
    if mode == ViewMode::OnePage || pages.len() == 1 {
        return one_page_spread(last);
    }
    if pages[last].is_landscape() {
        return Spread {
            left: PageRef::new(last, PagePart::Left),
            right: Some(PageRef::new(last, PagePart::Right)),
        };
    }
    Spread {
        left: previous_page_ref(last - 1, pages),
        right: Some(PageRef::new(last, PagePart::Full)),
    }
}

fn start_spread(pages: &[PageOrientation], page: usize) -> Spread {
    if pages[page].is_landscape() {
        return Spread {
            left: PageRef::new(page, PagePart::Left),
            right: Some(PageRef::new(page, PagePart::Right)),
        };
    }
    let following = page + 1;
    Spread {
        left: PageRef::new(page, PagePart::Full),
        right: (following < pages.len()).then(|| page_ref(following, pages)),
    }
}

fn page_ref(page: usize, pages: &[PageOrientation]) -> PageRef {
    let part = if pages[page].is_landscape() {
        PagePart::Left
    } else {
        PagePart::Full
    };
    PageRef::new(page, part)
}

fn previous_page_ref(page: usize, pages: &[PageOrientation]) -> PageRef {
    let part = if pages[page].is_landscape() {
        PagePart::Right
    } else {
        PagePart::Full
    };
    PageRef::new(page, part)
}

fn next_page(page: usize, len: usize) -> usize {
    (page + 1).min(len - 1)
}

fn previous_page(page: usize) -> usize {
    page.saturating_sub(1)
}
