use gtr_model::{
    Accent, Beat, BendKind, Dynamic, Hairpin, Note, NoteValue, PickStroke, Rational, Score, Slap,
    Slide, Staff, Strum, TimeSignature, Vibrato, interval_label,
};

use crate::display::r;
use crate::{Align, BarBox, BarStatus, Color, Font, Item, Layout, SystemBox};

const MARGIN_X: f64 = 36.0;
const HEADER_HEIGHT: f64 = 96.0;
const STRING_SPACING: f64 = 13.0;
/// Space above the staff for bar numbers and the tempo mark, before any effect lanes.
const ABOVE_STAFF: f64 = 30.0;
/// Space below the staff for stems, beams and tuplet numbers.
const BELOW_STAFF: f64 = 64.0;
/// Extra space below that for dynamics and hairpins, on lines that have them.
const DYNAMICS_HEIGHT: f64 = 22.0;
const SYSTEM_GAP: f64 = 18.0;
const CLEF_WIDTH: f64 = 28.0;

const BAR_PAD_LEFT: f64 = 12.0;
const BAR_PAD_RIGHT: f64 = 4.0;
const TIME_SIG_DIGIT_WIDTH: f64 = 24.0;
/// Distance from the start of a beat's space to the centre of its fret numbers.
const BEAT_INSET: f64 = 9.0;
const SLOT_WIDTH: f64 = 26.0;
const MIN_BAR_WIDTH: f64 = 76.0;

const STEM_GAP: f64 = 10.0;
const STEM_LENGTH: f64 = 26.0;
const BEAM_THICKNESS: f64 = 3.0;
const BEAM_PITCH: f64 = 5.0;
const FRET_SIZE: f64 = 12.0;
const REST_SIZE: f64 = 30.0;
const LABEL_SIZE: f64 = 9.0;

/// SMuFL code points.
const TIME_SIG_0: u32 = 0xE080;
const REST_WHOLE: u32 = 0xE4E3;
const DOWN_BOW: char = '\u{E610}';
const UP_BOW: char = '\u{E612}';
/// Dynamic marks from ppp to fff, in the order of [`Dynamic`].
const DYNAMICS: [char; 8] = [
    '\u{E52A}', '\u{E52B}', '\u{E520}', '\u{E52C}', '\u{E52D}', '\u{E522}', '\u{E52F}', '\u{E530}',
];

/// A horizontal band above the staff that holds one kind of marking. A system only makes
/// room for the lanes its music uses.
#[derive(Clone, Copy)]
enum Lane {
    Bend,
    Vibrato,
    PalmMute,
    LetRing,
    Harmonic,
    Tap,
    Accent,
    Stroke,
    Trill,
    Whammy,
}

/// Lanes from the staff upwards, with their heights.
const LANES: [(Lane, f64); 10] = [
    (Lane::Bend, 30.0),
    (Lane::Vibrato, 11.0),
    (Lane::PalmMute, 12.0),
    (Lane::LetRing, 12.0),
    (Lane::Harmonic, 12.0),
    (Lane::Tap, 12.0),
    (Lane::Accent, 12.0),
    (Lane::Stroke, 15.0),
    (Lane::Trill, 12.0),
    (Lane::Whammy, 26.0),
];

/// Distance from the top string up to the first lane, clear of the bar numbers.
const LANE_BASE: f64 = 17.0;

/// The lanes one system uses and where they sit.
#[derive(Default)]
struct Lanes {
    used: [bool; LANES.len()],
    /// Vertical centre of each used lane, as a distance above the top string.
    centre: [f64; LANES.len()],
    height: f64,
    /// The line has dynamics or hairpins, which go below the staff.
    dynamics: bool,
}

impl Lanes {
    fn for_beats<'a>(beats: impl Iterator<Item = &'a Beat>) -> Lanes {
        let mut lanes = Lanes::default();
        for beat in beats {
            lanes.dynamics |= beat.effects.dynamic.is_some() || beat.effects.hairpin.is_some();
            let mut mark = |lane: Lane, used: bool| lanes.used[lane as usize] |= used;
            mark(Lane::Stroke, beat.effects.stroke.is_some());
            mark(Lane::Whammy, beat.effects.whammy.is_some());
            let hand = beat.effects.slap.is_some() || beat.effects.strum == Some(Strum::Rasgueado);
            mark(Lane::Tap, hand);
            for note in &beat.notes {
                let e = &note.effects;
                mark(Lane::Bend, e.bend.is_some());
                mark(Lane::Vibrato, e.vibrato.is_some());
                mark(Lane::PalmMute, e.palm_mute);
                mark(Lane::LetRing, e.let_ring);
                mark(Lane::Harmonic, e.harmonic.is_some());
                mark(Lane::Tap, e.tapped);
                mark(Lane::Trill, e.trill.is_some());
                mark(Lane::Accent, e.accent.is_some() || e.staccato);
            }
        }
        let mut offset = LANE_BASE;
        for (i, (_, height)) in LANES.iter().enumerate() {
            if lanes.used[i] {
                lanes.centre[i] = offset + height / 2.0;
                offset += height;
                lanes.height += height;
            }
        }
        lanes
    }

    /// Vertical centre of a lane for a staff whose top string is at `staff_y`.
    fn y(&self, lane: Lane, staff_y: f64) -> f64 {
        staff_y - self.centre[lane as usize]
    }
}

/// A bar measured at its natural width, with x positions relative to the bar's left edge.
struct Measured {
    /// Width of the part that does not stretch when the system is justified.
    fixed: f64,
    width: f64,
    beats: Vec<f64>,
    slot: Option<f64>,
    status: BarStatus,
    time_signature: Option<TimeSignature>,
}

