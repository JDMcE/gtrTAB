use serde::Serialize;

/// A laid-out score. Coordinates are in pixels at 100% zoom, origin top left.
#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct Layout {
    pub width: f64,
    pub height: f64,
    /// Vertical distance between adjacent string lines.
    pub string_spacing: f64,
    /// Strings of the staff being edited, or if it has none the number of pitches there are.
    pub string_count: usize,
    /// Whether the staff being edited is shown as tab, which is where the cursor picks
    /// out a string.
    pub tab: bool,
    /// Set when the staff being edited has no strings and is shown as notation, where
    /// the cursor picks out a pitch: the note on the staff's middle line, counted in
    /// steps of the scale from C in the octave below MIDI's lowest.
    pub middle: Option<i32>,
    /// Vertical distance between one step of the scale and the next on a notation staff.
    pub step: f64,
    pub items: Vec<Item>,
    pub systems: Vec<SystemBox>,
    /// One entry per bar of the score.
    pub bars: Vec<BarBox>,
}

/// One line of music.
#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct SystemBox {
    /// Top of the system including the space reserved above the staff.
    pub top: f64,
    /// Y of the first (highest) string line of the staff being edited, or of the top line
    /// of its notation when no tab is shown, or of the middle line if it has no strings.
    pub y: f64,
    pub bottom: f64,
    /// Top and bottom staff lines of the track being edited, across all its staves.
    pub cursor_top: f64,
    pub cursor_bottom: f64,
    /// The tracks shown on this line, from the top.
    pub rows: Vec<RowBox>,
}

/// The space one track takes on a line.
#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct RowBox {
    pub track: usize,
    /// Which of the track's staves this is, counted from the top.
    pub staff: usize,
    pub top: f64,
    pub bottom: f64,
    /// Y of the track's first string line, or of the top line of its notation, or of the
    /// middle line of a staff without strings.
    pub y: f64,
    /// Number of string lines shown; zero without a tab staff.
    pub strings: usize,
    /// The note on the middle line of a staff without strings; see [`Layout::middle`].
    pub middle: Option<i32>,
    /// X centre of each beat this staff has in the voice being edited, for every bar of
    /// the line, so that a click on a staff other than the cursor's can find its beat.
    pub beats: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct BarBox {
    pub system: usize,
    pub x: f64,
    pub w: f64,
    /// X centre of each beat.
    pub beats: Vec<f64>,
    /// X centre of the empty slot after the last beat, if the bar has room for more.
    pub slot: Option<f64>,
    pub status: BarStatus,
}

/// How a bar's content compares with its time signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum BarStatus {
    Empty,
    Incomplete,
    Complete,
    Overfull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Color {
    Ink,
    Muted,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Font {
    /// Titles and labels.
    Text,
    /// Fret numbers.
    Fret,
    /// SMuFL music symbols, drawn with the glyph origin at `y`.
    Music,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Item {
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        w: f64,
        c: Color,
    },
    Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        c: Color,
    },
    Dot {
        x: f64,
        y: f64,
        r: f64,
    },
    /// A beam: a band `h` thick with upright ends, whose top edge runs from `(x1, y1)` to
    /// `(x2, y2)`.
    Beam {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        h: f64,
    },
    /// A tie: a curve from `(x1, y)` to `(x2, y)` bulging upwards by `h`.
    Arc {
        x1: f64,
        x2: f64,
        y: f64,
        h: f64,
    },
    /// A quadratic curve from `(x1, y1)` to `(x2, y2)`, pulled towards `(cx, cy)`.
    Curve {
        x1: f64,
        y1: f64,
        cx: f64,
        cy: f64,
        x2: f64,
        y2: f64,
    },
    Text {
        x: f64,
        /// Vertical centre of the text, or the glyph origin for [`Font::Music`].
        y: f64,
        s: String,
        size: f64,
        font: Font,
        align: Align,
        /// Erase the background behind the text, as for fret numbers on a string line.
        knockout: bool,
        c: Color,
    },
}

/// Rounds to a tenth of a pixel to keep the serialised display list small.
pub(crate) fn r(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

impl Item {
    pub(crate) fn line(x1: f64, y1: f64, x2: f64, y2: f64, w: f64, c: Color) -> Item {
        Item::Line {
            x1: r(x1),
            y1: r(y1),
            x2: r(x2),
            y2: r(y2),
            w,
            c,
        }
    }

    pub(crate) fn rect(x: f64, y: f64, w: f64, h: f64, c: Color) -> Item {
        Item::Rect {
            x: r(x),
            y: r(y),
            w: r(w),
            h: r(h),
            c,
        }
    }

    pub(crate) fn beam(x1: f64, y1: f64, x2: f64, y2: f64, h: f64) -> Item {
        Item::Beam {
            x1: r(x1),
            y1: r(y1),
            x2: r(x2),
            y2: r(y2),
            h,
        }
    }

    pub(crate) fn curve(x1: f64, y1: f64, cx: f64, cy: f64, x2: f64, y2: f64) -> Item {
        Item::Curve {
            x1: r(x1),
            y1: r(y1),
            cx: r(cx),
            cy: r(cy),
            x2: r(x2),
            y2: r(y2),
        }
    }

    /// How far down the page the item starts.
    pub fn top(&self) -> f64 {
        match self {
            Item::Line { y1, y2, .. } | Item::Beam { y1, y2, .. } | Item::Curve { y1, y2, .. } => {
                y1.min(*y2)
            }
            Item::Rect { y, .. }
            | Item::Dot { y, .. }
            | Item::Arc { y, .. }
            | Item::Text { y, .. } => *y,
        }
    }

    /// Moves the item down the page.
    pub(crate) fn shift_y(&mut self, by: f64) {
        match self {
            Item::Line { y1, y2, .. } | Item::Beam { y1, y2, .. } => {
                *y1 = r(*y1 + by);
                *y2 = r(*y2 + by);
            }
            Item::Curve { y1, cy, y2, .. } => {
                *y1 = r(*y1 + by);
                *cy = r(*cy + by);
                *y2 = r(*y2 + by);
            }
            Item::Rect { y, .. }
            | Item::Dot { y, .. }
            | Item::Arc { y, .. }
            | Item::Text { y, .. } => *y = r(*y + by),
        }
    }

    /// Makes a text item erase what is behind it.
    pub(crate) fn knockout(mut self) -> Item {
        if let Item::Text { knockout, .. } = &mut self {
            *knockout = true;
        }
        self
    }

    pub(crate) fn colored(mut self, color: Color) -> Item {
        if let Item::Text { c, .. } | Item::Line { c, .. } | Item::Rect { c, .. } = &mut self {
            *c = color;
        }
        self
    }

    pub(crate) fn text(x: f64, y: f64, s: impl Into<String>, size: f64, font: Font) -> Item {
        Item::Text {
            x: r(x),
            y: r(y),
            s: s.into(),
            size,
            font,
            align: Align::Center,
            knockout: false,
            c: Color::Ink,
        }
    }
}
