//! Puts a score on the page: breaks it into lines, stacks the staves of each line and has
//! each staff drawn.

use gtr_model::{Bar, Clef, MasterBar, Rational, Score, Staff, Target, TempoPoint};
use serde::Deserialize;

use crate::display::r;
use crate::notation::{self, Notation as NotationStaff, SPACE, SpelledBar};
use crate::spacing::{self, BAR_PAD_LEFT, MIN_BAR_WIDTH, Measured, REPEAT_WIDTH, Row};
use crate::tab::{
    self, ABOVE_STAFF, BELOW_STAFF, DYNAMICS_HEIGHT, LYRIC_LINE, Lanes, STRING_SPACING, System,
};
use crate::{Align, BarBox, BarStatus, Color, Font, Item, Layout, RowBox, SystemBox};

pub(crate) const MARGIN_X: f64 = 36.0;
const HEADER_HEIGHT: f64 = 96.0;
const SYSTEM_GAP: f64 = 18.0;
/// Space between the staves of two tracks on the same line.
const TRACK_GAP: f64 = 16.0;
/// Space between a track's notation and its tab.
const STAFF_GAP: f64 = 12.0;
/// Room at the start of a line for the letters TAB.
const TAB_PREFIX: f64 = 28.0;
/// Room at the start of a line for a clef.
const CLEF_PREFIX: f64 = 36.0;
/// Height of the band holding a track's name, in the first line of a multitrack view.
const NAME_BAND: f64 = 14.0;
/// How far a clef reaches above what a staff otherwise needs over it.
const CLEF_RISE: f64 = 12.0;
/// Height of the band above a line for section names, tempo changes and jumps.
const MARK_BAND: f64 = 17.0;
/// Height of the band for the brackets over alternate endings.
const ENDING_BAND: f64 = 15.0;
const SEGNO: char = '\u{E047}';
const CODA: char = '\u{E048}';

/// Which staves a track is shown on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Notation {
    Tab,
    Standard,
    #[default]
    Both,
    /// Rhythm slashes on a single staff.
    Slash,
}

/// What to lay out and how.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct View {
    /// The track being edited, which is the only one shown unless `multitrack` is set.
    pub track: usize,
    /// The staff of that track being edited: a piano has two.
    pub staff: usize,
    /// The voice being edited.
    pub voice: usize,
    pub notation: Notation,
    /// Show every track, one above the other.
    pub multitrack: bool,
    /// Every bar on one line, each as wide as it needs, for scrolling along sideways. The
    /// heading is laid out to the width given, and the line runs on past it.
    pub single_line: bool,
}

/// One staff of a track with what its notation needs.
struct Part<'a> {
    track: usize,
    /// Which of the track's staves this is, and whether it is the last.
    index: usize,
    last: bool,
    name: &'a str,
    staff: &'a Staff,
    /// The track's other sounds, which a beat can switch to.
    sounds: &'a [gtr_model::Sound],
    clef: Clef,
    /// Whether the staff is shown as standard notation, and whether as tab. A staff
    /// without strings has no tab to show, and gets notation in its place.
    standard: bool,
    tab: bool,
    /// A drum staff's notation with the feet apart from the hands, if any bar needs it.
    split: Option<notation::Split>,
    spelled: Option<Vec<SpelledBar>>,
    leads: Option<Vec<Vec<Vec<f64>>>>,
}

/// Lays out the first staff of `track` as tablature alone, wrapped to `width` pixels.
pub fn layout_track(score: &Score, track: usize, width: f64) -> Layout {
    let view = View {
        track,
        notation: Notation::Tab,
        ..View::default()
    };
    layout(score, &view, width)
}