/// Lays out the first staff and voice of `track` as tablature, wrapped to `width` pixels.
pub fn layout_track(score: &Score, track: usize, width: f64) -> Layout {
    let staff = &score.tracks[track.min(score.tracks.len() - 1)].staves[0];
    let string_count = staff.tuning.strings.len();
    let staff_height = (string_count - 1) as f64 * STRING_SPACING;
    let width = width.max(2.0 * MARGIN_X + CLEF_WIDTH + MIN_BAR_WIDTH);
    let available = width - 2.0 * MARGIN_X - CLEF_WIDTH;
    let voice = |bar: usize| staff.bars[bar].voices[0].beats.as_slice();

    let measured: Vec<Measured> = (score.master_bars.iter().enumerate())
        .map(|(i, master)| {
            let shown = (i == 0
                || score.master_bars[i - 1].time_signature != master.time_signature)
                .then_some(master.time_signature);
            measure(voice(i), master.time_signature, shown)
        })
        .collect();

    let mut items = vec![];
    header(score, width, &mut items);

    let mut systems = Vec::new();
    let mut bars: Vec<BarBox> = Vec::with_capacity(measured.len());
    let mut top = HEADER_HEIGHT;
    let mut first = 0;
    while first < measured.len() {
        // Take as many bars as fit, but always at least one.
        let mut end = first + 1;
        let mut natural = measured[first].width;
        while end < measured.len() && natural + measured[end].width <= available {
            natural += measured[end].width;
            end += 1;
        }
        let fixed: f64 = measured[first..end].iter().map(|m| m.fixed).sum();
        let mut stretch = (available - fixed) / (natural - fixed).max(1.0);
        if end == measured.len() {
            // Leave a short final line ragged rather than stretching it across the page.
            stretch = stretch.min(1.0);
        }

        let lanes = Lanes::for_beats((first..end).flat_map(voice));
        let y = top + ABOVE_STAFF + lanes.height;
        let below = BELOW_STAFF + if lanes.dynamics { DYNAMICS_HEIGHT } else { 0.0 };
        let bottom = y + staff_height + below;
        let system = systems.len();
        systems.push(SystemBox {
            top: r(top),
            y: r(y),
            bottom: r(bottom),
        });

        let mut x = MARGIN_X + CLEF_WIDTH;
        for m in &measured[first..end] {
            let place = |rel: f64| r(x + m.fixed + (rel - m.fixed) * stretch);
            let w = m.fixed + (m.width - m.fixed) * stretch;
            bars.push(BarBox {
                system,
                x: r(x),
                w: r(w),
                beats: m.beats.iter().map(|&b| place(b)).collect(),
                slot: m.slot.map(place),
                status: m.status,
            });
            x += w;
        }
        let context = System {
            score,
            staff,
            bars: &bars,
            first,
            y,
            staff_height,
            lanes: &lanes,
        };
        for (offset, measured_bar) in measured[first..end].iter().enumerate() {
            context.draw_bar(first + offset, measured_bar, &mut items);
        }
        context.draw_hairpins(end, &mut items);

        draw_staff(
            MARGIN_X,
            x,
            y,
            string_count,
            end == measured.len(),
            &mut items,
        );
        if first == 0 {
            items.push(Item::Text {
                x: MARGIN_X,
                y: r(top + 6.0),
                s: format!("\u{2669} = {}", score.tempo),
                size: 13.0,
                font: Font::Text,
                align: Align::Left,
                knockout: false,
                c: Color::Ink,
            });
        }
        top = bottom + SYSTEM_GAP;
        first = end;
    }

    Layout {
        width: r(width),
        height: r(top + 24.0),
        string_spacing: STRING_SPACING,
        string_count,
        items,
        systems,
        bars,
    }
}

