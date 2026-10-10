use gtr_model::{Beat, NoteValue, Rational, Score, TimeSignature};

use crate::display::r;
use crate::{Align, BarBox, BarStatus, Color, Font, Item, Layout, SystemBox};

const MARGIN_X: f64 = 36.0;
const HEADER_HEIGHT: f64 = 96.0;
const STRING_SPACING: f64 = 13.0;
/// Space above the staff for bar numbers and the tempo mark.
const ABOVE_STAFF: f64 = 30.0;
/// Space below the staff for stems, beams and tuplet numbers.
const BELOW_STAFF: f64 = 64.0;
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

/// SMuFL code points.
const TIME_SIG_0: u32 = 0xE080;
const REST_WHOLE: u32 = 0xE4E3;

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

    let measured: Vec<Measured> = (score.master_bars.iter().enumerate())
        .map(|(i, master)| {
            let shown = (i == 0
                || score.master_bars[i - 1].time_signature != master.time_signature)
                .then_some(master.time_signature);
            measure(&staff.bars[i].voices[0].beats, master.time_signature, shown)
        })
        .collect();

    let mut items = vec![];
    header(score, width, &mut items);

    let mut systems = Vec::new();
    let mut bars = Vec::with_capacity(measured.len());
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

        let y = top + ABOVE_STAFF;
        let bottom = y + staff_height + BELOW_STAFF;
        let system = systems.len();
        systems.push(SystemBox {
            top: r(top),
            y: r(y),
            bottom: r(bottom),
        });

        let mut x = MARGIN_X + CLEF_WIDTH;
        let system_start = bars.len();
        for (i, m) in measured[first..end].iter().enumerate() {
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
            draw_bar(
                score,
                track,
                first + i,
                m,
                &bars,
                system_start,
                y,
                staff_height,
                &mut items,
            );
            x += w;
        }

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
        let mut credit = Item::text(
            width / 2.0,
            64.0,
            credits.join(" \u{2013} "),
            13.0,
            Font::Text,
        );
        if let Item::Text { c, .. } = &mut credit {
            *c = Color::Muted;
        }
        items.push(credit);
    }
}

/// Horizontal space a beat takes at its natural width. Longer notes get more room, but less
/// than proportionally.
fn spacing(beat: &Beat) -> f64 {
    let quarters = beat.duration.length() * 4;
    let quarters = *quarters.numer() as f64 / *quarters.denom() as f64;
    (28.0 * quarters.powf(0.55)).max(20.0)
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
        positions.push(x + BEAT_INSET);
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
        let mut item = Item::text(x1 + 13.0, letter_y, letter, 13.0, Font::Text);
        if let Item::Text { knockout, .. } = &mut item {
            *knockout = true;
        }
        items.push(item);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_bar(
    score: &Score,
    track: usize,
    index: usize,
    measured: &Measured,
    bars: &[BarBox],
    system_start: usize,
    y: f64,
    staff_height: f64,
    items: &mut Vec<Item>,
) {
    let staff = &score.tracks[track].staves[0];
    let beats = &staff.bars[index].voices[0].beats;
    let bar = &bars[index];
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
            let mut rest = Item::text(x, y + staff_height / 2.0, glyph, REST_SIZE, Font::Music);
            if let Item::Text { knockout, .. } = &mut rest {
                *knockout = true;
            }
            items.push(rest);
            for dot in 0..beat.duration.dots.min(2) {
                items.push(Item::Dot {
                    x: r(x + 9.0 + 4.0 * f64::from(dot)),
                    y: r(y + staff_height / 2.0 - 3.0),
                    r: 1.4,
                });
            }
        }
        for note in &beat.notes {
            let note_y = y + f64::from(note.string) * STRING_SPACING;
            let label = match note.tied {
                true => format!("({})", note.fret),
                false => note.fret.to_string(),
            };
            let mut text = Item::text(x, note_y, label, FRET_SIZE, Font::Fret);
            if let Item::Text { knockout, .. } = &mut text {
                *knockout = true;
            }
            items.push(text);
            if note.tied {
                // Tie back to the previous beat, or to the start of the line.
                let from = match i.checked_sub(1) {
                    Some(previous) => bar.beats[previous],
                    None if index > system_start => {
                        bars[index - 1].beats.last().copied().unwrap_or(bar.x)
                    }
                    None => bar.x - 6.0,
                };
                items.push(Item::Arc {
                    x1: r(from + 6.0),
                    x2: r(x - 9.0),
                    y: r(note_y - 7.0),
                    h: 5.0,
                });
            }
        }
    }

    draw_rhythm(
        beats,
        &bar.beats,
        score.master_bars[index].time_signature,
        bottom,
        items,
    );
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

    // One number under each run of tuplet notes within a pulse.
    let mut start = 0;
    while start < beats.len() {
        let mut end = start + 1;
        if let Some(tuplet) = beats[start].duration.tuplet {
            while end < beats.len()
                && beats[end].duration.tuplet == Some(tuplet)
                && pulses[end] == pulses[start]
            {
                end += 1;
            }
            let x = (xs[start] + xs[end - 1]) / 2.0;
            let label = tuplet.actual.to_string();
            items.push(Item::text(x, stem_bottom + 10.0, label, 10.0, Font::Text));
        }
        start = end;
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
    use gtr_model::{Bar, MasterBar, demo_score};

    use super::*;

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
}
