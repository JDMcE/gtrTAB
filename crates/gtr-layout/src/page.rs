//! Paper: a laid-out score cut into pages, a whole line of music at a time.

use crate::display::r;
use crate::{Align, Color, Font, Item, Layout, Sheet};

/// How the pages are set out on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pages {
    /// One under another, for scrolling down.
    Down,
    /// Side by side, for scrolling across.
    Across,
}

/// Room between two pages on the screen, and around them all.
const PAGE_GAP: f64 = 24.0;

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

    /// Where each page starts: its first line of music, and how far that line and those
    /// after it are moved up to the page's top. The first page has the heading and as many
    /// lines of music as fit, and each after it as many as fit below its top margin. A line
    /// of music is never cut in two; one taller than a page has a page to itself and runs
    /// off the end of it.
    fn page_starts(&self, page: Page) -> Vec<(usize, f64)> {
        let tops: Vec<f64> = self.systems.iter().map(|system| system.top).collect();
        // A line of music has the page down to where the next one starts.
        let ends = |line: usize| tops.get(line + 1).copied().unwrap_or(self.height);
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
        starts
    }

    /// Cuts the layout into pages, each item moved by `place` from where it is to where
    /// it goes on its page, given the page's number and how far up its lines are moved.
    /// Returns each page's items, numbered if there are several.
    fn cut(&self, page: Page, place: impl Fn(usize, &mut Item)) -> Vec<Vec<Item>> {
        let starts = self.page_starts(page);
        let tops: Vec<f64> = self.systems.iter().map(|system| system.top).collect();
        let mut pages: Vec<Vec<Item>> = vec![Vec::new(); starts.len()];
        for item in &self.items {
            // The line of music an item belongs to is the last that starts above it.
            let line = tops.partition_point(|top| *top <= item.top()).saturating_sub(1);
            let number = starts.partition_point(|(first, _)| *first <= line) - 1;
            let mut item = item.clone();
            item.shift_y(-starts[number].1);
            place(number, &mut item);
            pages[number].push(item);
        }
        let count = pages.len();
        if count > 1 {
            for (number, items) in pages.iter_mut().enumerate() {
                let mut text = Item::Text {
                    x: page.width / 2.0,
                    y: page.height - page.margin / 2.0,
                    s: format!("{} / {count}", number + 1),
                    size: 9.0,
                    font: Font::Text,
                    align: Align::Center,
                    knockout: false,
                    c: Color::Muted,
                };
                place(number, &mut text);
                items.push(text);
            }
        }
        pages
    }

    /// The score on pages of `page`'s height, for a layout made to its width, as it is
    /// printed: see [`Layout::page_starts`] for where each page begins.
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
        printed.cut(page, |_, _| {}).into_iter().map(blank).collect()
    }

    /// The score on pages of `page`'s size, as they are printed, set out on the screen one
    /// under another or side by side, for a layout made to the page's width. Unlike
    /// printed pages it keeps the marks for whoever is writing, and the boxes of its lines
    /// and bars, moved with them, for the cursor and the mouse.
    pub fn sheets(&self, page: Page, set: Pages) -> Layout {
        if self.systems.is_empty() {
            return self.clone();
        }
        let starts = self.page_starts(page);
        let offset = |number: usize| {
            let (across, down) = match set {
                Pages::Down => (0.0, number as f64 * (page.height + PAGE_GAP)),
                Pages::Across => (number as f64 * (page.width + PAGE_GAP), 0.0),
            };
            (PAGE_GAP + across, PAGE_GAP + down)
        };
        let pages = self.cut(page, |number, item| {
            let (x, y) = offset(number);
            item.shift_x(x);
            item.shift_y(y);
        });
        let count = pages.len();
        let page_of = |line: usize| starts.partition_point(|(first, _)| *first <= line) - 1;
        // Where a line's page puts it: how far across, and how far down.
        let moved = |line: usize| {
            let number = page_of(line);
            let (x, y) = offset(number);
            (x, y - starts[number].1)
        };

        let mut systems = self.systems.clone();
        for (line, system) in systems.iter_mut().enumerate() {
            let (dx, dy) = moved(line);
            for y in [
                &mut system.top,
                &mut system.y,
                &mut system.bottom,
                &mut system.cursor_top,
                &mut system.cursor_bottom,
            ] {
                *y = r(*y + dy);
            }
            for row in &mut system.rows {
                row.top = r(row.top + dy);
                row.bottom = r(row.bottom + dy);
                row.y = r(row.y + dy);
                for x in row.beats.iter_mut().flatten() {
                    *x = r(*x + dx);
                }
            }
        }
        let mut bars = self.bars.clone();
        for bar in &mut bars {
            let (dx, _) = moved(bar.system);
            bar.x = r(bar.x + dx);
            for x in bar.beats.iter_mut().chain(bar.slot.as_mut()) {
                *x = r(*x + dx);
            }
        }
        let sheets: Vec<Sheet> = (0..count)
            .map(|number| {
                let (x, y) = offset(number);
                Sheet { x, y, w: page.width, h: page.height }
            })
            .collect();
        let (right, bottom) = offset(count - 1);
        Layout {
            width: r(right + page.width + PAGE_GAP),
            height: r(bottom + page.height + PAGE_GAP),
            items: pages.into_iter().flatten().collect(),
            systems,
            bars,
            sheets,
            ..self.clone()
        }
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

    #[test]
    fn pages_on_the_screen_are_set_out_down_or_across_with_their_lines_and_bars() {
        let whole = layout(&long(120), &View::default(), Page::A4.width);
        let printed = whole.pages(Page::A4);
        for set in [Pages::Down, Pages::Across] {
            let sheets = whole.sheets(Page::A4, set);
            assert_eq!(sheets.sheets.len(), printed.len());
            // As many items as the printed pages, each on its sheet.
            assert_eq!(sheets.items.len(), printed.iter().map(|p| p.items.len()).sum::<usize>());
            let on_a_sheet = |x: f64, y: f64| {
                (sheets.sheets.iter())
                    .any(|s| x >= s.x && x <= s.x + s.w && y >= s.y && y <= s.y + s.h)
            };
            for item in &sheets.items {
                if let Item::Text { x, y, .. } = item {
                    assert!(on_a_sheet(*x, *y), "{set:?}: {item:?}");
                }
            }
            // Every bar is on the sheet its line is on, inside it, with its beats.
            assert_eq!(sheets.bars.len(), whole.bars.len());
            for bar in &sheets.bars {
                let system = &sheets.systems[bar.system];
                assert!(on_a_sheet(bar.x, system.y) && on_a_sheet(bar.x + bar.w, system.bottom));
                assert!(bar.beats.iter().all(|&x| x >= bar.x && x <= bar.x + bar.w));
            }
            let (last, first) = (sheets.sheets.last().unwrap(), sheets.sheets[0]);
            match set {
                // With room around them all.
                Pages::Down => {
                    assert_eq!(last.x, first.x);
                    assert_eq!(sheets.width, first.x * 2.0 + Page::A4.width);
                    assert!(last.y > first.y + Page::A4.height);
                    assert_eq!(sheets.height, last.y + Page::A4.height + first.y);
                }
                Pages::Across => {
                    assert_eq!(last.y, first.y);
                    assert_eq!(sheets.height, first.y * 2.0 + Page::A4.height);
                    assert!(last.x > first.x + Page::A4.width);
                    assert_eq!(sheets.width, last.x + Page::A4.width + first.x);
                }
            }
        }
    }

    #[test]
    fn pages_on_the_screen_keep_the_marks_for_whoever_is_writing() {
        let mut score = long(40);
        score.tracks[0].staves[0].bars[30].voices[0].beats.truncate(1);
        let whole = layout(&score, &View::default(), Page::A4.width);
        let marked = (whole.sheets(Page::A4, Pages::Down).items.iter())
            .any(|item| matches!(item, Item::Rect { c: Color::Error, .. }));
        assert!(marked);
    }
}