fn header(score: &Score, width: f64, items: &mut Vec<Item>) {
    let title = match score.info.title.as_str() {
        "" => "Untitled",
        title => title,
    };
    items.push(Item::text(width / 2.0, 38.0, title, 24.0, Font::Text));
    let credits: Vec<&str> = [score.info.artist.as_str(), score.info.album.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    if !credits.is_empty() {
        let credit = Item::text(
            width / 2.0,
            64.0,
            credits.join(" \u{2013} "),
            13.0,
            Font::Text,
        );
        items.push(credit.colored(Color::Muted));
    }
}

/// Horizontal space a beat takes at its natural width. Longer notes get more room, but less
/// than proportionally, and markings that sit beside a note get room of their own.
fn spacing(beat: &Beat) -> f64 {
    let quarters = beat.duration.length() * 4;
    let quarters = *quarters.numer() as f64 / *quarters.denom() as f64;
    let wide = beat.notes.iter().any(|n| {
        let e = &n.effects;
        e.bend.is_some()
            || e.harmonic.is_some()
            || e.slide.is_some()
            || e.hammer
            || e.trill.is_some()
    });
    let width = (28.0 * quarters.powf(0.55)).max(if wide { 30.0 } else { 20.0 });
    // Grace notes and strum arrows sit to the left of the beat.
    let grace = beat.notes.iter().any(|n| n.effects.grace.is_some());
    let strum = matches!(beat.effects.strum, Some(s) if s != Strum::Rasgueado);
    width + if grace { 12.0 } else { 0.0 } + if strum { 6.0 } else { 0.0 }
}

fn measure(
    beats: &[Beat],
    time_signature: TimeSignature,
    shown: Option<TimeSignature>,
) -> Measured {
    let mut x = BAR_PAD_LEFT;
    if let Some(ts) = shown {
        let digits = ts.numerator.max(ts.denominator).to_string().len();
        x += digits as f64 * TIME_SIG_DIGIT_WIDTH + 6.0;
    }
    let fixed = x;

    let mut positions = Vec::with_capacity(beats.len());
    let mut length = Rational::from_integer(0);
    for beat in beats {
        let grace = beat.notes.iter().any(|n| n.effects.grace.is_some());
        let strum = matches!(beat.effects.strum, Some(s) if s != Strum::Rasgueado);
        let lead = if grace { 12.0 } else { 0.0 } + if strum { 6.0 } else { 0.0 };
        positions.push(x + BEAT_INSET + lead);
        x += spacing(beat);
        length += beat.duration.length();
    }
    let status = match length.cmp(&time_signature.bar_length()) {
        _ if beats.is_empty() => BarStatus::Empty,
        std::cmp::Ordering::Less => BarStatus::Incomplete,
        std::cmp::Ordering::Equal => BarStatus::Complete,
        std::cmp::Ordering::Greater => BarStatus::Overfull,
    };
    let slot = matches!(status, BarStatus::Empty | BarStatus::Incomplete).then(|| {
        let slot = x + BEAT_INSET;
        x += SLOT_WIDTH;
        slot
    });
    Measured {
        fixed,
        width: (x + BAR_PAD_RIGHT).max(MIN_BAR_WIDTH),
        beats: positions,
        slot,
        status,
        time_signature: shown,
    }
}

fn draw_staff(x1: f64, x2: f64, y: f64, strings: usize, last: bool, items: &mut Vec<Item>) {
    let bottom = y + (strings - 1) as f64 * STRING_SPACING;
    // String lines go first so that fret numbers can knock them out.
    for s in 0..strings {
        let line_y = y + s as f64 * STRING_SPACING;
        items.insert(0, Item::line(x1, line_y, x2, line_y, 1.0, Color::Muted));
    }
    items.push(Item::line(x1, y, x1, bottom, 1.0, Color::Ink));
    if last {
        items.push(Item::line(x2 - 5.0, y, x2 - 5.0, bottom, 1.0, Color::Ink));
        items.push(Item::rect(x2 - 3.0, y, 3.0, bottom - y, Color::Ink));
    }
    let middle = (y + bottom) / 2.0;
    for (i, letter) in ["T", "A", "B"].into_iter().enumerate() {
        let letter_y = middle + (i as f64 - 1.0) * 13.0;
        items.push(Item::text(x1 + 13.0, letter_y, letter, 13.0, Font::Text).knockout());
    }
}

/// The text written on the string for a note.
fn fret_label(note: &Note) -> String {
    let e = &note.effects;
    if e.dead {
        "X".into()
    } else if e.harmonic == Some(gtr_model::Harmonic::Natural) {
        format!("<{}>", note.fret)
    } else if note.tied || e.ghost {
        format!("({})", note.fret)
    } else {
        note.fret.to_string()
    }
}

/// Half the width of a fret label, for starting lines clear of it.
fn half_width(note: &Note) -> f64 {
    fret_label(note).chars().count() as f64 * 3.4 + 1.5
}

/// One line of music being drawn.
struct System<'a> {
    score: &'a Score,
    staff: &'a Staff,
    /// Boxes of every bar laid out so far, up to the end of this system.
    bars: &'a [BarBox],
    /// Index of the system's first bar.
    first: usize,
    /// Y of the top string.
    y: f64,
    staff_height: f64,
    lanes: &'a Lanes,
}

