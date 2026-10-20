//! Generated spread invariants for one through twelve pages.

use comical_core::{PageOrientation, PagePart, PageRef, Spread, SpreadState, ViewMode};

use PageOrientation::{Landscape, Portrait};

#[test]
fn generated_spread_invariants() {
    for len in 1..=12 {
        for pattern in patterns(len) {
            check_pattern(&pattern);
        }
    }
}

fn check_pattern(pages: &[PageOrientation]) {
    let len = pages.len();
    let mut state = SpreadState::new(pages.to_vec(), ViewMode::TwoPage).unwrap();
    verify_spread(pages, state.spread(), "new");

    state.first();
    assert_eq!(state.spread(), expected_first(pages), "first, n={len}");
    verify_spread(pages, state.spread(), "first");

    state.prev_turn();
    assert_eq!(state.spread(), expected_first(pages), "prev turn clamp");
    state.prev_page();
    assert_eq!(state.spread(), expected_first(pages), "prev page clamp");

    state.last();
    assert_eq!(state.spread(), expected_last(pages), "last, n={len}");
    verify_spread(pages, state.spread(), "last");

    state.next_turn();
    assert_eq!(state.spread(), expected_last(pages), "next turn clamp");
    state.next_page();
    assert_eq!(state.spread(), expected_last(pages), "next page clamp");

    state.go_to_page(usize::MAX);
    assert_eq!(state.spread(), expected_last(pages), "goto clamp right");
    state.go_to_page(0);
    assert_eq!(state.spread(), expected_first(pages), "goto clamp left");

    let sequence = turn_sequence(pages);
    check_turn_sequence(pages, &sequence);

    state.set_mode(ViewMode::OnePage);
    assert_one_page(&state, state.current_page());
    state.toggle_mode();
    assert_eq!(state.mode(), ViewMode::TwoPage);
    verify_spread(pages, state.spread(), "mode round trip");
}

#[test]
fn all_portrait_turn_and_page_operations_are_inverse() {
    for len in 1..=12 {
        let pages = vec![Portrait; len];
        let mut state = SpreadState::new(pages.clone(), ViewMode::TwoPage).unwrap();
        state.first();
        let first = state.spread();

        state.next_turn();
        state.prev_turn();
        assert_eq!(state.spread(), first, "turn inverse, n={len}");

        state.first();
        state.next_page();
        state.prev_page();
        assert_eq!(state.spread(), first, "page inverse, n={len}");
    }
}

#[test]
fn mode_switch_uses_edge_rules_and_keeps_the_current_page() {
    let pages = vec![Portrait, Landscape, Portrait, Portrait];
    let mut state = SpreadState::new(pages.clone(), ViewMode::TwoPage).unwrap();
    let first = state.spread();

    state.set_mode(ViewMode::OnePage);
    assert_one_page(&state, 0);
    state.set_mode(ViewMode::TwoPage);
    assert_eq!(state.spread(), first);

    state.go_to_page(1);
    let split = state.spread();
    assert_eq!(split.left, PageRef::new(1, PagePart::Left));
    assert_eq!(split.right, Some(PageRef::new(1, PagePart::Right)));
    state.set_mode(ViewMode::OnePage);
    assert_one_page(&state, 1);
    state.set_mode(ViewMode::TwoPage);
    assert_eq!(state.spread(), split);

    state.go_to_page(3);
    let last = state.spread();
    assert_eq!(last.left.page, 2);
    state.set_mode(ViewMode::OnePage);
    assert_one_page(&state, 2);
    state.set_mode(ViewMode::TwoPage);
    assert_eq!(state.spread(), last);
    assert_eq!(state.spread(), expected_last(&pages));

    state.go_to_page(3);
    state.set_mode(ViewMode::OnePage);
    state.go_to_page(3);
    state.set_mode(ViewMode::TwoPage);
    assert_eq!(state.spread(), expected_last(&pages));
}

#[test]
fn one_page_mode_keeps_one_full_page() {
    for len in 1..=12 {
        for pattern in patterns(len) {
            let mut state = SpreadState::new(pattern.clone(), ViewMode::OnePage).unwrap();
            assert_one_page(&state, 0);

            state.next_turn();
            if len > 1 {
                assert_one_page(&state, 1);
            } else {
                assert_one_page(&state, 0);
            }

            state.last();
            assert_one_page(&state, len - 1);
            state.next_page();
            assert_one_page(&state, len - 1);
            state.prev_page();
            assert_one_page(&state, len.saturating_sub(2));

            state.first();
            assert_one_page(&state, 0);
            state.prev_turn();
            assert_one_page(&state, 0);
            state.go_to_page(len / 2);
            assert_one_page(&state, len / 2);
            state.go_to_page(usize::MAX);
            assert_one_page(&state, len - 1);
        }
    }
}

fn assert_one_page(state: &SpreadState, page: usize) {
    assert_eq!(
        state.spread(),
        Spread {
            left: PageRef::new(page, PagePart::Full),
            right: None,
        }
    );
}

