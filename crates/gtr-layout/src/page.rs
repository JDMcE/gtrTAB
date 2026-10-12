//! Paper: a laid-out score cut into pages, a whole line of music at a time.

use crate::{Align, Color, Font, Item, Layout};

/// A sheet of paper, measured in the pixels a layout is measured in: 96 to the inch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Page {
    pub width: f64,
    pub height: f64,
    /// Room left clear at the top and the bottom of a page.
    pub margin: f64,
}

impl Page {
    /// 210 by 297 millimetres.
    pub const A4: Page = Page {
        width: 794.0,
        height: 1123.0,
        margin: 44.0,
    };
    /// 8.5 by 11 inches.
    pub const LETTER: Page = Page {
        width: 816.0,
        height: 1056.0,
        margin: 44.0,
    };
}

impl Layout {
    /// The score as it goes on paper or into a picture: without the marks that are only
    /// there for whoever is writing it, the red under a bar that does not add up.
    pub fn printed(&self) -> Layout {
        let mut printed = self.clone();
        printed.items.retain(|item| {
            !matches!(
                item,
                Item::Rect {
                    c: Color::Error,
                    ..
                }
            )
        });
        for item in &mut printed.items {
            if let Item::Text { c, .. } | Item::Line { c, .. } = item
                && *c == Color::Error
            {
                *c = Color::Muted;
            }
        }
        printed
    }

    /// The score on pages of `page`'s height, for a layout made to its width: the heading
    /// and as many lines of music as fit on the first, and as many as fit on each after
    /// it. A line of music is never cut in two; one taller than a page has a page to
    /// itself and runs off the end of it. Each page is numbered if there are several.
    pub fn pages(&self, page: Page) -> Vec<Layout> {
        let printed = self.printed();
        let blank = |items: Vec<Item>| Layout {
            height: page.height,
            items,
            systems: Vec::new(),
            bars: Vec::new(),
            ..printed.clone()
        };
        if printed.systems.is_empty() {
            return vec![blank(printed.items.clone())];
        }
        // Where each line of music starts; it has the page down to where the next does.
        let tops: Vec<f64> = printed.systems.iter().map(|system| system.top).collect();
        let ends = |line: usize| tops.get(line + 1).copied().unwrap_or(printed.height);
        // The first line of each page, and how far the page's lines are moved up it.
        let mut starts: Vec<(usize, f64)> = vec![(0, 0.0)];
        let mut used = ends(0);
        for (line, top) in tops.iter().enumerate().skip(1) {
            let height = ends(line) - top;
            if used + height > page.height - page.margin {
                starts.push((line, top - page.margin));
                used = page.margin;
            }
            used += height;
        }
        let mut pages: Vec<Vec<Item>> = vec![Vec::new(); starts.len()];
        for item in &printed.items {
            // The line of music an item belongs to is the last that starts above it.
            let line = tops
                .partition_point(|top| *top <= item.top())
                .saturating_sub(1);
            let number = starts.partition_point(|(first, _)| *first <= line) - 1;
            let mut item = item.clone();
            item.shift_y(-starts[number].1);
            pages[number].push(item);
        }
        let count = pages.len();
        (pages.into_iter().enumerate())
            .map(|(number, mut items)| {
                if count > 1 {
                    items.push(Item::Text {
                        x: page.width / 2.0,
                        y: page.height - page.margin / 2.0,
                        s: format!("{} / {count}", number + 1),
                        size: 9.0,
                        font: Font::Text,
                        align: Align::Center,
                        knockout: false,
                        c: Color::Muted,
                    });
                }
                blank(items)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use gtr_model::{Bar, MasterBar, TimeSignature, demo_score};

    use super::*;
    use crate::{View, layout};

    /// The demo riff over and over, `bars` bars long.
    fn long(bars: usize) -> gtr_model::Score {
        let mut score = demo_score();
        let riff: Vec<Bar> = score.tracks[0].staves[0].bars.clone();
        score.master_bars = vec![MasterBar::new(TimeSignature::COMMON); bars];
        score.tracks[0].staves[0].bars = (0..bars).map(|bar| riff[bar % 2].clone()).collect();
        score
    }

    #[test]
    fn a_short_score_is_one_page_and_a_long_one_several() {
        let short = layout(&demo_score(), &View::default(), Page::A4.width);
        let pages = short.pages(Page::A4);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].height, Page::A4.height);
        assert_eq!(pages[0].items.len(), short.items.len());

        let whole = layout(&long(120), &View::default(), Page::A4.width);
        let pages = whole.pages(Page::A4);
        assert!(pages.len() > 3, "{}", pages.len());
        // Every item is on one page, and each page has its number as well.
        let placed: usize = pages.iter().map(|page| page.items.len()).sum();
        assert_eq!(placed, whole.items.len() + pages.len());
        for (number, page) in pages.iter().enumerate() {
            // Nothing runs off the bottom of the page, or sits above its top margin on
            // any page but the first, which has the heading.
            let lowest = page.items.iter().map(Item::top).fold(0.0, f64::max);
            assert!(lowest <= Page::A4.height, "page {number}: {lowest}");
            let highest = page.items.iter().map(Item::top).fold(f64::MAX, f64::min);
            assert!(
                number == 0 || highest >= Page::A4.margin - 1.0,
                "page {number}: {highest}"
            );
            let numbered = page.items.iter().any(|item| {
                matches!(item, Item::Text { s, .. } if *s == format!("{} / {}", number + 1, pages.len()))
            });
            assert!(numbered, "page {number}");
        }
        // The title is on the first page alone.
        let titled = |page: &Layout| {
            (page.items.iter()).any(|item| matches!(item, Item::Text { s, .. } if s == "Demo riff"))
        };
        assert!(titled(&pages[0]) && !pages[1..].iter().any(titled));
    }

    #[test]
    fn paper_has_none_of_the_marks_for_whoever_is_writing() {
        // A bar with too little in it is marked on the screen.
        let mut score = demo_score();
        score.tracks[0].staves[0].bars[1].voices[0]
            .beats
            .truncate(2);
        let shown = layout(&score, &View::default(), 800.0);
        let marked = |layout: &Layout| {
            (layout.items.iter()).any(|item| {
                matches!(
                    item,
                    Item::Rect {
                        c: Color::Error,
                        ..
                    } | Item::Text {
                        c: Color::Error,
                        ..
                    }
                )
            })
        };
        assert!(marked(&shown));
        assert!(!marked(&shown.printed()));
        assert!(!shown.pages(Page::A4).iter().any(marked));
    }
}