impl System<'_> {
    fn beats(&self, bar: usize) -> &[Beat] {
        &self.staff.bars[bar].voices[0].beats
    }

    /// The beat after `beat`, with its x position if it is on this system.
    fn following(&self, bar: usize, beat: usize) -> Option<(&Beat, Option<f64>)> {
        if let Some(next) = self.beats(bar).get(beat + 1) {
            return Some((next, Some(self.bars[bar].beats[beat + 1])));
        }
        let next = self.staff.bars.get(bar + 1)?.voices[0].beats.first()?;
        let x = self.bars.get(bar + 1).map(|b| b.beats[0]);
        Some((next, x))
    }

    fn draw_bar(&self, index: usize, measured: &Measured, items: &mut Vec<Item>) {
        let beats = self.beats(index);
        let bar = &self.bars[index];
        let (y, staff_height) = (self.y, self.staff_height);
        let bottom = y + staff_height;
        let invalid = matches!(bar.status, BarStatus::Incomplete | BarStatus::Overfull);
        let right = bar.x + bar.w;

        items.push(Item::line(right, y, right, bottom, 1.0, Color::Ink));
        items.push(Item::Text {
            x: r(bar.x + 3.0),
            y: r(y - 9.0),
            s: (index + 1).to_string(),
            size: 10.0,
            font: Font::Text,
            align: Align::Left,
            knockout: false,
            c: if invalid { Color::Error } else { Color::Muted },
        });
        if invalid {
            let mark_y = bottom + BELOW_STAFF - 12.0;
            items.push(Item::rect(
                bar.x + 4.0,
                mark_y,
                bar.w - 8.0,
                2.0,
                Color::Error,
            ));
        }

        if let Some(ts) = measured.time_signature {
            let digits = ts.numerator.max(ts.denominator).to_string().len();
            let x = bar.x + BAR_PAD_LEFT + digits as f64 * TIME_SIG_DIGIT_WIDTH / 2.0;
            let size = (staff_height * 0.8).clamp(28.0, 60.0);
            for (number, at) in [(ts.numerator, 0.25), (ts.denominator, 0.75)] {
                let glyphs: String = number
                    .to_string()
                    .bytes()
                    .filter_map(|d| char::from_u32(TIME_SIG_0 + u32::from(d - b'0')))
                    .collect();
                items.push(Item::text(
                    x,
                    y + staff_height * at,
                    glyphs,
                    size,
                    Font::Music,
                ));
            }
        }

        for (i, beat) in beats.iter().enumerate() {
            let x = bar.beats[i];
            if beat.is_rest() {
                let glyph = char::from_u32(REST_WHOLE + beat.duration.value as u32).unwrap_or(' ');
                let middle = y + staff_height / 2.0;
                items.push(Item::text(x, middle, glyph, REST_SIZE, Font::Music).knockout());
                for dot in 0..beat.duration.dots.min(2) {
                    items.push(Item::Dot {
                        x: r(x + 9.0 + 4.0 * f64::from(dot)),
                        y: r(middle - 3.0),
                        r: 1.4,
                    });
                }
            }
            for note in &beat.notes {
                self.draw_note(index, i, note, items);
            }
            self.draw_beat_marks(index, i, items);
        }
        self.draw_spans(
            index,
            Lane::PalmMute,
            "P.M.",
            |n| n.effects.palm_mute,
            items,
        );
        self.draw_spans(
            index,
            Lane::LetRing,
            "let ring",
            |n| n.effects.let_ring,
            items,
        );

        draw_rhythm(
            beats,
            &bar.beats,
            self.score.master_bars[index].time_signature,
            bottom,
            items,
        );
    }

    /// Draws a note's fret number and the markings tied to its string: ties, slurs, slides
    /// and bends.
    fn draw_note(&self, bar_index: usize, beat: usize, note: &Note, items: &mut Vec<Item>) {
        let bar = &self.bars[bar_index];
        let x = bar.beats[beat];
        let y = self.y + f64::from(note.string) * STRING_SPACING;
        let half = half_width(note);
        let effects = &note.effects;
        items.push(Item::text(x, y, fret_label(note), FRET_SIZE, Font::Fret).knockout());

        if note.tied {
            // Tie back to the previous beat, or to the start of the line.
            let from = match beat.checked_sub(1) {
                Some(previous) => bar.beats[previous],
                None if bar_index > self.first => {
                    let previous = &self.bars[bar_index - 1];
                    previous.beats.last().copied().unwrap_or(bar.x)
                }
                None => bar.x - 6.0,
            };
            items.push(Item::Arc {
                x1: r(from + 6.0),
                x2: r(x - 9.0),
                y: r(y - 7.0),
                h: 5.0,
            });
        }

        // Where a slur or slide into the next note ends: at that note, at the end of the
        // line if the note is on the next one, or a short way off if there is none.
        let following = self.following(bar_index, beat);
        let target = following.and_then(|(next, _)| next.note_on(note.string));
        let join_end = match (target, following) {
            (Some(target), Some((_, Some(next_x)))) => next_x - half_width(target) - 2.0,
            (Some(_), _) => bar.x + bar.w - 3.0,
            _ => x + half + 16.0,
        };
        let rising = target.map_or(0.0, |t| f64::from(t.fret.cmp(&note.fret) as i8));

        if effects.hammer || effects.slide == Some(Slide::Legato) {
            let (from, to) = (x + 2.0, join_end + 4.0);
            items.push(Item::Arc {
                x1: r(from),
                x2: r(to),
                y: r(y - 8.0),
                h: 4.0,
            });
            if effects.hammer {
                let letter = if rising < 0.0 { "P" } else { "H" };
                let label = Item::text((from + to) / 2.0, y - 14.0, letter, 8.0, Font::Text);
                items.push(label.knockout());
            }
        }
        match effects.slide {
            Some(Slide::Shift | Slide::Legato) => {
                let (from, to) = (x + half + 2.0, join_end);
                items.push(Item::line(
                    from,
                    y + 3.0 * rising,
                    to,
                    y - 3.0 * rising,
                    1.1,
                    Color::Ink,
                ));
            }
            Some(Slide::InFromBelow) => {
                items.push(Item::line(
                    x - half - 9.0,
                    y + 4.0,
                    x - half - 1.0,
                    y,
                    1.1,
                    Color::Ink,
                ));
            }
            Some(Slide::InFromAbove) => {
                items.push(Item::line(
                    x - half - 9.0,
                    y - 4.0,
                    x - half - 1.0,
                    y,
                    1.1,
                    Color::Ink,
                ));
            }
            Some(Slide::OutDown) => {
                items.push(Item::line(
                    x + half + 1.0,
                    y,
                    x + half + 9.0,
                    y + 4.0,
                    1.1,
                    Color::Ink,
                ));
            }
            Some(Slide::OutUp) => {
                items.push(Item::line(
                    x + half + 1.0,
                    y,
                    x + half + 9.0,
                    y - 4.0,
                    1.1,
                    Color::Ink,
                ));
            }
            None => {}
        }

        if let Some(grace) = effects.grace {
            let grace_x = x - half - 8.0;
            let label = Item::text(grace_x, y, grace.fret.to_string(), 8.5, Font::Fret);
            items.push(label.knockout());
            if grace.hammer {
                items.push(Item::Arc {
                    x1: r(grace_x),
                    x2: r(x - 1.0),
                    y: r(y - 7.0),
                    h: 3.0,
                });
            }
        }
        // Fingering: fretting hand below left of the number, picking hand below right.
        if let Some(finger) = effects.left_finger {
            let label = Item::text(
                x - half - 3.0,
                y + 6.0,
                finger.left_label(),
                7.0,
                Font::Text,
            );
            items.push(label.colored(Color::Muted));
        }
        if let Some(finger) = effects.right_finger {
            let label = Item::text(
                x + half + 3.0,
                y + 6.0,
                finger.right_label(),
                7.0,
                Font::Text,
            );
            items.push(label.colored(Color::Muted));
        }

        if let Some(bend) = &effects.bend {
            let lane = self.lanes.y(Lane::Bend, self.y);
            let tip = lane - 4.0;
            let arrow = |items: &mut Vec<Item>, x: f64, y: f64, up: f64| {
                items.push(Item::line(x, y, x - 3.0, y + 5.0 * up, 1.1, Color::Ink));
                items.push(Item::line(x, y, x + 3.0, y + 5.0 * up, 1.1, Color::Ink));
            };
            // Drawn as the nearest common shape, labelled with the curve's highest point.
            let kind = bend.kind();
            let prebend = matches!(kind, BendKind::Prebend | BendKind::PrebendRelease);
            let release = matches!(kind, BendKind::BendRelease | BendKind::PrebendRelease);
            // The arrow rises from the note to the bend lane; a pre-bend goes straight up.
            let peak = if prebend { x } else { x + half + 9.0 };
            if prebend {
                items.push(Item::line(x, y - 8.0, x, tip, 1.1, Color::Ink));
            } else {
                items.push(Item::curve(x + half + 1.0, y, peak, y, peak, tip));
            }
            arrow(items, peak, tip, 1.0);
            if release {
                let end = (peak + 13.0, y - 5.0);
                items.push(Item::curve(peak, tip, end.0, tip, end.0, end.1));
                arrow(items, end.0, end.1, -1.0);
            }
            let label = interval_label(bend.peak());
            items.push(Item::text(peak, tip - 7.0, label, LABEL_SIZE, Font::Text));
        }
    }

    /// Draws the markings that sit in lanes above a beat.
    fn draw_beat_marks(&self, bar_index: usize, beat_index: usize, items: &mut Vec<Item>) {
        let beat = &self.beats(bar_index)[beat_index];
        let bar = &self.bars[bar_index];
        let x = bar.beats[beat_index];
        let lane = |lane: Lane| self.lanes.y(lane, self.y);
        let any = |test: fn(&Note) -> bool| beat.notes.iter().any(test);

        let vibrato = beat
            .notes
            .iter()
            .filter_map(|n| n.effects.vibrato)
            .max_by_key(|v| *v as u8);
        if let Some(vibrato) = vibrato {
            // A wavy line that runs towards the next beat.
            let end = match self.following(bar_index, beat_index) {
                Some((_, Some(next_x))) if next_x > x => (next_x - 7.0).min(x + 60.0),
                _ => (bar.x + bar.w - 4.0).min(x + 28.0),
            };
            let (amplitude, weight) = match vibrato {
                Vibrato::Slight => (2.0, 1.1),
                Vibrato::Wide => (3.5, 1.6),
            };
            let y = lane(Lane::Vibrato);
            let mut from = (x - 4.0, y + amplitude);
            let mut up = true;
            while from.0 < end.max(x + 8.0) {
                let to = (from.0 + 3.0, if up { y - amplitude } else { y + amplitude });
                items.push(Item::line(from.0, from.1, to.0, to.1, weight, Color::Ink));
                from = to;
                up = !up;
            }
        }
        if let Some(harmonic) = beat.notes.iter().find_map(|n| n.effects.harmonic) {
            let y = lane(Lane::Harmonic);
            items.push(Item::text(x, y, harmonic.label(), LABEL_SIZE, Font::Text));
        }
        let hand = match (beat.effects.slap, beat.effects.strum) {
            (Some(Slap::Slap), _) => Some("S"),
            (Some(Slap::Pop), _) => Some("P"),
            (_, Some(Strum::Rasgueado)) => Some("rasg."),
            _ if any(|n| n.effects.tapped) => Some("T"),
            _ => None,
        };
        if let Some(hand) = hand {
            items.push(Item::text(x, lane(Lane::Tap), hand, 10.0, Font::Text));
        }
        if let Some(trill) = beat.notes.iter().find_map(|n| n.effects.trill) {
            let label = format!("tr({})", trill.fret);
            items.push(Item::text(
                x + 4.0,
                lane(Lane::Trill),
                label,
                LABEL_SIZE,
                Font::Text,
            ));
        }
        if let Some(whammy) = &beat.effects.whammy {
            // The curve itself, scaled to the lane, with its furthest point labelled.
            let centre = lane(Lane::Whammy) + 2.0;
            let reach = f64::from(whammy.peak().unsigned_abs().max(1));
            let width = match self.following(bar_index, beat_index) {
                Some((_, Some(next_x))) if next_x > x => (next_x - x - 8.0).clamp(14.0, 44.0),
                _ => 24.0,
            };
            let point = |p: &gtr_model::CurvePoint| {
                let along = f64::from(p.position) / f64::from(gtr_model::Bend::LENGTH);
                (x + along * width, centre - f64::from(p.value) / reach * 7.0)
            };
            for pair in whammy.points.windows(2) {
                let (from, to) = (point(&pair[0]), point(&pair[1]));
                items.push(Item::line(from.0, from.1, to.0, to.1, 1.2, Color::Ink));
            }
            let peak = (whammy.points.iter()).max_by_key(|p| p.value.unsigned_abs());
            if let Some(peak) = peak {
                let (peak_x, peak_y) = point(peak);
                let label_y = peak_y + if peak.value < 0 { 7.0 } else { -7.0 };
                let label = interval_label(peak.value);
                items.push(Item::text(peak_x, label_y, label, 8.0, Font::Text));
            }
        }
        if let Some(strum) = beat.effects.strum.filter(|s| *s != Strum::Rasgueado)
            && let (Some(top), Some(low)) = (beat.notes.first(), beat.notes.last())
        {
            // An arrow beside the chord. A downstroke runs from the low strings to the
            // high ones, which is up the page.
            let arrow_x = x - 13.0;
            let top_y = self.y + f64::from(top.string) * STRING_SPACING - 6.0;
            let low_y = self.y + f64::from(low.string) * STRING_SPACING + 6.0;
            let down = matches!(strum, Strum::BrushDown | Strum::ArpeggioDown);
            if matches!(strum, Strum::ArpeggioDown | Strum::ArpeggioUp) {
                let mut y = top_y;
                let mut left = true;
                while y < low_y {
                    let next = (y + 3.0).min(low_y);
                    let (x1, x2) = if left { (-1.5, 1.5) } else { (1.5, -1.5) };
                    items.push(Item::line(
                        arrow_x + x1,
                        y,
                        arrow_x + x2,
                        next,
                        1.0,
                        Color::Ink,
                    ));
                    y = next;
                    left = !left;
                }
            } else {
                items.push(Item::line(arrow_x, top_y, arrow_x, low_y, 1.1, Color::Ink));
            }
            let (tip, back) = if down { (top_y, 4.0) } else { (low_y, -4.0) };
            items.push(Item::line(
                arrow_x,
                tip,
                arrow_x - 2.5,
                tip + back,
                1.1,
                Color::Ink,
            ));
            items.push(Item::line(
                arrow_x,
                tip,
                arrow_x + 2.5,
                tip + back,
                1.1,
                Color::Ink,
            ));
        }
        if let Some(dynamic) = beat.effects.dynamic {
            let y = self.y + self.staff_height + BELOW_STAFF + 9.0;
            let glyph = DYNAMICS[dynamic as usize];
            items.push(Item::text(x, y, glyph, 22.0, Font::Music));
        }
        let y = lane(Lane::Accent);
        match beat
            .notes
            .iter()
            .filter_map(|n| n.effects.accent)
            .max_by_key(|a| *a as u8)
        {
            Some(Accent::Normal) => items.push(Item::text(x, y, ">", 12.0, Font::Fret)),
            Some(Accent::Heavy) => items.push(Item::text(x, y, "\u{2227}", 12.0, Font::Fret)),
            None => {}
        }
        if any(|n| n.effects.staccato) {
            let beside = if any(|n| n.effects.accent.is_some()) {
                8.0
            } else {
                0.0
            };
            items.push(Item::Dot {
                x: r(x + beside),
                y: r(y + 1.0),
                r: 1.6,
            });
        }
        if let Some(stroke) = beat.effects.stroke {
            let glyph = match stroke {
                PickStroke::Down => DOWN_BOW,
                PickStroke::Up => UP_BOW,
            };
            items.push(Item::text(
                x,
                lane(Lane::Stroke) + 5.0,
                glyph,
                20.0,
                Font::Music,
            ));
        }
    }

    /// Draws a wedge under each run of beats that share a hairpin, up to `end`, the bar
    /// after the last one on this line.
    fn draw_hairpins(&self, end: usize, items: &mut Vec<Item>) {
        let y = self.y + self.staff_height + BELOW_STAFF + 5.0;
        let beats: Vec<(f64, Option<Hairpin>, Option<Dynamic>)> = (self.first..end)
            .flat_map(|bar| {
                let xs = &self.bars[bar].beats;
                (self.beats(bar).iter().zip(xs))
                    .map(|(b, &x)| (x, b.effects.hairpin, b.effects.dynamic))
            })
            .collect();
        let mut start = 0;
        while start < beats.len() {
            let Some(hairpin) = beats[start].1 else {
                start += 1;
                continue;
            };
            let mut stop = start + 1;
            while stop < beats.len() && beats[stop].1 == Some(hairpin) {
                stop += 1;
            }
            // Leave room for a dynamic mark at either end.
            let from = beats[start].0 + if beats[start].2.is_some() { 12.0 } else { -4.0 };
            let to = match beats.get(stop) {
                Some(next) => next.0 - if next.2.is_some() { 12.0 } else { 6.0 },
                None => beats[stop - 1].0 + 14.0,
            }
            .max(from + 10.0);
            let (open_x, point_x) = match hairpin {
                Hairpin::Crescendo => (to, from),
                Hairpin::Decrescendo => (from, to),
            };
            items.push(Item::line(point_x, y, open_x, y - 4.0, 1.0, Color::Ink));
            items.push(Item::line(point_x, y, open_x, y + 4.0, 1.0, Color::Ink));
            start = stop;
        }
    }

    /// Draws a label with a dashed line over each run of beats in a bar that share a
    /// marking, such as palm muting. A run carried over from the previous bar on the same
    /// line continues the dashes without repeating the label.
    fn draw_spans(
        &self,
        bar_index: usize,
        lane: Lane,
        label: &str,
        marked: fn(&Note) -> bool,
        items: &mut Vec<Item>,
    ) {
        let beats = self.beats(bar_index);
        let bar = &self.bars[bar_index];
        let has = |beat: &Beat| beat.notes.iter().any(marked);
        let y = self.lanes.y(lane, self.y);
        let label_width = label.chars().count() as f64 * 4.6 + 4.0;

        let mut start = 0;
        while start < beats.len() {
            if !has(&beats[start]) {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            while end < beats.len() && has(&beats[end]) {
                end += 1;
            }
            let continued = start == 0
                && bar_index > self.first
                && self.beats(bar_index - 1).last().is_some_and(has);
            let continues = end == beats.len()
                && self.bars.len() > bar_index + 1
                && self.beats(bar_index + 1).first().is_some_and(has);

            let first_x = bar.beats[start];
            let mut dash = if continued {
                bar.x
            } else {
                items.push(Item::Text {
                    x: r(first_x - 5.0),
                    y: r(y),
                    s: label.into(),
                    size: LABEL_SIZE,
                    font: Font::Text,
                    align: Align::Left,
                    knockout: false,
                    c: Color::Ink,
                });
                first_x - 5.0 + label_width
            };
            let stop = if continues {
                bar.x + bar.w
            } else {
                bar.beats[end - 1] + 6.0
            };
            while dash + 4.0 <= stop {
                items.push(Item::line(dash, y, dash + 4.0, y, 1.0, Color::Ink));
                dash += 7.0;
            }
            if !continues && stop > first_x - 5.0 + label_width {
                items.push(Item::line(stop, y - 3.0, stop, y + 3.0, 1.0, Color::Ink));
            }
            start = end;
        }
    }
}