/// Lays out the score as `view` asks, wrapped to `width` pixels.
pub fn layout(score: &Score, view: &View, width: f64) -> Layout {
    let edit_track = view.track.min(score.tracks.len() - 1);
    let voice = view.voice.min(Bar::MAX_VOICES - 1);
    let shown: Vec<usize> = match view.multitrack {
        true => (0..score.tracks.len()).collect(),
        false => vec![edit_track],
    };
    let edit_staff = view.staff.min(score.tracks[edit_track].staves.len() - 1);
    let standard = matches!(view.notation, Notation::Standard | Notation::Both);
    let slash = view.notation == Notation::Slash;
    let tab = matches!(view.notation, Notation::Tab | Notation::Both);
    let masters = &score.master_bars;

    let parts: Vec<Part> = (shown.iter())
        .flat_map(|&index| (0..score.tracks[index].staves.len()).map(move |s| (index, s)))
        .map(|(index, staff_index)| {
            let track = &score.tracks[index];
            let staff = &track.staves[staff_index];
            let clef = track.instrument.clef(staff_index);
            let tab = tab && !staff.is_pitched();
            let standard = standard || (staff.is_pitched() && !slash);
            let kit = match clef {
                Clef::Percussion => track.drum_kit(),
                _ => Vec::new(),
            };
            let split = (standard && clef == Clef::Percussion)
                .then(|| notation::split_feet(staff, &kit, masters))
                .flatten();
            let spelled = standard.then(|| {
                let transposition = track.instrument.written_transposition();
                let drawn = split.as_ref().map_or(staff, |s| &s.staff);
                notation::spell(drawn, clef, transposition, masters, &kit)
            });
            let leads = (spelled.as_ref())
                .map(|bars| bars.iter().map(notation::accidental_leads).collect());
            Part {
                track: index,
                index: staff_index,
                last: staff_index + 1 == track.staves.len(),
                name: &track.name,
                staff,
                sounds: &track.sounds,
                clef,
                standard,
                tab,
                split,
                spelled,
                leads,
            }
        })
        .collect();
    let edit_row = (parts.iter())
        .position(|p| (p.track, p.index) == (edit_track, edit_staff))
        .unwrap_or(0);
    let string_count = parts[edit_row].staff.places();
    // Whether any staff is in standard notation, which decides what a line starts with.
    let standard = parts.iter().any(|p| p.standard);
    let middle_of = |part: &Part| {
        (part.standard && part.staff.is_pitched()).then(|| notation::middle_line(part.clef))
    };

    let times = gtr_model::bar_times(score);
    let measured: Vec<Measured> = (masters.iter().enumerate())
        .map(|(i, master)| {
            let rows: Vec<Row> = (parts.iter())
                .map(|part| Row {
                    bar: &part.staff.bars[i],
                    leads: part.leads.as_ref().map(|leads| leads[i].as_slice()),
                })
                .collect();
            let previous = i.checked_sub(1).map(|p| &masters[p]);
            let shown = (previous.is_none_or(|p| p.time_signature != master.time_signature))
                .then_some(master.time_signature);
            let key = (previous.filter(|_| standard))
                .is_some_and(|p| p.key_signature != master.key_signature)
                .then_some(master.key_signature);
            let repeat = (master.repeat_start, master.repeat_count >= 2);
            let mut measured = spacing::measure(
                &rows,
                (edit_row, voice),
                master.time_signature,
                shown,
                key,
                repeat,
            );
            // A bar is at least as wide as what is written over it.
            let marks = marks_width(master, i, times[i]);
            measured.width = measured.width.max(marks);
            measured
        })
        .collect();

    // What comes before the first bar of a line: a clef and the key, or the letters TAB.
    // A bar that changes the key writes the new one itself, so a line that starts with
    // one has only the clef before it.
    let prefix = |first: usize| match (standard, measured[first].key) {
        (true, None) => CLEF_PREFIX + spacing::key_width(masters[first].key_signature),
        (true, Some(_)) => CLEF_PREFIX,
        (false, _) => TAB_PREFIX,
    };
    let widest_prefix = (0..masters.len()).map(prefix).fold(TAB_PREFIX, f64::max);
    let width = width.max(2.0 * MARGIN_X + widest_prefix + MIN_BAR_WIDTH);

    let mut items = vec![];
    let header_height = header(score, width, &mut items);

    // X of every beat laid out so far, as `[row][voice][bar][beat]`.
    let mut positions: Vec<Vec<Vec<Vec<f64>>>> =
        vec![vec![Vec::new(); Bar::MAX_VOICES]; parts.len()];
    let mut systems = Vec::new();
    let mut bars: Vec<BarBox> = Vec::with_capacity(measured.len());
    let mut top = header_height;
    let mut first = 0;
    // As wide as the widest line, which only a single line can be wider than the width given.
    let mut widest = width;
    while first < measured.len() {
        let left = MARGIN_X + prefix(first);
        let available = match view.single_line {
            true => f64::INFINITY,
            false => width - MARGIN_X - left,
        };
        // Take as many bars as fit, but always at least one.
        let mut end = first + 1;
        let mut natural = measured[first].width;
        while end < measured.len() && natural + measured[end].width <= available {
            natural += measured[end].width;
            end += 1;
        }
        let fixed: f64 = measured[first..end].iter().map(|m| m.fixed).sum();
        let mut stretch = (available - fixed) / (natural - fixed).max(1.0);
        if end == measured.len() || view.single_line {
            // Leave a short final line ragged rather than stretching it across the page.
            stretch = stretch.min(1.0);
        }

        let mut x = left;
        for m in &measured[first..end] {
            let place = |rel: f64| r(x + m.fixed + (rel - m.fixed) * stretch);
            let w = m.fixed + (m.width - m.fixed) * stretch;
            for (row, voices) in positions.iter_mut().enumerate() {
                for (v, placed) in voices.iter_mut().enumerate() {
                    let beats = m.beats[row].get(v).map_or(&[][..], Vec::as_slice);
                    placed.push(beats.iter().map(|&b| place(b)).collect());
                }
            }
            bars.push(BarBox {
                system: systems.len(),
                x: r(x),
                w: r(w),
                beats: positions[edit_row][voice][bars.len()].clone(),
                slot: m.slot.map(place),
                status: m.status,
            });
            x += w;
        }
        let right = x;
        widest = widest.max(right + MARGIN_X);
        let last = end == measured.len();
        let range = first..end;

        // Stack the staves of the line, working down the page.
        // Where a moment inside a bar falls across the page: on the beat that starts
        // there in some staff shown, or failing that as far along the bar as it is through it.
        let moment_x = |bar: usize, moment: Rational| -> f64 {
            for (row, part) in parts.iter().enumerate() {
                for (v, voice) in part.staff.bars[bar].voices.iter().enumerate() {
                    let mut time = Rational::from_integer(0);
                    for (i, beat) in voice.beats.iter().enumerate() {
                        if time == moment {
                            return positions[row][v][bar][i];
                        }
                        time += beat.duration.length();
                    }
                }
            }
            let through = moment / masters[bar].time_signature.bar_length();
            let through = *through.numer() as f64 / *through.denom() as f64;
            r(bars[bar].x + bars[bar].w * through)
        };
        let mut cursor = bar_marks(
            score,
            &times,
            &bars,
            &moment_x,
            range.clone(),
            top,
            &mut items,
        );
        // Top line of the first staff of a track that has several, for the bracket.
        let mut bracket_top = None;
        let mut rows = Vec::with_capacity(parts.len());
        // Top and bottom staff lines of everything drawn so far on this line.
        let mut lines: Option<(f64, f64)> = None;
        let mut edit_lines = (top, top);
        let mut edit_y = top;
        let mut mark_y = top;
        for (row, part) in parts.iter().enumerate() {
            let row_top = cursor;
            if view.multitrack && systems.is_empty() && part.index == 0 {
                // Leave the tempo mark its place above the first name.
                // And a clef, which stands taller than its staff, room under a later one.
                let band = match (row == 0, part.standard) {
                    (true, _) => NAME_BAND + 12.0,
                    (false, true) => NAME_BAND + CLEF_RISE,
                    (false, false) => NAME_BAND,
                };
                items.push(Item::Text {
                    x: MARGIN_X,
                    y: r(cursor + band
                        - NAME_BAND / 2.0
                        - if row > 0 && part.standard {
                            CLEF_RISE
                        } else {
                            0.0
                        }),
                    s: part.name.to_owned(),
                    size: 11.0,
                    font: Font::Text,
                    align: Align::Left,
                    knockout: false,
                    c: Color::Ink,
                });
                cursor += band;
            }
            let beats = |bar: usize| part.staff.bars[bar].voices.iter().flat_map(|v| &v.beats);
            let lanes = Lanes::for_beats(range.clone().flat_map(beats));
            let dynamics = if lanes.dynamics { DYNAMICS_HEIGHT } else { 0.0 };
            // A track shown as notation over tab has its markings above the notation,
            // not between the two; only the bends, drawn from the strings, stay with
            // the tab. Shown on one staff, that staff has them all.
            let (over_notation, over_tab) = match (part.standard || slash) && part.tab {
                true => (lanes.half(false), lanes.half(true)),
                false => (lanes.clone(), lanes.clone()),
            };
            let editing = |v: usize| row != edit_row || v == voice;
            let mut row_lines: Option<(f64, f64)> = None;
            let mut row_y = None;

            if part.standard || slash {
                let mut drawn = Vec::new();
                let notation = NotationStaff {
                    staff: part.split.as_ref().map_or(part.staff, |s| &s.staff),
                    spelled: part.spelled.as_deref(),
                    xs: &positions[row],
                    bars: &bars,
                    masters,
                    first,
                    end,
                    feet: part.split.as_ref().map(|s| s.feet.as_slice()),
                    string_marks: !part.tab,
                };
                let extent = notation.draw(&mut drawn);
                notation.draw_bends(over_notation.bend_y(extent.top), &mut drawn);
                let head = if lines.is_none() { ABOVE_STAFF } else { 8.0 };
                let mid = cursor + head + over_notation.height - extent.top;
                for item in &mut drawn {
                    item.shift_y(mid);
                }
                items.append(&mut drawn);

                let (line_top, line_bottom) = (mid - 2.0 * SPACE, mid + 2.0 * SPACE);
                notation::draw_lines(MARGIN_X, right, mid, &mut items);
                if part.standard {
                    notation::draw_clef(part.clef, MARGIN_X + 5.0, mid, &mut items);
                }
                if part.standard && measured[first].key.is_none() {
                    let key = masters[first].key_signature;
                    notation::draw_key(
                        key,
                        part.clef,
                        MARGIN_X + CLEF_PREFIX - 4.0,
                        mid,
                        &mut items,
                    );
                }
                for (bar, m) in range.clone().zip(&measured[range.clone()]) {
                    let repeat = if masters[bar].repeat_start {
                        REPEAT_WIDTH
                    } else {
                        0.0
                    };
                    let mut at = bars[bar].x + BAR_PAD_LEFT - 4.0 + repeat;
                    if let Some(key) = m.key {
                        notation::draw_key(key, part.clef, at, mid, &mut items);
                        at += spacing::key_width(key);
                    }
                    if let Some(ts) = m.time_signature {
                        notation::draw_time(ts, at + 4.0, mid, &mut items);
                    }
                }
                bar_lines(
                    &bars[range.clone()],
                    &masters[range.clone()],
                    line_top,
                    line_bottom,
                    last,
                    &mut items,
                );

                let mut bottom = mid + extent.bottom + 4.0;
                // The markings above the staff, and without a tab staff under it the
                // ones below as well. There are no strings here to write the rest on.
                let music_top = mid + extent.top;
                for (v, xs) in (positions[row].iter().enumerate()).take(part.staff_voices()) {
                    let system = System {
                        staff: part.staff,
                        sounds: part.sounds,
                        voice: v,
                        xs,
                        bars: &bars,
                        first,
                        y: music_top,
                        staff_height: bottom - music_top,
                        lanes: &over_notation,
                        below: 2.0,
                        strings: false,
                        above: true,
                        under: !part.tab,
                        ink: Color::Ink,
                    };
                    for bar in range.clone() {
                        system.draw_voice(bar, masters[bar].time_signature, false, &mut items);
                    }
                    if !part.tab {
                        system.draw_hairpins(end, &mut items);
                    }
                }
                if !part.tab {
                    mark_y = if row == edit_row {
                        bottom + 4.0
                    } else {
                        mark_y
                    };
                    bottom += 8.0 + dynamics + lanes.lyrics as f64 * LYRIC_LINE;
                    // On a staff without strings the cursor is placed from its middle line.
                    row_y = Some(if part.staff.is_pitched() {
                        mid
                    } else {
                        line_top
                    });
                }
                row_lines = Some((line_top, line_bottom));
                cursor = bottom;
            }

            if part.tab {
                let staff_height = (part.staff.tuning.strings.len() - 1) as f64 * STRING_SPACING;
                let head = match (lines.is_none() && row_lines.is_none(), row_lines) {
                    (true, _) => ABOVE_STAFF,
                    (false, Some(_)) => STAFF_GAP,
                    (false, None) => 8.0,
                };
                // Under a notation staff the tab leaves the rhythm to the notation.
                let rhythm = !part.standard && !slash;
                let below = if rhythm { BELOW_STAFF } else { 12.0 };
                let y = cursor + head + over_tab.height;
                for (v, xs) in (positions[row].iter().enumerate()).take(part.staff_voices()) {
                    let system = System {
                        staff: part.staff,
                        sounds: part.sounds,
                        voice: v,
                        xs,
                        bars: &bars,
                        first,
                        y,
                        staff_height,
                        lanes: &over_tab,
                        below,
                        strings: true,
                        above: row_lines.is_none(),
                        under: true,
                        ink: if editing(v) { Color::Ink } else { Color::Muted },
                    };
                    // One voice's rhythm fits under the staff: the one being edited.
                    let shows_rhythm = rhythm && if row == edit_row { v == voice } else { v == 0 };
                    for bar in range.clone() {
                        system.draw_voice(
                            bar,
                            masters[bar].time_signature,
                            shows_rhythm,
                            &mut items,
                        );
                    }
                    system.draw_hairpins(end, &mut items);
                }
                let strings = part.staff.tuning.strings.len();
                tab::draw_staff(MARGIN_X, right, y, strings, &mut items);
                if rhythm {
                    for (bar, m) in range.clone().zip(&measured[range.clone()]) {
                        if let Some(ts) = m.time_signature {
                            tab::draw_time(ts, bars[bar].x, y, staff_height, &mut items);
                        }
                    }
                }
                bar_lines(
                    &bars[range.clone()],
                    &masters[range.clone()],
                    y,
                    y + staff_height,
                    last,
                    &mut items,
                );
                if row == edit_row {
                    mark_y = y + staff_height + below - 12.0;
                }
                row_lines = Some((row_lines.map_or(y, |l| l.0), y + staff_height));
                row_y = Some(y);
                cursor = y + staff_height + below + dynamics + lanes.lyrics as f64 * LYRIC_LINE;
            }

            let (line_top, line_bottom) = row_lines.unwrap_or((cursor, cursor));
            if lines.is_none() {
                // Bar numbers go above the first staff of the line.
                for bar in range.clone() {
                    let invalid = matches!(
                        bars[bar].status,
                        BarStatus::Incomplete | BarStatus::Overfull
                    );
                    // Clear of the sharps and flats of a key written at the start of the bar.
                    let keyed = part.standard && measured[bar].key.is_some();
                    let lift = if keyed { 17.0 } else { 9.0 };
                    items.push(Item::Text {
                        x: r(bars[bar].x + 3.0),
                        y: r(line_top - lift),
                        s: (bar + 1).to_string(),
                        size: 10.0,
                        font: Font::Text,
                        align: Align::Left,
                        knockout: false,
                        c: if invalid { Color::Error } else { Color::Muted },
                    });
                }
            }
            lines = Some((lines.map_or(line_top, |l| l.0), line_bottom));
            if row == edit_row {
                edit_lines = (line_top, line_bottom);
                edit_y = row_y.unwrap_or(line_top);
            }
            // The staves of one track are joined by a bracket down their left edge.
            if part.index == 0 && !part.last {
                bracket_top = Some(line_top);
            }
            if part.last
                && part.index > 0
                && let Some(from) = bracket_top.take()
            {
                items.push(Item::rect(
                    MARGIN_X - 6.0,
                    from,
                    3.0,
                    line_bottom - from,
                    Color::Ink,
                ));
                for (y, tip) in [(from, -4.0), (line_bottom, 4.0)] {
                    items.push(Item::line(
                        MARGIN_X - 4.5,
                        y,
                        MARGIN_X + 1.0,
                        y + tip,
                        1.4,
                        Color::Ink,
                    ));
                }
            }
            rows.push(RowBox {
                track: part.track,
                staff: part.index,
                top: r(row_top),
                bottom: r(cursor),
                y: r(row_y.unwrap_or(line_top)),
                strings: if part.tab {
                    part.staff.tuning.strings.len()
                } else {
                    0
                },
                middle: middle_of(part),
                beats: positions[row][voice][range.clone()].to_vec(),
            });
            if row + 1 < parts.len() {
                cursor += if part.last { TRACK_GAP } else { STAFF_GAP };
            }
        }

        // A bar that does not add up is underlined beneath the staff being edited.
        for bar in range.clone() {
            if matches!(
                bars[bar].status,
                BarStatus::Incomplete | BarStatus::Overfull
            ) {
                let (x, w) = (bars[bar].x + 4.0, bars[bar].w - 8.0);
                items.push(Item::rect(x, mark_y, w, 2.0, Color::Error));
            }
        }
        // One line down the left edge joins the staves that play together.
        if let Some((line_top, line_bottom)) = lines {
            items.push(Item::line(
                MARGIN_X,
                line_top,
                MARGIN_X,
                line_bottom,
                1.0,
                Color::Ink,
            ));
        }
        if first == 0 {
            items.push(Item::Text {
                x: MARGIN_X,
                y: r(top + 6.0),
                s: tempo_mark(
                    &gtr_model::tempo_points(score),
                    0,
                    Rational::from_integer(0),
                    Some(score.tempo),
                ),
                size: 13.0,
                font: Font::Text,
                align: Align::Left,
                knockout: false,
                c: Color::Ink,
            });
        }

        systems.push(SystemBox {
            top: r(top),
            y: r(edit_y),
            bottom: r(cursor),
            cursor_top: r(edit_lines.0),
            cursor_bottom: r(edit_lines.1),
            rows,
        });
        top = cursor + SYSTEM_GAP;
        first = end;
    }

    if !score.info.copyright.is_empty() {
        // The copyright line closes the score.
        let notice = Item::text(
            width / 2.0,
            top + 8.0,
            score.info.copyright.as_str(),
            10.0,
            Font::Text,
        );
        items.push(notice.colored(Color::Muted));
        top += 24.0;
    }

    Layout {
        width: r(widest),
        height: r(top + 24.0),
        string_spacing: STRING_SPACING,
        string_count,
        tab: parts[edit_row].tab,
        middle: middle_of(&parts[edit_row]),
        step: SPACE / 2.0,
        items,
        systems,
        bars,
        sheets: Vec::new(),
    }
}

