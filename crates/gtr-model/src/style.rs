//! How a score is set out: the size of its music, its spacing, the sizes of its heading,
//! the conventions it is written in, and how many bars go on a line.

use serde::{Deserialize, Serialize};

/// The stylesheet of a score. Every field has the value the score is set out with when the
/// stylesheet is left as it comes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(default)]
pub struct Style {
    /// The size of the music on the page, as a percentage: the staves, the symbols and the
    /// room between them all grow or shrink together.
    pub size: u16,
    /// How far apart the notes are along a bar, as a percentage of their usual room.
    pub spacing: u16,
    /// Room between one line of music and the next, in pixels at the usual size.
    pub line_gap: u16,
    /// Room between the staves of two tracks on a line.
    pub track_gap: u16,
    /// Room between a track's notation and its tab.
    pub staff_gap: u16,
    /// Sizes of the title and the subtitle, in points.
    pub title_size: u16,
    pub subtitle_size: u16,
    /// Which bars have their number written over them.
    pub bar_numbers: BarNumbers,
    /// Where the stems and beams of the rhythm go under the tab.
    pub tab_rhythm: TabRhythm,
    /// The tempo at the start of the score is written over its first bar.
    pub tempo: bool,
    /// The heading lists each track's tuning and capo, for those not in the usual tuning.
    pub tuning: bool,
    /// Bars on each line, unless a bar starts a new line before then; `None` for as many as
    /// fit.
    pub bars_per_line: Option<u8>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum BarNumbers {
    #[default]
    Every,
    /// Only the first bar of each line.
    Lines,
    None,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum TabRhythm {
    /// Under the tab even when the notation over it has the rhythm already.
    Always,
    /// Only when the tab is shown without the notation, which has the rhythm already.
    #[default]
    Alone,
    Never,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            size: 100,
            spacing: 100,
            line_gap: 18,
            track_gap: 16,
            staff_gap: 12,
            title_size: 24,
            subtitle_size: 15,
            bar_numbers: BarNumbers::Every,
            tab_rhythm: TabRhythm::Alone,
            tempo: true,
            tuning: true,
            bars_per_line: None,
        }
    }
}

impl Style {
    pub const SIZES: std::ops::RangeInclusive<u16> = 50..=200;
    pub const SPACINGS: std::ops::RangeInclusive<u16> = 50..=300;
    pub const GAPS: std::ops::RangeInclusive<u16> = 0..=120;
    pub const TEXT_SIZES: std::ops::RangeInclusive<u16> = 6..=72;

    pub fn is_default(&self) -> bool {
        *self == Style::default()
    }

    pub fn is_valid(&self) -> bool {
        Style::SIZES.contains(&self.size)
            && Style::SPACINGS.contains(&self.spacing)
            && [self.line_gap, self.track_gap, self.staff_gap]
                .iter()
                .all(|gap| Style::GAPS.contains(gap))
            && [self.title_size, self.subtitle_size]
                .iter()
                .all(|size| Style::TEXT_SIZES.contains(size))
            && self.bars_per_line != Some(0)
    }
}