/// Draws stems, beams, flags, dots and tuplet numbers below the staff.
fn draw_rhythm(
    beats: &[Beat],
    xs: &[f64],
    time_signature: TimeSignature,
    staff_bottom: f64,
    items: &mut Vec<Item>,
) {
    let stem_top = staff_bottom + STEM_GAP;
    let stem_bottom = stem_top + STEM_LENGTH;
    let flags = |beat: &Beat| match beat.is_rest() {
        true => 0,
        false => beat.duration.value.flags(),
    };

    // Beams join notes that start within the same pulse: a dotted quarter in compound
    // time, a quarter otherwise.
    let compound = time_signature.denominator == 8 && time_signature.numerator.is_multiple_of(3);
    let pulse = if compound {
        Rational::new(3, 8)
    } else {
        Rational::new(1, 4)
    };
    let mut pulses = Vec::with_capacity(beats.len());
    let mut position = Rational::from_integer(0);
    for beat in beats {
        pulses.push((position / pulse).floor().to_integer());
        position += beat.duration.length();
    }

    for (i, beat) in beats.iter().enumerate() {
        let x = xs[i];
        if !beat.is_rest() {
            match beat.duration.value {
                NoteValue::Whole => {}
                NoteValue::Half => {
                    let top = stem_bottom - STEM_LENGTH / 2.0;
                    items.push(Item::line(x, top, x, stem_bottom, 1.2, Color::Ink));
                }
                _ => items.push(Item::line(x, stem_top, x, stem_bottom, 1.2, Color::Ink)),
            }
            // Tremolo picking: one slash across the stem per beam of the picked value.
            let slashes = beat.effects.tremolo.map_or(0, |v| v.flags());
            for slash in 0..slashes {
                let y = stem_top + 6.0 + 4.0 * f64::from(slash);
                items.push(Item::line(
                    x - 4.0,
                    y + 2.0,
                    x + 4.0,
                    y - 2.0,
                    1.8,
                    Color::Ink,
                ));
            }
        }
        // A rest carries its dots beside the rest symbol instead.
        let dots = match beat.is_rest() {
            true => 0,
            false => beat.duration.dots.min(2),
        };
        for dot in 0..dots {
            items.push(Item::Dot {
                x: r(x + 5.0 + 4.0 * f64::from(dot)),
                y: r(stem_top + 5.0),
                r: 1.4,
            });
        }
    }

    // Split the bar into runs of beamable notes within one pulse.
    let mut start = 0;
    while start < beats.len() {
        let mut end = start + 1;
        if flags(&beats[start]) > 0 {
            while end < beats.len() && flags(&beats[end]) > 0 && pulses[end] == pulses[start] {
                end += 1;
            }
            draw_beams(&beats[start..end], &xs[start..end], stem_bottom, items);
        }
        start = end;
    }

    // One number under each tuplet: a run of up to as many notes as the tuplet holds. A
    // tuplet nested inside another gets its number on a second row.
    let rows = [
        (
            stem_bottom + 10.0,
            (|b: &Beat| b.duration.tuplet) as fn(&Beat) -> _,
        ),
        (stem_bottom + 20.0, |b: &Beat| b.duration.nested),
    ];
    for (y, tuplet_of) in rows {
        let mut start = 0;
        while start < beats.len() {
            let mut end = start + 1;
            if let Some(tuplet) = tuplet_of(&beats[start]) {
                let size = usize::from(tuplet.actual.get());
                while end < beats.len()
                    && end - start < size
                    && tuplet_of(&beats[end]) == Some(tuplet)
                {
                    end += 1;
                }
                let x = (xs[start] + xs[end - 1]) / 2.0;
                let label = tuplet.actual.to_string();
                items.push(Item::text(x, y, label, 10.0, Font::Text));
            }
            start = end;
        }
    }
}