impl Part<'_> {
    /// The most voices any bar of the staff has.
    fn staff_voices(&self) -> usize {
        let most = self.staff.bars.iter().map(|b| b.voices.len()).max();
        most.unwrap_or(1).min(Bar::MAX_VOICES)
    }
}

/// What is written at the right-hand end of a bar, over it: the time the piece has been
/// playing when it first gets there, how often a repeat is played, and a jump.
fn ending_label(master: &MasterBar, time: Option<f64>) -> Option<String> {
    let mut ending = Vec::new();
    if let (true, Some(seconds)) = (master.timer, time) {
        let whole = seconds.round() as u64;
        ending.push(format!("[{}:{:02}]", whole / 60, whole % 60));
    }
    if master.repeat_count > 2 {
        ending.push(format!("\u{00D7}{}", master.repeat_count));
    }
    ending.extend(master.jumps.iter().map(|jump| jump.label().to_owned()));
    (!ending.is_empty()).then(|| ending.join("  "))
}

/// Width a bar needs for what [`bar_marks`] writes over it, so that the marks of one bar
/// do not run into those of the next.
fn marks_width(master: &MasterBar, bar: usize, time: Option<f64>) -> f64 {
    let mut width = 0.0;
    width += master.targets.iter().map(|t| target_width(*t)).sum::<f64>();
    if let Some(section) = &master.section {
        width += section.chars().count() as f64 * SECTION_CHARACTER + 16.0;
    }
    if let Some(tempo) = master.tempo.filter(|_| bar > 0) {
        width += (tempo.to_string().len() + 4) as f64 * 6.5 + 8.0;
    }
    if master.tempo_gradual && bar > 0 {
        width += 40.0;
    }
    if master.feel.is_some() {
        width += 72.0;
    }
    if let Some(label) = ending_label(master, time) {
        width += label.chars().count() as f64 * ENDING_CHARACTER + 8.0;
    }
    match width > 0.0 {
        true => width + 8.0,
        false => 0.0,
    }
}

/// Room a sign takes over a bar: a word, one symbol, or two side by side.
fn target_width(target: Target) -> f64 {
    match target {
        Target::Fine => 30.0,
        Target::Segno | Target::Coda => 20.0,
        Target::SegnoSegno | Target::DoubleCoda => 32.0,
    }
}