fn verify_spread(pages: &[PageOrientation], spread: Spread, label: &str) {
    let len = pages.len();
    assert!(spread.left.page < len, "{label}: left out of range");
    if len == 1 {
        assert_eq!(
            spread.left,
            PageRef::new(0, PagePart::Full),
            "{label}: lone page"
        );
        assert!(
            spread.right.is_none(),
            "{label}: lone page has a right page"
        );
        return;
    }
    assert!(
        spread.right.is_some(),
        "{label}: two-page spread lost right"
    );

    match spread.left.part {
        PagePart::Full => assert_eq!(pages[spread.left.page], Portrait, "{label}: full left"),
        PagePart::Left | PagePart::Right => {
            assert_eq!(pages[spread.left.page], Landscape, "{label}: half left");
        }
    }

    let Some(right) = spread.right else {
        assert_eq!(len, 1, "{label}: only a one-page book may have no right");
        assert_eq!(spread.left.part, PagePart::Full, "{label}: lone page");
        return;
    };
    assert!(right.page < len, "{label}: right out of range");
    assert!(spread.left.page <= right.page, "{label}: left after right");

    if right.page == spread.left.page {
        assert_eq!(
            pages[right.page], Landscape,
            "{label}: same page not landscape"
        );
        assert_eq!(
            spread.left.part,
            PagePart::Left,
            "{label}: same page left part"
        );
        assert_eq!(right.part, PagePart::Right, "{label}: same page right part");
    } else {
        match right.part {
            PagePart::Full => assert_eq!(pages[right.page], Portrait, "{label}: full right"),
            PagePart::Left => assert_eq!(pages[right.page], Landscape, "{label}: left half right"),
            PagePart::Right => panic!("{label}: right half on a different page"),
        }
        if spread.left.part == PagePart::Left {
            panic!("{label}: left half must be paired with its other half");
        }
    }
}

fn expected_first(pages: &[PageOrientation]) -> Spread {
    let len = pages.len();
    if len == 1 {
        return Spread {
            left: PageRef::new(0, PagePart::Full),
            right: None,
        };
    }
    if pages[0] == Landscape {
        return Spread {
            left: PageRef::new(0, PagePart::Left),
            right: Some(PageRef::new(0, PagePart::Right)),
        };
    }
    Spread {
        left: PageRef::new(0, PagePart::Full),
        right: Some(right_reference(1, pages)),
    }
}

fn expected_last(pages: &[PageOrientation]) -> Spread {
    let last = pages.len() - 1;
    if pages.len() == 1 {
        return Spread {
            left: PageRef::new(last, PagePart::Full),
            right: None,
        };
    }
    if pages[last] == Landscape {
        return Spread {
            left: PageRef::new(last, PagePart::Left),
            right: Some(PageRef::new(last, PagePart::Right)),
        };
    }
    Spread {
        left: left_reference(last - 1, pages),
        right: Some(PageRef::new(last, PagePart::Full)),
    }
}

fn right_reference(page: usize, pages: &[PageOrientation]) -> PageRef {
    PageRef::new(
        page,
        if pages[page] == Landscape {
            PagePart::Left
        } else {
            PagePart::Full
        },
    )
}

fn left_reference(page: usize, pages: &[PageOrientation]) -> PageRef {
    PageRef::new(
        page,
        if pages[page] == Landscape {
            PagePart::Right
        } else {
            PagePart::Full
        },
    )
}

fn turn_sequence(pages: &[PageOrientation]) -> Vec<Spread> {
    let mut state = SpreadState::new(pages.to_vec(), ViewMode::TwoPage).unwrap();
    state.first();
    let mut sequence = vec![state.spread()];
    loop {
        state.next_turn();
        if sequence.last() == Some(&state.spread()) {
            break;
        }
        assert!(
            sequence.len() <= pages.len() * 2 + 2,
            "turn sequence did not terminate"
        );
        sequence.push(state.spread());
    }
    sequence
}

fn check_turn_sequence(pages: &[PageOrientation], sequence: &[Spread]) {
    let len = pages.len();
    assert_eq!(sequence.first(), Some(&expected_first(pages)));
    assert_eq!(sequence.last(), Some(&expected_last(pages)));

    for (position, spread) in sequence.iter().enumerate() {
        verify_spread(pages, *spread, "turn sequence");

        if let Some(right) = spread.right
            && right.page != spread.left.page
            && right.part == PagePart::Left
        {
            let next = sequence.get(position + 1).unwrap_or_else(|| {
                panic!(
                    "landscape page {} left half has no following spread",
                    right.page
                )
            });
            assert!(
                contains(*next, PageRef::new(right.page, PagePart::Right)),
                "landscape page {} must continue on the next spread",
                right.page
            );
        }

        if spread.left.part == PagePart::Right {
            let previous = position
                .checked_sub(1)
                .and_then(|index| sequence.get(index))
                .unwrap_or_else(|| {
                    panic!(
                        "landscape page {} right half has no preceding spread",
                        spread.left.page
                    )
                });
            assert!(
                contains(*previous, PageRef::new(spread.left.page, PagePart::Left)),
                "landscape page {} right half must follow its left half",
                spread.left.page
            );
        }
    }

    if len == 1 {
        return;
    }

    for (page, orientation) in pages.iter().copied().enumerate() {
        if orientation == Landscape {
            assert!(
                sequence.iter().any(
                    |spread| contains(*spread, PageRef::new(page, PagePart::Left))
                        || contains(*spread, PageRef::new(page, PagePart::Right))
                ),
                "landscape page {page} never appears, n={len}"
            );
        }
    }
}

fn contains(spread: Spread, page: PageRef) -> bool {
    spread.left == page || spread.right == Some(page)
}

fn patterns(len: usize) -> Vec<Vec<PageOrientation>> {
    let mut result = vec![
        vec![Portrait; len],
        vec![Landscape; len],
        (0..len)
            .map(|index| if index % 2 == 0 { Landscape } else { Portrait })
            .collect(),
        (0..len)
            .map(|index| if index % 3 == 0 { Landscape } else { Portrait })
            .collect(),
    ];
    if len > 1 {
        let mut first = vec![Portrait; len];
        first[0] = Landscape;
        result.push(first);

        let mut last = vec![Portrait; len];
        last[len - 1] = Landscape;
        result.push(last);

        let mut middle = vec![Portrait; len];
        middle[len / 2] = Landscape;
        result.push(middle);
    }
    result
}