/// Beams a run of flagged notes, or draws flags if the run is a single note.
fn draw_beams(beats: &[Beat], xs: &[f64], stem_bottom: f64, items: &mut Vec<Item>) {
    let level_y = |level: u32| stem_bottom - BEAM_THICKNESS - f64::from(level - 1) * BEAM_PITCH;
    if let [beat] = beats {
        for level in 1..=beat.duration.value.flags() {
            let y = level_y(level) + BEAM_THICKNESS;
            items.push(Item::line(xs[0], y, xs[0] + 7.0, y - 8.0, 1.6, Color::Ink));
        }
        return;
    }
    let deepest = beats.iter().map(|b| b.duration.value.flags()).max();
    for level in 1..=deepest.unwrap_or(0) {
        let reaches = |i: usize| beats[i].duration.value.flags() >= level;
        let mut start = 0;
        while start < beats.len() {
            if !reaches(start) {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            while end < beats.len() && reaches(end) {
                end += 1;
            }
            let (x, w) = match end - start {
                // A lone short note gets a stub pointing into the group.
                1 if start == 0 => (xs[start], 7.0),
                1 => (xs[start] - 7.0, 7.0),
                _ => (xs[start], xs[end - 1] - xs[start]),
            };
            items.push(Item::rect(
                x - 0.6,
                level_y(level),
                w + 1.2,
                BEAM_THICKNESS,
                Color::Ink,
            ));
            start = end;
        }
    }
}

#[cfg(test)]
mod tests {
    use gtr_model::{Bar, Bend, Finger, Grace, Harmonic, MasterBar, Trill, demo_score};

    use super::*;

    fn texts(layout: &Layout) -> Vec<&str> {
        (layout.items.iter())
            .filter_map(|item| match item {
                Item::Text { s, .. } => Some(s.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn demo_fits_on_one_system() {
        let score = demo_score();
        let layout = layout_track(&score, 0, 1000.0);
        assert_eq!(layout.systems.len(), 1);
        assert_eq!(layout.bars.len(), 2);
        assert_eq!(layout.bars[0].beats.len(), 8);
        assert!(layout.bars.iter().all(|b| b.status == BarStatus::Complete));
        assert!(layout.bars.iter().all(|b| b.slot.is_none()));
        // Beats run left to right inside their bar.
        for bar in &layout.bars {
            assert!(bar.beats.windows(2).all(|w| w[0] < w[1]));
            assert!(bar.beats.iter().all(|&x| x > bar.x && x < bar.x + bar.w));
        }
    }

    #[test]
    fn bars_wrap_and_fill_the_width() {
        let mut score = demo_score();
        for _ in 0..10 {
            score.master_bars.push(MasterBar {
                time_signature: TimeSignature::COMMON,
            });
            let bar = score.tracks[0].staves[0].bars[0].clone();
            score.tracks[0].staves[0].bars.push(bar);
        }
        let width = 900.0;
        let layout = layout_track(&score, 0, width);
        assert!(layout.systems.len() > 1);
        // Every system but the last is justified to the right margin.
        let last_system = layout.systems.len() - 1;
        for (i, bar) in layout.bars.iter().enumerate() {
            let ends_system = layout
                .bars
                .get(i + 1)
                .is_none_or(|n| n.system != bar.system);
            if ends_system && bar.system != last_system {
                assert!((bar.x + bar.w - (width - MARGIN_X)).abs() < 0.5);
            }
        }
        assert!(layout.height > layout.systems[last_system].bottom);
    }

    #[test]
    fn empty_and_incomplete_bars_have_a_slot() {
        let mut score = demo_score();
        score.tracks[0].staves[0].bars[0].voices[0].beats.pop();
        score.master_bars.push(MasterBar {
            time_signature: TimeSignature::COMMON,
        });
        score.tracks[0].staves[0].bars.push(Bar::empty());
        let layout = layout_track(&score, 0, 1000.0);
        assert_eq!(layout.bars[0].status, BarStatus::Incomplete);
        assert!(layout.bars[0].slot.is_some());
        assert_eq!(layout.bars[2].status, BarStatus::Empty);
        assert!(layout.bars[2].slot.is_some());
    }

    #[test]
    fn techniques_are_labelled_and_make_room_above_the_staff() {
        let plain = layout_track(&demo_score(), 0, 1000.0);

        let mut score = demo_score();
        let beats = &mut score.tracks[0].staves[0].bars[0].voices[0].beats;
        beats[0].notes[0].effects.palm_mute = true;
        beats[1].notes[0].effects.palm_mute = true;
        beats[2].notes[0].effects.bend = Some(Bend::preset(BendKind::BendRelease, 6));
        beats[3].notes[0].effects.dead = true;
        beats[4].notes[0].effects.hammer = true;
        beats[5].notes[0].effects.harmonic = Some(Harmonic::Pinch);
        beats[6].notes[0].effects.ghost = true;
        beats[7].notes[0].effects.let_ring = true;
        let layout = layout_track(&score, 0, 1000.0);

        let found = texts(&layout);
        for label in ["P.M.", "1½", "X", "H", "P.H.", "(2)", "let ring"] {
            assert!(found.contains(&label), "missing {label:?} in {found:?}");
        }
        // One label covers both palm-muted beats.
        assert_eq!(found.iter().filter(|t| **t == "P.M.").count(), 1);
        // Bend, palm mute, let ring and harmonic lanes push the staff down.
        assert_eq!(layout.systems[0].y, plain.systems[0].y + 30.0 + 12.0 * 3.0);
        assert!(layout.items.iter().any(|i| matches!(i, Item::Curve { .. })));
    }

    #[test]
    fn beat_markings_are_drawn_above_and_below() {
        let plain = layout_track(&demo_score(), 0, 1000.0);

        let mut score = demo_score();
        let beats = &mut score.tracks[0].staves[0].bars[0].voices[0].beats;
        beats[0].effects.dynamic = Some(Dynamic::Ff);
        beats[1].effects.hairpin = Some(Hairpin::Decrescendo);
        beats[2].effects.hairpin = Some(Hairpin::Decrescendo);
        beats[3].effects.whammy = Some(Bend::dive(6));
        beats[4].effects.slap = Some(Slap::Pop);
        beats[5].effects.strum = Some(Strum::Rasgueado);
        beats[6].notes[0].effects.trill = Some(Trill {
            fret: 4,
            speed: NoteValue::Sixteenth,
        });
        beats[7].notes[0].effects.grace = Some(Grace {
            fret: 9,
            on_beat: false,
            hammer: true,
        });
        beats[7].notes[0].effects.left_finger = Some(Finger::Ring);
        beats[7].notes[0].effects.right_finger = Some(Finger::Middle);
        let layout = layout_track(&score, 0, 1000.0);

        let found = texts(&layout);
        let fortissimo = DYNAMICS[Dynamic::Ff as usize].to_string();
        for label in [
            fortissimo.as_str(),
            "-1½",
            "P",
            "rasg.",
            "tr(4)",
            "9",
            "3",
            "m",
        ] {
            assert!(found.contains(&label), "missing {label:?} in {found:?}");
        }
        // Whammy, trill and hand lanes above; a row for dynamics below.
        assert_eq!(layout.systems[0].y, plain.systems[0].y + 26.0 + 12.0 + 12.0);
        let height = |l: &Layout| l.systems[0].bottom - l.systems[0].y;
        assert_eq!(height(&layout), height(&plain) + DYNAMICS_HEIGHT);
    }
}