/// How a tempo is written where the score sets it: the tempo, if it is a new one, and
/// which way it moves from there if it does so by degrees.
fn tempo_mark(points: &[TempoPoint], bar: usize, position: Rational, new: Option<u32>) -> String {
    let index = (points.iter()).position(|p| (p.bar, p.position) == (bar, position));
    let moving = match index.and_then(|index| gtr_model::gradual_change(points, index)) {
        Some(std::cmp::Ordering::Greater) => "accel.",
        Some(std::cmp::Ordering::Less) => "rit.",
        _ => "",
    };
    let tempo = new.map_or(String::new(), |tempo| format!("\u{2669} = {tempo}"));
    [tempo.as_str(), moving]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Widths of a character of a section name and of a label at the end of a bar.
const SECTION_CHARACTER: f64 = 6.4;
const ENDING_CHARACTER: f64 = 6.0;

/// Draws what applies to whole bars above a line of music: section names, tempo and feel
/// changes, segno and coda signs, jumps, repeat counts and the brackets over alternate
/// endings. Returns the y at which the staves can start.
fn bar_marks(
    score: &Score,
    times: &[Option<f64>],
    bars: &[BarBox],
    moment_x: &dyn Fn(usize, Rational) -> f64,
    range: std::ops::Range<usize>,
    top: f64,
    items: &mut Vec<Item>,
) -> f64 {
    let masters = &score.master_bars;
    let text = |x: f64, y: f64, s: String, size: f64, align: Align| Item::Text {
        x: r(x),
        y: r(y),
        s,
        size,
        font: Font::Text,
        align,
        knockout: false,
        c: Color::Ink,
    };
    let has_marks = range.clone().any(|bar| {
        let m = &masters[bar];
        let changes = m.feel.is_some() || (bar > 0 && (m.tempo.is_some() || m.tempo_gradual));
        m.section.is_some()
            || !m.tempo_changes.is_empty()
            || m.timer
            || !m.targets.is_empty()
            || !m.jumps.is_empty()
            || m.repeat_count > 2
            || changes
    });
    let points = gtr_model::tempo_points(score);
    let has_endings = range.clone().any(|bar| masters[bar].alternate_endings != 0);
    // The first line keeps its opening tempo mark above everything else.
    let mut cursor = top
        + if range.start == 0 && (has_marks || has_endings) {
            14.0
        } else {
            0.0
        };

    if has_marks {
        let y = cursor + MARK_BAND / 2.0;
        for bar in range.clone() {
            let m = &masters[bar];
            let (left, right) = (bars[bar].x + 3.0, bars[bar].x + bars[bar].w - 3.0);
            let mut x = left;
            for &target in &m.targets {
                // A second segno or coda is the sign written twice.
                let (sign, twice) = match target {
                    Target::Segno => (Some(SEGNO), false),
                    Target::SegnoSegno => (Some(SEGNO), true),
                    Target::Coda => (Some(CODA), false),
                    Target::DoubleCoda => (Some(CODA), true),
                    Target::Fine => (None, false),
                };
                match sign {
                    Some(sign) => {
                        for copy in 0..=usize::from(twice) {
                            let centre = if sign == SEGNO { 7.0 } else { 8.0 };
                            let at = x + centre + 12.0 * copy as f64;
                            items.push(Item::text(at, y + 6.0, sign, 20.0, Font::Music));
                        }
                    }
                    None => items.push(text(x, y, "Fine".into(), 12.0, Align::Left)),
                }
                x += target_width(target);
            }
            if let Some(section) = &m.section {
                // A section name stands in a box.
                let width = section.chars().count() as f64 * SECTION_CHARACTER + 8.0;
                for (x1, y1, x2, y2) in [
                    (x, y - 7.0, x + width, y - 7.0),
                    (x, y + 7.0, x + width, y + 7.0),
                    (x, y - 7.0, x, y + 7.0),
                    (x + width, y - 7.0, x + width, y + 7.0),
                ] {
                    items.push(Item::line(x1, y1, x2, y2, 1.0, Color::Ink));
                }
                let name = Item::text(x + width / 2.0, y, section.as_str(), 11.0, Font::Fret);
                items.push(name);
                x += width + 8.0;
            }
            // The tempo the piece starts in is written once, over the first line.
            let mark = match bar {
                0 => String::new(),
                _ => tempo_mark(&points, bar, Rational::from_integer(0), m.tempo),
            };
            if !mark.is_empty() {
                let width = mark.chars().count() as f64 * 6.5 + 8.0;
                items.push(text(x, y, mark, 12.0, Align::Left));
                x += width;
            }
            if let Some(feel) = m.feel {
                items.push(text(x, y, feel.label().into(), 11.0, Align::Left));
            }
            // A change of tempo inside the bar stands over the beat it starts on.
            for change in &m.tempo_changes {
                let mark = tempo_mark(&points, bar, change.position, Some(change.tempo));
                let at = moment_x(bar, change.position) - 5.0;
                items.push(text(at, y, mark, 12.0, Align::Left));
            }
            // What happens at the end of the bar is written at its right-hand end.
            if let Some(label) = ending_label(m, times[bar]) {
                let width = label.chars().count() as f64 * ENDING_CHARACTER;
                items.push(text(right - width, y, label, 11.0, Align::Left));
            }
        }
        cursor += MARK_BAND;
    }

    if has_endings {
        let y = cursor + 3.0;
        for bar in range.clone() {
            let mask = masters[bar].alternate_endings;
            if mask == 0 {
                continue;
            }
            let (left, right) = (bars[bar].x + 1.0, bars[bar].x + bars[bar].w - 2.0);
            items.push(Item::line(left, y, right, y, 1.0, Color::Ink));
            // The bracket opens where the ending starts and closes where it stops.
            let continued = bar > range.start && masters[bar - 1].alternate_endings == mask;
            if !continued {
                items.push(Item::line(
                    left,
                    y,
                    left,
                    y + ENDING_BAND - 4.0,
                    1.0,
                    Color::Ink,
                ));
                let passes: Vec<String> = (0..8)
                    .filter(|pass| mask & (1 << pass) != 0)
                    .map(|pass| format!("{}.", pass + 1))
                    .collect();
                items.push(text(
                    left + 4.0,
                    y + 7.0,
                    passes.join(" "),
                    10.0,
                    Align::Left,
                ));
            }
            if masters[bar].repeat_count >= 2 {
                items.push(Item::line(
                    right,
                    y,
                    right,
                    y + ENDING_BAND - 4.0,
                    1.0,
                    Color::Ink,
                ));
            }
        }
        cursor += ENDING_BAND;
    }
    cursor
}

/// Draws the lines between the bars of a staff: plain bar lines, repeat signs, and the
/// double line that ends the score.
fn bar_lines(
    bars: &[BarBox],
    masters: &[MasterBar],
    top: f64,
    bottom: f64,
    last: bool,
    items: &mut Vec<Item>,
) {
    let middle = (top + bottom) / 2.0;
    let gap = ((bottom - top) / 8.0).max(4.5);
    let dots = |x: f64, items: &mut Vec<Item>| {
        for y in [middle - gap, middle + gap] {
            items.push(Item::Dot {
                x: r(x),
                y: r(y),
                r: 1.7,
            });
        }
    };
    for (bar, master) in bars.iter().zip(masters) {
        let right = bar.x + bar.w;
        if master.repeat_start {
            items.push(Item::rect(bar.x, top, 3.0, bottom - top, Color::Ink));
            items.push(Item::line(
                bar.x + 5.5,
                top,
                bar.x + 5.5,
                bottom,
                1.0,
                Color::Ink,
            ));
            dots(bar.x + 9.5, items);
        }
        if master.repeat_count >= 2 {
            dots(right - 9.5, items);
            items.push(Item::line(
                right - 5.5,
                top,
                right - 5.5,
                bottom,
                1.0,
                Color::Ink,
            ));
            items.push(Item::rect(right - 3.0, top, 3.0, bottom - top, Color::Ink));
        } else {
            items.push(Item::line(right, top, right, bottom, 1.0, Color::Ink));
        }
    }
    if let (true, Some(bar)) = (
        last && masters.last().is_some_and(|m| m.repeat_count < 2),
        bars.last(),
    ) {
        let right = bar.x + bar.w;
        items.push(Item::line(
            right - 5.0,
            top,
            right - 5.0,
            bottom,
            1.0,
            Color::Ink,
        ));
        items.push(Item::rect(right - 3.0, top, 3.0, bottom - top, Color::Ink));
    }
}

/// Draws the title and credits at the top of the first page and returns the height they
/// take.
fn header(score: &Score, width: f64, items: &mut Vec<Item>) -> f64 {
    let info = &score.info;
    let title = match info.title.as_str() {
        "" => "Untitled",
        title => title,
    };
    items.push(Item::text(width / 2.0, 38.0, title, 24.0, Font::Text));
    let mut y = 64.0;
    if !info.subtitle.is_empty() {
        let subtitle = Item::text(width / 2.0, y, info.subtitle.as_str(), 15.0, Font::Text);
        items.push(subtitle);
        y += 20.0;
    }
    let credits: Vec<&str> = [info.artist.as_str(), info.album.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    if !credits.is_empty() {
        let credit = Item::text(width / 2.0, y, credits.join(" \u{2013} "), 13.0, Font::Text);
        items.push(credit.colored(Color::Muted));
        y += 20.0;
    }
    // Who wrote what: the words on the left, the music and the transcription on the right.
    let line = |x: f64, y: f64, s: String, align: Align| Item::Text {
        x: r(x),
        y: r(y),
        s,
        size: 11.0,
        font: Font::Text,
        align,
        knockout: false,
        c: Color::Ink,
    };
    let (mut left, mut right) = (y, y);
    if !info.words.is_empty() {
        items.push(line(
            MARGIN_X,
            left,
            format!("Words by {}", info.words),
            Align::Left,
        ));
        left += 15.0;
    }
    for (label, name) in [("Music by", &info.music), ("Tabbed by", &info.transcriber)] {
        if !name.is_empty() {
            items.push(line(
                width - MARGIN_X,
                right,
                format!("{label} {name}"),
                Align::Right,
            ));
            right += 15.0;
        }
    }
    (left.max(right) + 12.0).max(HEADER_HEIGHT)
}

#[cfg(test)]
mod tests {
    use gtr_model::{
        Beat, Duration, Instrument, Note, NoteValue, TimeSignature, Track, Voice, demo_score,
    };

    use super::*;

    fn texts(layout: &Layout) -> Vec<&str> {
        (layout.items.iter())
            .filter_map(|item| match item {
                Item::Text { s, .. } => Some(s.as_str()),
                _ => None,
            })
            .collect()
    }

    fn view(notation: Notation) -> View {
        View {
            notation,
            ..View::default()
        }
    }

    #[test]
    fn notation_and_tab_share_their_beat_positions() {
        let score = demo_score();
        let tab = layout(&score, &view(Notation::Tab), 1000.0);
        let both = layout(&score, &view(Notation::Both), 1000.0);
        let standard = layout(&score, &view(Notation::Standard), 1000.0);

        // A clef and noteheads appear, and the tab keeps its fret numbers.
        let found = texts(&both);
        assert!(found.contains(&"\u{E050}"), "no treble clef");
        assert_eq!(found.iter().filter(|t| **t == "\u{E0A4}").count(), 12);
        assert_eq!(found.iter().filter(|t| **t == "\u{E0A3}").count(), 3);
        assert!(found.contains(&"T") && found.contains(&"2"));
        assert!(!texts(&standard).contains(&"T"));
        assert!(!texts(&tab).contains(&"\u{E050}"));

        // The notation staff sits above the tab staff, and the cursor spans both.
        let system = &both.systems[0];
        assert!(both.tab && !standard.tab);
        assert!(system.cursor_top < system.y);
        assert_eq!(system.cursor_bottom, system.y + 5.0 * STRING_SPACING);
        assert_eq!(system.rows.len(), 1);
        assert_eq!(system.rows[0].strings, 6);
        assert_eq!(standard.systems[0].rows[0].strings, 0);
        // Low notes hang below the staff on ledger lines, so the staff takes more room.
        assert!(system.bottom - system.top > tab.systems[0].bottom - tab.systems[0].top);
        assert_eq!(both.bars.len(), 2);
        assert!(both.bars.iter().all(|b| b.status == BarStatus::Complete));
    }

    #[test]
    fn key_signatures_are_written_at_the_start_and_where_they_change() {
        let mut score = demo_score();
        let sharps = |layout: &Layout| texts(layout).iter().filter(|t| **t == "\u{E262}").count();
        let plain = layout(&score, &view(Notation::Both), 1000.0);
        assert_eq!(sharps(&plain), 0);

        // E minor: one sharp at the start of the line, and no accidentals needed.
        score.master_bars[0].key_signature = 1;
        score.master_bars[1].key_signature = 1;
        let keyed = layout(&score, &view(Notation::Both), 1000.0);
        assert_eq!(sharps(&keyed), 1);
        assert!(keyed.bars[0].x > plain.bars[0].x);

        // A change of key in the second bar is written there, and pushes its notes along.
        score.master_bars[1].key_signature = 3;
        let changed = layout(&score, &view(Notation::Both), 1000.0);
        assert_eq!(sharps(&changed), 1 + 3);
        let offset = |l: &Layout| l.bars[1].beats[0] - l.bars[1].x;
        assert!(offset(&changed) > offset(&keyed));
        // The tab-only view has no key signatures to show.
        assert_eq!(sharps(&layout(&score, &view(Notation::Tab), 1000.0)), 0);
    }

    #[test]
    fn multitrack_view_stacks_tracks_and_aligns_their_beats() {
        let mut score = demo_score();
        let mut bass = Track::empty("Bass", Instrument::Bass, 2);
        bass.staves[0].bars[0].voices[0].beats = vec![
            Beat::new(Duration::new(NoteValue::Half), vec![Note::new(3, 0)]),
            Beat::new(Duration::new(NoteValue::Half), vec![Note::new(3, 3)]),
        ];
        score.tracks.push(bass);

        let single = layout(&score, &view(Notation::Both), 1000.0);
        assert_eq!(single.systems[0].rows.len(), 1);
        assert!(!texts(&single).contains(&"Bass"));

        let multi = View {
            multitrack: true,
            ..view(Notation::Both)
        };
        let guitar = layout(&score, &multi, 1000.0);
        let rows = &guitar.systems[0].rows;
        assert_eq!(rows.iter().map(|r| r.track).collect::<Vec<_>>(), [0, 1]);
        assert!(rows[0].bottom <= rows[1].top);
        assert_eq!(rows[1].strings, 4);
        let found = texts(&guitar);
        assert!(found.contains(&"Guitar") && found.contains(&"Bass"));
        assert!(found.contains(&"\u{E062}"), "no bass clef");

        // Editing the bass: its two half notes sit under the guitar's first and fifth
        // eighth notes.
        let bass_view = View { track: 1, ..multi };
        let bass = layout(&score, &bass_view, 1000.0);
        assert_eq!(bass.string_count, 4);
        assert_eq!(
            bass.bars[0].beats,
            [guitar.bars[0].beats[0], guitar.bars[0].beats[4]]
        );
        // Each row knows where its own beats are, whichever is being edited.
        assert_eq!(rows[1].beats, [bass.bars[0].beats.clone(), vec![]]);
        assert_eq!(bass.systems[0].rows[0].beats[1], guitar.bars[1].beats);
        // Its empty second bar offers a slot where the guitar's first beat is.
        assert_eq!(bass.bars[1].status, BarStatus::Empty);
        assert_eq!(bass.bars[1].slot, Some(guitar.bars[1].beats[0]));
        // The cursor belongs to the lower row.
        assert_eq!(bass.systems[0].y, rows[1].y);
    }

    #[test]
    fn voices_share_a_staff_and_the_edited_one_sets_the_bar_status() {
        let mut score = demo_score();
        score.tracks[0].staves[0].bars[0].voices.push(Voice {
            beats: vec![Beat::new(
                Duration::new(NoteValue::Half),
                vec![Note::new(0, 3)],
            )],
        });
        let first = layout(&score, &view(Notation::Both), 1000.0);
        let second = layout(
            &score,
            &View {
                voice: 1,
                ..view(Notation::Both)
            },
            1000.0,
        );
        assert_eq!(first.bars[0].status, BarStatus::Complete);
        assert_eq!(first.bars[0].beats.len(), 8);
        // The second voice holds half a bar: one beat, then a slot where the first
        // voice's fifth note is.
        assert_eq!(second.bars[0].status, BarStatus::Incomplete);
        assert_eq!(second.bars[0].beats, [first.bars[0].beats[0]]);
        assert_eq!(second.bars[0].slot, Some(first.bars[0].beats[4]));
        // A bar without the voice is simply empty.
        assert_eq!(second.bars[1].status, BarStatus::Empty);
        // Fret numbers of the voice not being edited are drawn muted.
        let muted = |layout: &Layout| {
            (layout.items.iter())
                .filter(|i| {
                    matches!(
                        i,
                        Item::Text {
                            font: Font::Fret,
                            c: Color::Muted,
                            ..
                        }
                    )
                })
                .count()
        };
        assert_eq!((muted(&first), muted(&second)), (1, 15));
    }

    #[test]
    fn repeats_sections_and_beat_text_are_written_and_given_room() {
        let plain = layout(&demo_score(), &view(Notation::Tab), 1000.0);

        let mut score = demo_score();
        let first = &mut score.master_bars[0];
        first.repeat_start = true;
        first.section = Some("Chorus".into());
        first.targets = vec![gtr_model::Target::Segno];
        first.alternate_endings = 0b01;
        first.repeat_count = 3;
        let second = &mut score.master_bars[1];
        second.alternate_endings = 0b110;
        second.jumps = vec![gtr_model::Jump::DalSegnoAlFine];
        second.tempo = Some(90);
        second.tempo_changes = vec![gtr_model::TempoChange {
            position: Rational::new(1, 2),
            tempo: 132,
            gradual: false,
        }];
        second.feel = Some(gtr_model::Feel::SwingEighths);
        let beats = &mut score.tracks[0].staves[0].bars[0].voices[0].beats;
        beats[0].effects.chord = Some("Em7".into());
        beats[0].effects.text = Some("with feeling".into());
        beats[0].effects.fermata = true;
        beats[0].effects.lyrics = vec!["Hel".into(), "Good".into()];
        beats[1].effects.lyrics = vec!["lo".into()];
        let marked = layout(&score, &view(Notation::Tab), 1000.0);

        let found = texts(&marked);
        for label in [
            "Chorus",
            "\u{E047}",
            "\u{00D7}3",
            "1.",
            "2. 3.",
            "D.S. al Fine",
            "\u{2669} = 90",
            "\u{2669} = 132",
            "Swing 8ths",
            "Em7",
            "with feeling",
            "\u{E4C0}",
            "Hel",
            "Good",
            "lo",
        ] {
            assert!(found.contains(&label), "missing {label:?} in {found:?}");
        }
        // The bands for bar marks and endings, and the lanes for the fermata, chord name
        // and text, push the staff down; two lines of lyrics add to what is under it.
        let (was, now) = (&plain.systems[0], &marked.systems[0]);
        assert_eq!(
            now.y - was.y,
            14.0 + MARK_BAND + ENDING_BAND + 16.0 + 15.0 + 13.0
        );
        assert_eq!(
            (now.bottom - now.y) - (was.bottom - was.y),
            2.0 * LYRIC_LINE
        );
        // Repeat signs: two dots at each end, and the bars make room for them.
        let dots = |l: &Layout| {
            l.items
                .iter()
                .filter(|i| matches!(i, Item::Dot { .. }))
                .count()
        };
        assert_eq!(dots(&marked) - dots(&plain), 4);
        assert!(
            marked.bars[0].beats[0] - marked.bars[0].x > plain.bars[0].beats[0] - plain.bars[0].x
        );
    }

    #[test]
    fn credits_extend_the_header_only_when_there_are_any() {
        let mut score = demo_score();
        let plain = layout(&score, &view(Notation::Tab), 1000.0);
        score.info.artist = "Someone".into();
        assert_eq!(
            layout(&score, &view(Notation::Tab), 1000.0).systems[0].top,
            plain.systems[0].top
        );

        score.info.subtitle = "A study".into();
        score.info.words = "A. Writer".into();
        score.info.music = "B. Composer".into();
        score.info.transcriber = "C. Tabber".into();
        score.info.copyright = "\u{00A9} 2026".into();
        let full = layout(&score, &view(Notation::Tab), 1000.0);
        let found = texts(&full);
        for label in [
            "A study",
            "Words by A. Writer",
            "Music by B. Composer",
            "Tabbed by C. Tabber",
            "\u{00A9} 2026",
        ] {
            assert!(found.contains(&label), "missing {label:?}");
        }
        assert!(full.systems[0].top > plain.systems[0].top);
        assert_eq!(
            full.height - full.systems[0].bottom,
            plain.height - plain.systems[0].bottom + 24.0
        );
    }

    #[test]
    fn a_piano_has_a_staff_for_each_hand_and_drums_their_own_clef() {
        let mut score = demo_score();
        let quarter = Duration::new(NoteValue::Quarter);
        let mut piano = Track::empty("Piano", Instrument::Piano, 2);
        piano.staves[0].bars[0].voices[0].beats = vec![Beat::new(quarter, vec![Note::new(67, 0)])];
        piano.staves[1].bars[0].voices[0].beats =
            vec![Beat::new(quarter, vec![Note::new(38, 0)]); 2];
        let mut drums = Track::empty("Drums", Instrument::Drums, 2);
        drums.staves[0].bars[0].voices[0].beats =
            vec![Beat::new(quarter, vec![Note::new(0, 42), Note::new(4, 36)])];
        score.tracks.push(piano);
        score.tracks.push(drums);

        // The piano alone, left hand being edited: two rows, treble over bass.
        let left_hand = View {
            track: 1,
            staff: 1,
            ..view(Notation::Standard)
        };
        let piano = layout(&score, &left_hand, 1000.0);
        let rows = &piano.systems[0].rows;
        assert_eq!(
            rows.iter().map(|r| (r.track, r.staff)).collect::<Vec<_>>(),
            [(1, 0), (1, 1)]
        );
        let found = texts(&piano);
        assert!(found.contains(&"\u{E050}") && found.contains(&"\u{E062}"));
        // One bracket joins the two staves, from the top line of one to the bottom of the other.
        let brackets: Vec<(f64, f64)> = (piano.items.iter())
            .filter_map(|item| match item {
                Item::Rect { x, y, h, .. } if *x == MARGIN_X - 6.0 => Some((*y, *y + *h)),
                _ => None,
            })
            .collect();
        assert_eq!(brackets.len(), 1);
        assert!(brackets[0].0 < rows[0].bottom && brackets[0].1 > rows[1].top);
        // The cursor and the bar's beats belong to the lower staff.
        assert_eq!(piano.systems[0].y, rows[1].y);
        assert_eq!(piano.bars[0].beats.len(), 2);
        assert_eq!(piano.bars[0].status, BarStatus::Incomplete);

        // The drums: a percussion clef, a cross for the hi-hat, and frets on the tab.
        let kit = View {
            track: 2,
            ..view(Notation::Both)
        };
        let drums = layout(&score, &kit, 1000.0);
        let found = texts(&drums);
        assert!(found.contains(&"\u{E069}") && found.contains(&"\u{E0A9}"));
        assert!(found.contains(&"42") && found.contains(&"36"));
        assert_eq!(drums.string_count, 5);

        // Everything at once: four rows, the piano's two bracketed together.
        let all = View {
            multitrack: true,
            ..view(Notation::Standard)
        };
        assert_eq!(layout(&score, &all, 1000.0).systems[0].rows.len(), 4);

        // A piano has no strings, so no view gives it a tab staff: asked for tab alone or
        // for both, it is drawn as its two notation staves, one straight under the other.
        for notation in [Notation::Tab, Notation::Both] {
            let shown = layout(
                &score,
                &View {
                    track: 1,
                    staff: 1,
                    ..view(notation)
                },
                1000.0,
            );
            let rows = &shown.systems[0].rows;
            assert_eq!(rows.len(), 2);
            assert!(rows.iter().all(|r| r.strings == 0));
            let found = texts(&shown);
            assert!(found.contains(&"\u{E050}") && found.contains(&"\u{E062}"));
            assert!(!found.contains(&"T") && !found.contains(&"67"));
            // The cursor is placed from the middle line of the staff being edited: D3 in
            // the bass clef, where the left hand's notes are a step and a half octave apart.
            assert!(!shown.tab && shown.string_count == 128);
            assert_eq!((shown.middle, rows[0].middle), (Some(22), Some(34)));
            assert_eq!(shown.systems[0].y, rows[1].y);
            assert_eq!(rows[1].y, piano.systems[0].rows[1].y);
        }
        // Beside a guitar in the tab view, the guitar keeps its tab and the piano its notes.
        let mixed = layout(
            &score,
            &View {
                multitrack: true,
                ..view(Notation::Tab)
            },
            1000.0,
        );
        let strings: Vec<usize> = mixed.systems[0].rows.iter().map(|r| r.strings).collect();
        assert_eq!(strings, [6, 0, 0, 5]);
        assert!(mixed.tab && mixed.middle.is_none());
    }

    #[test]
    fn diagrams_mix_changes_and_timers_are_drawn() {
        let plain = layout(&demo_score(), &view(Notation::Tab), 1000.0);
        let mut score = demo_score();
        score.master_bars[1].timer = true;
        let beat = &mut score.tracks[0].staves[0].bars[0].voices[0].beats[0];
        beat.effects.diagram = Some(gtr_model::ChordDiagram {
            frets: vec![0, 0, 0, 2, 2, 0],
        });
        beat.effects.volume = Some(40);
        beat.effects.pan = Some(-30);
        let marked = layout(&score, &view(Notation::Tab), 1000.0);
        let found = texts(&marked);
        // Two seconds a bar at 120: the second bar starts two seconds in.
        for label in ["[0:02]", "vol 40, pan 30 L", "\u{25CB}"] {
            assert!(found.contains(&label), "missing {label:?} in {found:?}");
        }
        assert!(!found.contains(&"\u{00D7}") && !found.iter().any(|t| t.ends_with("fr")));
        // Two stopped strings: two dots more than the plain score has.
        let dots = |l: &Layout| {
            l.items
                .iter()
                .filter(|i| matches!(i, Item::Dot { .. }))
                .count()
        };
        assert_eq!(dots(&marked) - dots(&plain), 2);
        // The band for the timer, and the lanes for the mix change and the diagram.
        assert_eq!(
            marked.systems[0].y - plain.systems[0].y,
            14.0 + MARK_BAND + 13.0 + 52.0
        );

        // A switch of sound is written by the sound's name, in the same lane.
        score.tracks[0].sounds = vec![gtr_model::Sound {
            name: "Lead".into(),
            instrument: gtr_model::Instrument::DistortionGuitar,
            effects: Vec::new(),
        }];
        let beats = &mut score.tracks[0].staves[0].bars[0].voices[0].beats;
        beats[0].effects.sound = Some(1);
        beats[1].effects.sound = Some(0);
        let switched = layout(&score, &view(Notation::Tab), 1000.0);
        let found = texts(&switched);
        for label in ["sound: Lead, vol 40, pan 30 L", "own sound"] {
            assert!(found.contains(&label), "missing {label:?} in {found:?}");
        }
        let beats = &mut score.tracks[0].staves[0].bars[0].voices[0].beats;
        (beats[0].effects.sound, beats[1].effects.sound) = (None, None);

        // A chord up the neck says which fret it starts at and mutes a string.
        score.tracks[0].staves[0].bars[0].voices[0].beats[0]
            .effects
            .diagram = Some(gtr_model::ChordDiagram {
            frets: vec![7, 8, 9, 9, 7, -1],
        });
        let barre = layout(&score, &view(Notation::Tab), 1000.0);
        let found = texts(&barre);
        assert!(found.contains(&"7fr") && found.contains(&"\u{00D7}"));
    }

    /// A score of one bar of eighth notes on the top string, at the frets given.
    fn eighths(frets: &[u8]) -> Score {
        let mut score = demo_score();
        score.master_bars.truncate(1);
        let staff = &mut score.tracks[0].staves[0];
        staff.bars.truncate(1);
        staff.bars[0].voices[0].beats = (frets.iter())
            .map(|&fret| Beat::new(Duration::new(NoteValue::Eighth), vec![Note::new(0, fret)]))
            .collect();
        score
    }

    fn beams(layout: &Layout) -> Vec<(f64, f64, f64, f64)> {
        (layout.items.iter())
            .filter_map(|item| match item {
                Item::Beam { x1, y1, x2, y2, .. } => Some((*x1, *y1, *x2, *y2)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn beams_lean_the_way_their_notes_go() {
        // Four pairs: rising, falling, level, and a second pair of the same.
        let score = eighths(&[0, 7, 8, 1, 3, 3, 0, 7]);
        let layout = layout(&score, &view(Notation::Standard), 1000.0);
        let found = beams(&layout);
        assert_eq!(found.len(), 4);
        let rise = |b: &(f64, f64, f64, f64)| b.1 - b.3;
        assert!(rise(&found[0]) > 2.0, "a rising pair has a rising beam");
        assert!(rise(&found[1]) < -2.0, "a falling pair has a falling beam");
        assert_eq!(rise(&found[2]), 0.0);
        assert_eq!(rise(&found[3]), rise(&found[0]));
        // Never steeper than a beam should be, however far apart the notes are.
        for beam in &found {
            assert!(rise(beam).abs() <= 1.25 * SPACE + 0.1);
            assert!(rise(beam).abs() <= 0.3 * (beam.2 - beam.0) + 0.1);
        }
        // Every stem ends on its beam: these notes have their stems down, so the beam's
        // top edge is its thickness short of where the stems end.
        let stems: Vec<(f64, f64)> = (layout.items.iter())
            .filter_map(|item| match item {
                Item::Line {
                    x1, x2, y1, y2, w, ..
                } if x1 == x2 && *w == 1.2 => Some((*x1, y1.max(*y2))),
                _ => None,
            })
            .collect();
        assert_eq!(stems.len(), 8);
        for (pair, beam) in stems.chunks(2).zip(&found) {
            for (x, end) in pair {
                let along = (x - beam.0) / (beam.2 - beam.0);
                let top = beam.1 + (beam.3 - beam.1) * along;
                assert!((top + 4.0 - end).abs() < 0.3, "stem at {x} misses its beam");
            }
        }

        // A group whose middle notes dip nearer the beam than either end has a level
        // beam, and so do rhythm slashes; the beams under a tab staff are plain bars.
        let sixteenths = |score: &mut Score| {
            for beat in &mut score.tracks[0].staves[0].bars[0].voices[0].beats {
                beat.duration = Duration::new(NoteValue::Sixteenth);
            }
        };
        let mut arch = eighths(&[5, 0, 3, 7, 0, 3, 5, 7]);
        sixteenths(&mut arch);
        let found = beams(&super::layout(&arch, &view(Notation::Standard), 1000.0));
        // Two groups of four, each with two beams, one under the other and parallel.
        assert_eq!(found.len(), 4);
        assert_eq!(rise(&found[0]), 0.0);
        assert!(rise(&found[2]) > 2.0);
        assert_eq!(rise(&found[2]), rise(&found[3]));
        assert!(
            beams(&super::layout(&arch, &view(Notation::Slash), 1000.0))
                .iter()
                .all(|b| rise(b) == 0.0)
        );
        assert!(beams(&super::layout(&arch, &view(Notation::Tab), 1000.0)).is_empty());
    }

    /// Y of the first text item that reads `label`.
    fn y_of_text(layout: &Layout, label: &str) -> f64 {
        (layout.items.iter())
            .find_map(|item| match item {
                Item::Text { s, y, .. } if s == label => Some(*y),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no {label:?}"))
    }

    #[test]
    fn markings_go_above_the_notation_when_it_is_shown_over_the_tab() {
        let mut score = demo_score();
        let beats = &mut score.tracks[0].staves[0].bars[0].voices[0].beats;
        beats[0].effects.chord = Some("Em7".into());
        beats[0].effects.text = Some("with feeling".into());
        beats[0].effects.diagram = Some(gtr_model::ChordDiagram {
            frets: vec![7, 8, 9, 9, 7, -1],
        });
        beats[0].effects.dynamic = Some(gtr_model::Dynamic::Ff);
        beats[0].effects.lyrics = vec!["Hel".into()];
        beats[1].notes[0].effects.palm_mute = true;
        beats[2].notes[0].effects.harmonic = Some(gtr_model::Harmonic::Pinch);
        beats[3].notes[0].effects.bend =
            Some(gtr_model::Bend::preset(gtr_model::BendKind::Bend, 4));
        beats[4].notes[0].effects.hammer = true;

        let both = layout(&score, &view(Notation::Both), 1000.0);
        let system = &both.systems[0];
        // The top line of the notation, and the top string of the tab under it.
        let (staff_top, tab_top) = (system.cursor_top, system.y);
        assert!(staff_top < tab_top);
        for label in ["Em7", "with feeling", "7fr", "P.M.", "P.H."] {
            let y = y_of_text(&both, label);
            assert!(y < staff_top, "{label:?} at {y} is not above the notation");
        }
        // What is written on the strings stays there: the bend, with its size, between
        // the staves, and the hammer-on's letter on the tab.
        for label in ["full", "H"] {
            let y = y_of_text(&both, label);
            assert!(y > staff_top + 4.0 * SPACE && y < tab_top + 5.0 * STRING_SPACING);
        }
        // Dynamics and lyrics are still under the tab, and each is written once.
        let fortissimo = "\u{E52F}";
        for label in [fortissimo, "Hel"] {
            assert!(y_of_text(&both, label) > tab_top + 5.0 * STRING_SPACING);
        }
        let found = texts(&both);
        for label in ["Em7", "P.M.", "P.H.", "full", fortissimo, "Hel"] {
            assert_eq!(
                found.iter().filter(|t| **t == label).count(),
                1,
                "{label:?}"
            );
        }
        // Only the bend's lane is left between the staves.
        let plain = layout(&demo_score(), &view(Notation::Both), 1000.0);
        let gap = |l: &Layout| l.systems[0].y - l.systems[0].cursor_top;
        assert_eq!(gap(&both), gap(&plain) + 30.0);

        // In the tab view they are all over the tab, as before.
        let tab = layout(&score, &view(Notation::Tab), 1000.0);
        for label in ["Em7", "P.M.", "full"] {
            assert!(y_of_text(&tab, label) < tab.systems[0].y);
        }
    }

    #[test]
    fn notation_alone_shows_bends_and_slides() {
        use gtr_model::{Bend, BendKind, Slide};
        let lines = |l: &Layout| {
            (l.items.iter())
                .filter(|i| matches!(i, Item::Line { c: Color::Ink, w, .. } if *w == 1.1))
                .count()
        };
        let curves = |l: &Layout| {
            (l.items.iter())
                .filter(|i| matches!(i, Item::Curve { .. }))
                .count()
        };
        let plain = eighths(&[3, 5, 7, 8]);
        let was = layout(&plain, &view(Notation::Standard), 1000.0);

        let mut score = plain.clone();
        let beats = &mut score.tracks[0].staves[0].bars[0].voices[0].beats;
        beats[0].notes[0].effects.slide = Some(Slide::Shift);
        beats[1].notes[0].effects.slide = Some(Slide::InFromBelow);
        beats[2].notes[0].effects.slide = Some(Slide::OutDown);
        let slid = layout(&score, &view(Notation::Standard), 1000.0);
        assert_eq!(lines(&slid) - lines(&was), 3);
        // The slide to the next note runs from one head to the other, rising with them.
        let shift = (slid.items.iter())
            .find_map(|item| match item {
                Item::Line {
                    x1, y1, x2, y2, w, ..
                } if *w == 1.1 => Some((*x1, *y1, *x2, *y2)),
                _ => None,
            })
            .unwrap();
        let (from, to) = (slid.bars[0].beats[0], slid.bars[0].beats[1]);
        assert!(from < shift.0 && shift.0 < shift.2 && shift.2 < to);
        assert!(shift.3 < shift.1);
        // Over a tab staff the tab has the slides, once.
        let both = |score: &Score| layout(score, &view(Notation::Both), 1000.0);
        assert_eq!(lines(&both(&score)) - lines(&both(&plain)), 3);

        // A bend rises from its notehead into the lane above the staff, with its size.
        let mut score = plain.clone();
        score.tracks[0].staves[0].bars[0].voices[0].beats[3].notes[0]
            .effects
            .bend = Some(Bend::preset(BendKind::BendRelease, 4));
        let bent = layout(&score, &view(Notation::Standard), 1000.0);
        assert_eq!(curves(&bent) - curves(&was), 2);
        assert!(y_of_text(&bent, "full") < bent.systems[0].cursor_top);
        assert_eq!(bent.systems[0].y, was.systems[0].y + 30.0);
        let over_tab = both(&score);
        let sizes = texts(&over_tab).into_iter().filter(|t| *t == "full");
        assert_eq!(sizes.count(), 1);
    }

    #[test]
    fn a_key_change_at_the_start_of_a_line_is_written_once() {
        // Enough bars for a second line, with a change to four sharps wherever it starts.
        let mut score = demo_score();
        for _ in 0..10 {
            score
                .master_bars
                .push(MasterBar::new(gtr_model::TimeSignature::COMMON));
            let bar = score.tracks[0].staves[0].bars[0].clone();
            score.tracks[0].staves[0].bars.push(bar);
        }
        let plain = layout(&score, &view(Notation::Standard), 900.0);
        let second = (plain.bars.iter()).position(|b| b.system == 1).unwrap();
        for master in &mut score.master_bars[second..] {
            master.key_signature = 4;
        }
        let keyed = layout(&score, &view(Notation::Standard), 900.0);
        assert_eq!(keyed.bars[second].system, 1);
        // Four sharps on the line that changes key, and four at the start of each after it.
        let lines = keyed.systems.len();
        let sharps = (keyed.items.iter())
            .filter(|i| matches!(i, Item::Text { s, x, .. } if s == "\u{E262}" && *x < 140.0))
            .count();
        assert_eq!(sharps, 4 * (lines - 1));
        // The bar number steps up out of their way.
        let number = |layout: &Layout, bar: usize| y_of_text(layout, &(bar + 1).to_string());
        let top = |layout: &Layout, bar: usize| layout.systems[layout.bars[bar].system].cursor_top;
        assert_eq!(top(&keyed, second) - number(&keyed, second), 17.0);
        assert_eq!(top(&plain, second) - number(&plain, second), 9.0);
    }

    #[test]
    fn a_bar_is_as_wide_as_what_is_written_over_it() {
        // Two bars of one whole note each: narrow, until they are given marks.
        let mut score = eighths(&[0]);
        score.tracks[0].staves[0].bars[0].voices[0].beats[0].duration =
            Duration::new(NoteValue::Whole);
        score.master_bars.push(score.master_bars[0].clone());
        let bar = score.tracks[0].staves[0].bars[0].clone();
        score.tracks[0].staves[0].bars.push(bar);
        score.master_bars.push(score.master_bars[0].clone());
        let bar = score.tracks[0].staves[0].bars[0].clone();
        score.tracks[0].staves[0].bars.push(bar);
        let plain = layout(&score, &view(Notation::Tab), 1000.0);

        score.master_bars[0].repeat_count = 3;
        score.master_bars[1].jumps = vec![gtr_model::Jump::DalSegnoAlFine];
        score.master_bars[1].timer = true;
        score.master_bars[1].section = Some("Middle eight".into());
        let marked = layout(&score, &view(Notation::Tab), 1000.0);
        assert!(marked.bars[1].w > plain.bars[1].w + 60.0);
        // The label at the end of the second bar starts after its section name, inside
        // the bar, and so clear of the first bar's.
        let x_of = |label: &str| {
            (marked.items.iter())
                .find_map(|item| match item {
                    Item::Text { s, x, .. } if s == label => Some(*x),
                    _ => None,
                })
                .unwrap()
        };
        // The first bar is played three times, two seconds each, before the second is reached.
        let ending = x_of("[0:06]  D.S. al Fine");
        assert!(ending > marked.bars[1].x + 12.0 * 6.4);
        assert!(x_of("\u{00D7}3") + 12.0 < marked.bars[1].x);
        assert_eq!(marked.bars[2].w, plain.bars[2].w);
    }

    #[test]
    fn slash_view_shows_rhythm_only() {
        let layout = layout(&demo_score(), &view(Notation::Slash), 1000.0);
        let found = texts(&layout);
        assert_eq!(found.iter().filter(|t| **t == "\u{E101}").count(), 12);
        assert_eq!(found.iter().filter(|t| **t == "\u{E103}").count(), 1);
        assert!(!found.contains(&"\u{E050}") && !found.contains(&"T"));
    }

    #[test]
    fn a_single_line_has_every_bar_at_its_own_width() {
        let mut score = demo_score();
        let riff = score.tracks[0].staves[0].bars.clone();
        score.master_bars = vec![MasterBar::new(TimeSignature::COMMON); 40];
        score.tracks[0].staves[0].bars = (0..40).map(|bar| riff[bar % 2].clone()).collect();
        let wrapped = layout(&score, &View::default(), 800.0);
        let view = View {
            single_line: true,
            ..View::default()
        };
        let line = layout(&score, &view, 800.0);
        assert_eq!(line.systems.len(), 1);
        assert!(wrapped.systems.len() > 4);
        assert!(line.bars.iter().all(|bar| bar.system == 0));
        // Nothing is stretched: the bars follow on from one another at their own widths,
        // and the layout is as wide as the line.
        for pair in line.bars.windows(2) {
            assert!((pair[0].x + pair[0].w - pair[1].x).abs() < 0.2);
        }
        let end = line.bars.last().map(|bar| bar.x + bar.w).unwrap();
        assert!(line.width >= end && line.width > 2000.0, "{}", line.width);
        // The heading is laid out to the width asked for.
        let title_x = (line.items.iter())
            .find_map(|item| match item {
                Item::Text { s, x, .. } if s == "Demo riff" => Some(*x),
                _ => None,
            })
            .unwrap();
        assert_eq!(title_x, 400.0);
    }
}
