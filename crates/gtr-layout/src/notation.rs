//! The standard notation staff: pitches spelled from the tab, then noteheads, stems, beams,
//! accidentals and ledger lines. It can also show rhythm slashes in place of pitches.

use gtr_model::{
    Bar, Beat, Clef, DrumHead, Duration, Harmonic, KitPiece, MasterBar, Note, NoteValue, Rational,
    Slide, Staff, TimeSignature, Voice,
};

use crate::display::r;
use crate::spacing::{KEY_ACCIDENTAL_WIDTH, TIME_SIG_DIGIT_WIDTH};
use crate::{Align, BarBox, Color, Font, Item};

/// Distance between two staff lines.
pub(crate) const SPACE: f64 = 9.0;
/// Font size at which a SMuFL glyph fits a staff with lines [`SPACE`] apart.
const GLYPH: f64 = 4.0 * SPACE;
/// Half the width of a black notehead.
const HEAD_HALF: f64 = 5.3;
const STEM: f64 = 3.5 * SPACE;
const BEAM_THICKNESS: f64 = 4.0;
const BEAM_PITCH: f64 = 6.5;

/// SMuFL code points.
const G_CLEF: char = '\u{E050}';
const F_CLEF: char = '\u{E062}';
const PERCUSSION_CLEF: char = '\u{E069}';
const TIME_SIG_0: u32 = 0xE080;
const REST_WHOLE: u32 = 0xE4E3;
const FLAG_8TH_UP: u32 = 0xE240;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Accidental {
    Flat,
    Natural,
    Sharp,
}

impl Accidental {
    fn glyph(self) -> char {
        match self {
            Accidental::Flat => '\u{E260}',
            Accidental::Natural => '\u{E261}',
            Accidental::Sharp => '\u{E262}',
        }
    }

    fn of(alter: i8) -> Accidental {
        match alter {
            ..0 => Accidental::Flat,
            0 => Accidental::Natural,
            _ => Accidental::Sharp,
        }
    }
}

/// Where a note sits on the staff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Spelled {
    /// Steps above the middle line: lines are even, spaces odd.
    pub position: i32,
    /// The sign written before the note, if it needs one.
    pub accidental: Option<Accidental>,
    /// The notehead of a piece of the drum kit that has its own, as a SMuFL code point.
    pub head: Option<u32>,
}

/// The notes of one bar, as `[voice][beat][note]`.
pub(crate) type SpelledBar = Vec<Vec<Vec<Spelled>>>;

/// Steps of the scale, counted from C, that a key signature sharpens or flattens, in the
/// order the signs are written.
const SHARP_ORDER: [usize; 7] = [3, 0, 4, 1, 5, 2, 6];
const FLAT_ORDER: [usize; 7] = [6, 2, 5, 1, 4, 0, 3];

/// Step and alteration of each pitch class, spelled with sharps or with flats.
const SHARP_SPELLING: [(i32, i8); 12] = [
    (0, 0),
    (0, 1),
    (1, 0),
    (1, 1),
    (2, 0),
    (3, 0),
    (3, 1),
    (4, 0),
    (4, 1),
    (5, 0),
    (5, 1),
    (6, 0),
];
const FLAT_SPELLING: [(i32, i8); 12] = [
    (0, 0),
    (1, -1),
    (1, 0),
    (2, -1),
    (2, 0),
    (3, 0),
    (4, -1),
    (4, 0),
    (5, -1),
    (5, 0),
    (6, -1),
    (6, 0),
];

/// The alteration a key signature gives each step of the scale.
fn key_alters(key: i8) -> [i8; 7] {
    let mut alters = [0; 7];
    let count = usize::from(key.unsigned_abs()).min(7);
    let (order, alter) = if key > 0 {
        (SHARP_ORDER, 1)
    } else {
        (FLAT_ORDER, -1)
    };
    for step in &order[..count] {
        alters[*step] = alter;
    }
    alters
}

/// The diatonic step, counted from C0, on a clef's middle line.
pub(crate) fn middle_line(clef: Clef) -> i32 {
    match clef {
        // B4
        Clef::Treble => 4 * 7 + 6,
        // D3
        Clef::Bass => 3 * 7 + 1,
        // A percussion staff has no pitches; its notes are placed by the drum kit.
        Clef::Percussion => 0,
    }
}

/// Works out where every note of a staff goes in standard notation. Sharp keys spell the
/// notes outside the key with sharps and flat keys with flats; a sign lasts to the end of
/// its bar.
pub(crate) fn spell(
    staff: &Staff,
    clef: Clef,
    transposition: i8,
    masters: &[MasterBar],
    kit: &[KitPiece],
) -> Vec<SpelledBar> {
    let middle = middle_line(clef);
    (staff.bars.iter().zip(masters))
        .map(|(bar, master)| {
            let key = master.key_signature;
            let in_key = key_alters(key);
            let spelling = if key >= 0 {
                SHARP_SPELLING
            } else {
                FLAT_SPELLING
            };
            // Signs already written in this bar, by diatonic step.
            let mut written: Vec<(i32, i8)> = Vec::new();
            (bar.voices.iter())
                .map(|voice| {
                    (voice.beats.iter())
                        .map(|beat| {
                            (beat.notes.iter())
                                .map(|note| {
                                    if clef == Clef::Percussion {
                                        // The fret names a piece of the kit, which has its own place and shape.
                                        let piece = kit.iter().find(|p| p.number == note.fret);
                                        return Spelled {
                                            position: piece.map_or(0, |p| p.position.into()),
                                            accidental: None,
                                            head: piece.and_then(|p| match p.head {
                                                DrumHead::Normal => None,
                                                DrumHead::Cross => Some(0xE0A9),
                                                DrumHead::CircledCross => Some(0xE0B3),
                                                DrumHead::Diamond => Some(0xE0DB),
                                                DrumHead::Triangle => Some(0xE0BE),
                                            }),
                                        };
                                    }
                                    let sounding =
                                        staff.pitch(note.string, note.fret).unwrap_or(60);
                                    let pitch = i32::from(sounding) + i32::from(transposition);
                                    let (step, alter) = spelling[pitch.rem_euclid(12) as usize];
                                    let diatonic = (pitch.div_euclid(12) - 1) * 7 + step;
                                    let current = (written.iter().rev())
                                        .find(|(d, _)| *d == diatonic)
                                        .map_or(in_key[step as usize], |(_, a)| *a);
                                    let changed = alter != current && !note.tied;
                                    if changed {
                                        written.push((diatonic, alter));
                                    }
                                    Spelled {
                                        position: diatonic - middle,
                                        accidental: changed.then(|| Accidental::of(alter)),
                                        head: None,
                                    }
                                })
                                .collect()
                        })
                        .collect()
                })
                .collect()
        })
        .collect()
}

/// A drum staff redrawn with the feet apart from the hands.
pub(crate) struct Split {
    /// The staff with every bar that is to be split in two voices: the hands' and the feet's.
    pub staff: Staff,
    /// For each bar that was split, the beat of the bar as written that each beat of the
    /// feet's voice stands under, or `None` for a rest that is not drawn.
    pub feet: Vec<Option<Vec<Option<usize>>>>,
}

/// The note values without a tuplet, with a dot and without, longest first.
fn plain_durations() -> impl Iterator<Item = Duration> {
    NoteValue::ALL.into_iter().flat_map(|value| {
        [1, 0].map(|dots| Duration {
            dots,
            ..Duration::new(value)
        })
    })
}

/// Splits the bars of a drum staff that are written in one voice into two: what the hands
/// play, with stems up, and what the feet play, with stems down, as drum music is read.
/// The hands keep the rhythm as written. A foot's note lasts until the next one or the end
/// of its beat, whichever comes first, and the gaps between them are rests that are not
/// drawn. Returns nothing if no bar has both hands and feet in a single voice.
pub(crate) fn split_feet(staff: &Staff, kit: &[KitPiece], masters: &[MasterBar]) -> Option<Split> {
    let foot = |note: &Note| kit.iter().any(|p| p.foot && p.number == note.fret);
    let zero = Rational::from_integer(0);
    let mut feet = vec![None; staff.bars.len()];
    let mut bars = staff.bars.clone();
    for ((bar, master), source) in bars.iter_mut().zip(masters).zip(&mut feet) {
        let written = bar.voices.iter().filter(|v| !v.beats.is_empty()).count();
        let Some(beats) = bar.voices.first().map(|v| v.beats.clone()) else {
            continue;
        };
        let notes = || beats.iter().flat_map(|b| &b.notes);
        if written != 1 || beats.is_empty() || !notes().any(foot) || notes().all(foot) {
            continue;
        }
        let signature = master.time_signature;
        let pulse = match signature.denominator == 8 && signature.numerator.is_multiple_of(3) {
            true => Rational::new(3, 8),
            false => Rational::new(1, 4),
        };
        // Where each beat starts, and where the bar ends.
        let mut starts = Vec::with_capacity(beats.len());
        let mut end = zero;
        for beat in &beats {
            starts.push(end);
            end += beat.duration.length();
        }
        let end = end.max(signature.bar_length());

        let mut low = Voice::default();
        let mut under: Vec<Option<usize>> = Vec::new();
        // Fills a silence with rests, longest first, as far as plain note values can.
        let rest = |mut length: Rational, low: &mut Voice, under: &mut Vec<Option<usize>>| {
            while let Some(duration) = plain_durations().find(|d| d.length() <= length) {
                length -= duration.length();
                low.beats.push(Beat::rest(duration));
                under.push(None);
            }
        };
        let kicks: Vec<usize> = (0..beats.len())
            .filter(|&i| beats[i].notes.iter().any(foot))
            .collect();
        rest(starts[kicks[0]], &mut low, &mut under);
        for (k, &i) in kicks.iter().enumerate() {
            let until = kicks.get(k + 1).map_or(end, |&next| starts[next]);
            let gap = until - starts[i];
            let written = beats[i].duration;
            // As long as the gap allows, up to a beat; never shorter than it was written.
            let duration = plain_durations()
                .find(|d| d.length() <= gap.min(pulse))
                .filter(|d| d.length() >= written.length())
                .unwrap_or(written);
            let notes = beats[i].notes.iter().filter(|n| foot(n)).cloned().collect();
            low.beats.push(Beat::new(duration, notes));
            under.push(Some(i));
            rest(gap - duration.length(), &mut low, &mut under);
        }
        let mut high = Voice { beats };
        for beat in &mut high.beats {
            beat.notes.retain(|n| !foot(n));
        }
        *bar = Bar {
            voices: vec![high, low],
        };
        *source = Some(under);
    }
    feet.iter().any(Option::is_some).then_some(Split {
        staff: Staff {
            tuning: staff.tuning.clone(),
            capo: staff.capo,
            bars,
        },
        feet,
    })
}

/// Room each beat of a bar needs to its left for its accidentals, as `[voice][beat]`.
pub(crate) fn accidental_leads(bar: &SpelledBar) -> Vec<Vec<f64>> {
    (bar.iter())
        .map(|voice| {
            (voice.iter())
                .map(
                    |beat| match beat.iter().filter(|n| n.accidental.is_some()).count() {
                        0 => 0.0,
                        1 => 9.0,
                        _ => 17.0,
                    },
                )
                .collect()
        })
        .collect()
}

fn y_of(position: i32) -> f64 {
    -f64::from(position) * SPACE / 2.0
}

fn glyph(x: f64, y: f64, code: u32) -> Item {
    Item::text(
        x,
        y,
        char::from_u32(code).unwrap_or(' '),
        GLYPH,
        Font::Music,
    )
}

fn left(mut item: Item) -> Item {
    if let Item::Text { align, .. } = &mut item {
        *align = Align::Left;
    }
    item
}

/// Draws the five lines of a staff whose middle line is at `mid`.
pub(crate) fn draw_lines(x1: f64, x2: f64, mid: f64, items: &mut Vec<Item>) {
    for line in -2..=2 {
        let y = mid + f64::from(line) * SPACE;
        items.insert(0, Item::line(x1, y, x2, y, 1.0, Color::Muted));
    }
}

pub(crate) fn draw_clef(clef: Clef, x: f64, mid: f64, items: &mut Vec<Item>) {
    // Each clef is anchored on the line it names: G on the second line up, F on the fourth.
    let (symbol, y) = match clef {
        Clef::Treble => (G_CLEF, mid + SPACE),
        Clef::Bass => (F_CLEF, mid - SPACE),
        Clef::Percussion => (PERCUSSION_CLEF, mid),
    };
    items.push(left(Item::text(x, y, symbol, GLYPH, Font::Music)));
}

/// Draws a key signature starting at `x`.
pub(crate) fn draw_key(key: i8, clef: Clef, x: f64, mid: f64, items: &mut Vec<Item>) {
    const SHARPS: [i32; 7] = [4, 1, 5, 2, -1, 3, 0];
    const FLATS: [i32; 7] = [0, 3, -1, 2, -2, 1, -3];
    let (positions, sign) = if key > 0 {
        (SHARPS, Accidental::Sharp)
    } else {
        (FLATS, Accidental::Flat)
    };
    let shift = match clef {
        Clef::Treble => 0,
        Clef::Bass => -2,
        // Drums have no key.
        Clef::Percussion => return,
    };
    let count = usize::from(key.unsigned_abs()).min(7);
    for (i, position) in positions[..count].iter().enumerate() {
        let at = x + (i as f64 + 0.5) * KEY_ACCIDENTAL_WIDTH;
        let y = mid + y_of(position + shift);
        items.push(Item::text(at, y, sign.glyph(), GLYPH, Font::Music));
    }
}

/// Draws a time signature whose left edge is at `x`.
pub(crate) fn draw_time(ts: TimeSignature, x: f64, mid: f64, items: &mut Vec<Item>) {
    let digits = ts.numerator.max(ts.denominator).to_string().len();
    let centre = x + digits as f64 * TIME_SIG_DIGIT_WIDTH / 2.0;
    for (number, y) in [(ts.numerator, mid - SPACE), (ts.denominator, mid + SPACE)] {
        let glyphs: String = number
            .to_string()
            .bytes()
            .filter_map(|d| char::from_u32(TIME_SIG_0 + u32::from(d - b'0')))
            .collect();
        items.push(Item::text(centre, y, glyphs, GLYPH, Font::Music));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stem {
    Up,
    Down,
}

/// A beat's notes placed on the staff, lowest first.
struct Chord<'a> {
    x: f64,
    notes: Vec<(&'a Note, Spelled)>,
}

impl Chord<'_> {
    fn lowest(&self) -> i32 {
        self.notes.first().map_or(0, |n| n.1.position)
    }

    fn highest(&self) -> i32 {
        self.notes.last().map_or(0, |n| n.1.position)
    }
}

/// One staff of standard notation over one line of music, drawn with its middle line at
/// zero; the caller moves the result to where the staff goes.
pub(crate) struct Notation<'a> {
    pub staff: &'a Staff,
    /// Where each note goes, per bar of the score; `None` draws rhythm slashes instead.
    pub spelled: Option<&'a [SpelledBar]>,
    /// X of every beat, as `[voice][bar][beat]`, for the bars laid out so far.
    pub xs: &'a [Vec<Vec<f64>>],
    pub bars: &'a [BarBox],
    pub masters: &'a [MasterBar],
    /// First bar of the line and the bar after its last.
    pub first: usize,
    pub end: usize,
    /// On a drum staff split into hands and feet, where the feet's beats stand; see
    /// [`Split::feet`].
    pub feet: Option<&'a [Option<Vec<Option<usize>>>]>,
}

/// Vertical space a staff's music takes, relative to its middle line.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Extent {
    pub top: f64,
    pub bottom: f64,
}

impl Extent {
    fn include(&mut self, y: f64) {
        self.top = self.top.min(y);
        self.bottom = self.bottom.max(y);
    }
}

impl Notation<'_> {
    pub fn draw(&self, items: &mut Vec<Item>) -> Extent {
        let mut extent = Extent {
            top: -2.0 * SPACE,
            bottom: 2.0 * SPACE,
        };
        for bar in self.first..self.end {
            let voices = &self.staff.bars[bar].voices;
            let sounding = voices.iter().filter(|v| !v.beats.is_empty()).count();
            for voice in 0..voices.len() {
                // With several voices in a bar, they take turns pointing up and down so
                // that they can be told apart.
                let forced = match (self.spelled, sounding > 1) {
                    (None, _) => Some(Stem::Up),
                    (_, true) if voice % 2 == 0 => Some(Stem::Up),
                    (_, true) => Some(Stem::Down),
                    _ => None,
                };
                self.draw_voice(bar, voice, forced, sounding > 1, items, &mut extent);
            }
        }
        extent
    }

    fn beats(&self, bar: usize, voice: usize) -> &[Beat] {
        (self.staff.bars.get(bar))
            .and_then(|b| b.voices.get(voice))
            .map_or(&[], |v| &v.beats)
    }

    fn x(&self, bar: usize, voice: usize, beat: usize) -> Option<f64> {
        // The feet's voice of a split bar was not laid out: its notes stand under the
        // beats of the bar as written, and its rests nowhere.
        if let Some(under) = self.feet.and_then(|feet| feet.get(bar)?.as_ref())
            && voice == 1
        {
            return self
                .xs
                .first()?
                .get(bar)?
                .get((*under.get(beat)?)?)
                .copied();
        }
        self.xs.get(voice)?.get(bar)?.get(beat).copied()
    }

    fn chord(&self, bar: usize, voice: usize, index: usize) -> Chord<'_> {
        let beat = &self.beats(bar, voice)[index];
        let mut notes: Vec<(&Note, Spelled)> = (beat.notes.iter().enumerate())
            .map(|(n, note)| {
                let spelled = match self.spelled {
                    Some(spelled) => spelled[bar][voice][index][n],
                    None => Spelled {
                        position: 0,
                        accidental: None,
                        head: None,
                    },
                };
                (note, spelled)
            })
            .collect();
        notes.sort_by_key(|n| n.1.position);
        if self.spelled.is_none() {
            notes.truncate(1);
        }
        Chord {
            x: self.x(bar, voice, index).unwrap_or(0.0),
            notes,
        }
    }

    fn draw_voice(
        &self,
        bar: usize,
        voice: usize,
        forced: Option<Stem>,
        shared: bool,
        items: &mut Vec<Item>,
        extent: &mut Extent,
    ) {
        let beats = self.beats(bar, voice);
        let time_signature = self.masters[bar].time_signature;
        let flags = |beat: &Beat| match beat.is_rest() {
            true => 0,
            false => beat.duration.value.flags(),
        };

        // Beams join notes that start within the same pulse, as under the tab.
        let compound =
            time_signature.denominator == 8 && time_signature.numerator.is_multiple_of(3);
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

        // Where each beat's stem ends, for placing tuplet numbers.
        let mut edges: Vec<Option<(f64, Stem)>> = vec![None; beats.len()];
        let mut start = 0;
        while start < beats.len() {
            let mut end = start + 1;
            if beats[start].is_rest() {
                self.draw_rest(bar, voice, start, forced.filter(|_| shared), items, extent);
            } else {
                if flags(&beats[start]) > 0 {
                    while end < beats.len()
                        && flags(&beats[end]) > 0
                        && pulses[end] == pulses[start]
                    {
                        end += 1;
                    }
                }
                let (edge, stem) = self.draw_group(bar, voice, start..end, forced, items, extent);
                for slot in &mut edges[start..end] {
                    *slot = edge.map(|y| (y, stem));
                }
            }
            start = end;
        }

        for (row, tuplet_of) in [
            (|b: &Beat| b.duration.tuplet) as fn(&Beat) -> _,
            |b: &Beat| b.duration.nested,
        ]
        .into_iter()
        .enumerate()
        {
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
                    let first = self.x(bar, voice, start).unwrap_or(0.0);
                    let last = self.x(bar, voice, end - 1).unwrap_or(first);
                    let stems: Vec<(f64, Stem)> =
                        edges[start..end].iter().flatten().copied().collect();
                    let below = stems.first().is_some_and(|(_, s)| *s == Stem::Down);
                    let offset = 9.0 + 10.0 * row as f64;
                    let y = if below {
                        stems.iter().map(|s| s.0).fold(2.0 * SPACE, f64::max) + offset
                    } else {
                        stems.iter().map(|s| s.0).fold(-2.0 * SPACE, f64::min) - offset
                    };
                    let label = tuplet.actual.to_string();
                    items.push(Item::text((first + last) / 2.0, y, label, 10.0, Font::Text));
                    extent.include(y - 6.0);
                    extent.include(y + 6.0);
                }
                start = end;
            }
        }
    }

    fn draw_rest(
        &self,
        bar: usize,
        voice: usize,
        index: usize,
        side: Option<Stem>,
        items: &mut Vec<Item>,
        extent: &mut Extent,
    ) {
        let beat = &self.beats(bar, voice)[index];
        let Some(x) = self.x(bar, voice, index) else {
            return;
        };
        // In a bar shared by two voices, each keeps its rests to its own side.
        let shift = match side {
            Some(Stem::Up) => -2.0 * SPACE,
            Some(Stem::Down) => 2.0 * SPACE,
            None => 0.0,
        };
        // A whole rest hangs from the line above the middle; the others sit on the middle.
        let y = shift
            + match beat.duration.value {
                NoteValue::Whole => -SPACE,
                _ => 0.0,
            };
        items.push(glyph(x, y, REST_WHOLE + beat.duration.value as u32));
        for dot in 0..beat.duration.dots.min(2) {
            items.push(Item::Dot {
                x: r(x + 9.0 + 4.0 * f64::from(dot)),
                y: r(shift - SPACE / 2.0),
                r: 1.5,
            });
        }
        extent.include(shift - 1.5 * SPACE);
        extent.include(shift + 1.5 * SPACE);
    }

    /// Draws a run of notes that share a beam, or a single note. Returns the y at which
    /// the stems end, if they have stems, and the way they point.
    fn draw_group(
        &self,
        bar: usize,
        voice: usize,
        range: std::ops::Range<usize>,
        forced: Option<Stem>,
        items: &mut Vec<Item>,
        extent: &mut Extent,
    ) -> (Option<f64>, Stem) {
        let beats = &self.beats(bar, voice)[range.clone()];
        let chords: Vec<Chord> = range.clone().map(|i| self.chord(bar, voice, i)).collect();

        // Stems point away from the note furthest from the middle line; on the line, down.
        let lowest = chords.iter().map(Chord::lowest).min().unwrap_or(0);
        let highest = chords.iter().map(Chord::highest).max().unwrap_or(0);
        let stem = forced.unwrap_or(if -lowest > highest {
            Stem::Up
        } else {
            Stem::Down
        });
        let deepest = beats
            .iter()
            .map(|b| b.duration.value.flags())
            .max()
            .unwrap_or(0);
        let length = STEM + f64::from(deepest.saturating_sub(2)) * BEAM_PITCH;
        // Every stem in the group ends on the same level, and reaches the middle line.
        let edge = match stem {
            Stem::Up => (y_of(highest) - length).min(0.0),
            Stem::Down => (y_of(lowest) + length).max(0.0),
        };
        let stemmed = beats[0].duration.value != NoteValue::Whole;
        let stem_x = |chord: &Chord| match stem {
            Stem::Up => chord.x + HEAD_HALF - 0.6,
            Stem::Down => chord.x - HEAD_HALF + 0.6,
        };

        for (offset, (chord, beat)) in chords.iter().zip(beats).enumerate() {
            self.draw_chord(
                bar,
                voice,
                range.start + offset,
                chord,
                beat,
                stem,
                items,
                extent,
            );
            if stemmed {
                let from = match stem {
                    Stem::Up => y_of(chord.lowest()) - 1.0,
                    Stem::Down => y_of(chord.highest()) + 1.0,
                };
                let x = stem_x(chord);
                items.push(Item::line(x, from, x, edge, 1.2, Color::Ink));
                // Tremolo picking: slashes across the stem.
                let slashes = beat.effects.tremolo.map_or(0, |v| v.flags());
                let middle = (from + edge) / 2.0;
                for slash in 0..slashes {
                    let y = middle + 4.0 * (f64::from(slash) - f64::from(slashes - 1) / 2.0);
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
        }
        if !stemmed {
            return (None, stem);
        }
        extent.include(edge);

        if let [chord] = chords.as_slice() {
            // A note on its own carries flags instead of a beam.
            let count = beats[0].duration.value.flags();
            if count > 0 {
                let code = FLAG_8TH_UP + 2 * (count - 1) + u32::from(stem == Stem::Down);
                items.push(left(glyph(stem_x(chord) - 0.6, edge, code)));
            }
            return (Some(edge), stem);
        }
        for level in 1..=deepest {
            let reaches = |i: usize| beats[i].duration.value.flags() >= level;
            let step = f64::from(level - 1) * BEAM_PITCH;
            let y = match stem {
                Stem::Up => edge + step,
                Stem::Down => edge - BEAM_THICKNESS - step,
            };
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
                let first = stem_x(&chords[start]);
                let (x, w) = match end - start {
                    // A lone short note gets a stub pointing into the group.
                    1 if start == 0 => (first, 7.0),
                    1 => (first - 7.0, 7.0),
                    _ => (first, stem_x(&chords[end - 1]) - first),
                };
                items.push(Item::rect(x - 0.6, y, w + 1.2, BEAM_THICKNESS, Color::Ink));
                start = end;
            }
        }
        (Some(edge), stem)
    }

    /// Draws a chord's noteheads and everything attached to them: ledger lines,
    /// accidentals, dots, ties and slurs.
    #[allow(clippy::too_many_arguments)]
    fn draw_chord(
        &self,
        bar: usize,
        voice: usize,
        index: usize,
        chord: &Chord,
        beat: &Beat,
        stem: Stem,
        items: &mut Vec<Item>,
        extent: &mut Extent,
    ) {
        let x = chord.x;
        let slash = self.spelled.is_none();
        let count = chord.notes.len();

        // Two notes a step apart cannot share a side of the stem: the second one moves
        // across to the other.
        let mut across = vec![false; count];
        match stem {
            Stem::Up => {
                for i in 1..count {
                    let step = chord.notes[i].1.position - chord.notes[i - 1].1.position;
                    across[i] = step == 1 && !across[i - 1];
                }
            }
            Stem::Down => {
                for i in (0..count.saturating_sub(1)).rev() {
                    let step = chord.notes[i + 1].1.position - chord.notes[i].1.position;
                    across[i] = step == 1 && !across[i + 1];
                }
            }
        }
        let shift = match stem {
            Stem::Up => 2.0 * HEAD_HALF - 1.2,
            Stem::Down => -(2.0 * HEAD_HALF - 1.2),
        };
        let head_x = |i: usize| if across[i] { x + shift } else { x };
        let right = x
            + HEAD_HALF
            + if stem == Stem::Up && across.contains(&true) {
                shift
            } else {
                0.0
            };
        let left_edge = x - HEAD_HALF
            + if stem == Stem::Down && across.contains(&true) {
                shift
            } else {
                0.0
            };

        // Ledger lines for notes beyond the staff.
        for line in (6..=chord.highest())
            .step_by(2)
            .chain((chord.lowest()..=-6).filter(|p| p % 2 == 0))
        {
            let y = y_of(line);
            items.push(Item::line(
                left_edge - 3.0,
                y,
                right + 3.0,
                y,
                1.0,
                Color::Ink,
            ));
        }

        let value = beat.duration.value;
        let mut dotted: Vec<i32> = Vec::new();
        let mut signs = 0;
        // From the top note down, so that accidentals can be staggered clear of each other.
        let mut last_sign: Option<i32> = None;
        for i in (0..count).rev() {
            let (note, spelled) = chord.notes[i];
            let y = y_of(spelled.position);
            let effects = &note.effects;
            let code = if slash {
                match value {
                    NoteValue::Whole => 0xE102,
                    NoteValue::Half => 0xE103,
                    _ => 0xE101,
                }
            } else if let Some(head) = spelled.head {
                head
            } else if effects.dead {
                0xE0A9
            } else if effects.harmonic.is_some_and(|h| h != Harmonic::Semi) {
                match value {
                    NoteValue::Whole | NoteValue::Half => 0xE0D9,
                    _ => 0xE0DB,
                }
            } else {
                match value {
                    NoteValue::Whole => 0xE0A2,
                    NoteValue::Half => 0xE0A3,
                    _ => 0xE0A4,
                }
            };
            items.push(glyph(head_x(i), y, code));
            extent.include(y - SPACE);
            extent.include(y + SPACE);
            if effects.ghost {
                for (side, mark) in [(-1.0, "("), (1.0, ")")] {
                    let at = head_x(i) + side * (HEAD_HALF + 3.0);
                    items.push(Item::text(at, y, mark, 12.0, Font::Text));
                }
            }

            if let Some(sign) = spelled.accidental {
                // A sign too close under the last one steps out to a second column.
                let crowded = last_sign.is_some_and(|above| above - spelled.position < 6);
                let column = if crowded && signs % 2 == 1 { 1.0 } else { 0.0 };
                signs = if crowded { signs + 1 } else { 1 };
                last_sign = Some(spelled.position);
                let at = left_edge - 5.5 - 8.0 * column - if effects.ghost { 5.0 } else { 0.0 };
                items.push(Item::text(at, y, sign.glyph(), GLYPH, Font::Music));
            }

            // Dots go in a space: a note on a line puts its dot in the space above.
            let space = spelled.position + i32::from(spelled.position % 2 == 0);
            if !dotted.contains(&space) {
                dotted.push(space);
                for dot in 0..beat.duration.dots.min(2) {
                    items.push(Item::Dot {
                        x: r(right + 4.5 + 4.0 * f64::from(dot)),
                        y: r(y_of(space)),
                        r: 1.5,
                    });
                }
            }

            // Ties and slurs curve away from the stem.
            let (curve_y, bulge) = match stem {
                Stem::Up => (y + 6.0, -4.0),
                Stem::Down => (y - 6.0, 4.0),
            };
            if note.tied && !slash {
                let from = match index.checked_sub(1) {
                    Some(previous) => self.x(bar, voice, previous),
                    None if bar > self.first => (self.xs.get(voice))
                        .and_then(|v| v.get(bar - 1)?.last().copied())
                        .or(Some(self.bars[bar].x)),
                    None => None,
                }
                .map_or(self.bars[bar].x - 4.0, |from| from + HEAD_HALF + 1.0);
                items.push(Item::Arc {
                    x1: r(from),
                    x2: r(x - HEAD_HALF - 1.0),
                    y: r(curve_y),
                    h: bulge,
                });
                extent.include(curve_y - bulge * 2.0);
            }
            if (effects.hammer || effects.slide == Some(Slide::Legato)) && !slash {
                let next = match self.x(bar, voice, index + 1) {
                    Some(next) => Some(next),
                    None => self.x(bar + 1, voice, 0).filter(|_| bar + 1 < self.end),
                };
                let to = next.unwrap_or(self.bars[bar].x + self.bars[bar].w - 2.0);
                items.push(Item::Arc {
                    x1: r(x + 1.0),
                    x2: r(to - 1.0),
                    y: r(curve_y + bulge.signum() * -2.0),
                    h: bulge * 1.25,
                });
                extent.include(curve_y - bulge * 3.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use gtr_model::{Bar, Beat, Duration, Tuning, Voice};

    use super::*;

    fn staff_of(frets: &[(u8, u8)]) -> Staff {
        Staff {
            tuning: Tuning::guitar_standard(),
            capo: 0,
            bars: vec![Bar {
                voices: vec![Voice {
                    beats: (frets.iter())
                        .map(|&(string, fret)| {
                            Beat::new(
                                Duration::new(NoteValue::Quarter),
                                vec![Note::new(string, fret)],
                            )
                        })
                        .collect(),
                }],
            }],
        }
    }

    fn spelled_in(key: i8, frets: &[(u8, u8)]) -> Vec<Spelled> {
        let mut master = MasterBar::new(TimeSignature::COMMON);
        master.key_signature = key;
        let bars = spell(&staff_of(frets), Clef::Treble, 12, &[master], &[]);
        bars[0][0].iter().map(|beat| beat[0]).collect()
    }

    #[test]
    fn guitar_strings_sit_where_guitarists_read_them() {
        // Open low E is three ledger lines below the staff; open high E is the top space.
        let notes = spelled_in(0, &[(5, 0), (0, 0), (2, 0)]);
        assert_eq!(notes[0].position, -11);
        assert_eq!(notes[1].position, 3);
        // Open G is on the second line up.
        assert_eq!(notes[2].position, -2);
        assert!(notes.iter().all(|n| n.accidental.is_none()));
    }

    #[test]
    fn accidentals_follow_the_key_and_last_for_the_bar() {
        // F sharp, F sharp again, F natural, then F natural again, in C major.
        let notes = spelled_in(0, &[(0, 2), (0, 2), (0, 1), (0, 1)]);
        let signs: Vec<_> = notes.iter().map(|n| n.accidental).collect();
        assert_eq!(
            signs,
            [
                Some(Accidental::Sharp),
                None,
                Some(Accidental::Natural),
                None
            ]
        );
        assert!(notes.iter().all(|n| n.position == notes[0].position));

        // In G major the F sharp is in the key and it is the natural that needs a sign.
        let notes = spelled_in(1, &[(0, 2), (0, 1)]);
        let signs: Vec<_> = notes.iter().map(|n| n.accidental).collect();
        assert_eq!(signs, [None, Some(Accidental::Natural)]);

        // Flat keys spell the same pitch as a flat on the step above.
        let sharp = spelled_in(0, &[(1, 4)]);
        let flat = spelled_in(-3, &[(1, 4)]);
        assert_eq!(flat[0].position, sharp[0].position + 1);
        assert_eq!(flat[0].accidental, None);
        assert_eq!(sharp[0].accidental, Some(Accidental::Sharp));
    }

    #[test]
    fn drums_are_placed_by_the_kit_not_by_pitch() {
        // Bass drum, snare and closed hi-hat, whatever line of the tab they are on.
        let mut staff = staff_of(&[(4, 36), (0, 38), (2, 42)]);
        staff.tuning = Tuning::new([0; 5]);
        let master = MasterBar::new(TimeSignature::COMMON);
        let kit = gtr_model::standard_kit();
        let bars = spell(
            &staff,
            Clef::Percussion,
            0,
            std::slice::from_ref(&master),
            &kit,
        );
        let notes: Vec<Spelled> = bars[0][0].iter().map(|beat| beat[0]).collect();
        assert_eq!(
            notes.iter().map(|n| n.position).collect::<Vec<_>>(),
            [-3, 1, 5]
        );
        assert_eq!(
            notes.iter().map(|n| n.head).collect::<Vec<_>>(),
            [None, None, Some(0xE0A9)]
        );
        assert!(notes.iter().all(|n| n.accidental.is_none()));

        // A kit of the track's own moves a piece and gives it another head.
        let mut kit = kit;
        let snare = kit.iter_mut().find(|p| p.number == 38).unwrap();
        (snare.position, snare.head) = (3, DrumHead::Triangle);
        let bars = spell(&staff, Clef::Percussion, 0, &[master], &kit);
        assert_eq!(
            (bars[0][0][1][0].position, bars[0][0][1][0].head),
            (3, Some(0xE0BE))
        );
        // A number the kit does not have is written on the middle line.
        assert_eq!(
            spell(
                &staff,
                Clef::Percussion,
                0,
                &[MasterBar::new(TimeSignature::COMMON)],
                &[]
            )[0][0][0][0]
                .position,
            0
        );
    }

    #[test]
    fn hands_and_feet_are_split_into_two_voices() {
        let eighth = Duration::new(NoteValue::Eighth);
        let beat = |frets: &[u8]| {
            let notes = frets
                .iter()
                .enumerate()
                .map(|(s, &f)| Note::new(s as u8, f));
            Beat::new(eighth, notes.collect())
        };
        // Eighth-note hi-hats, with the bass drum on one, the and of two, and three, and
        // the snare on two and four; then a bar of hi-hats alone, and a bar for the feet.
        let groove = [
            &[42, 36][..],
            &[42],
            &[42, 38],
            &[42, 36],
            &[42, 36],
            &[42],
            &[42, 38],
            &[42],
        ];
        let hats = vec![beat(&[42]); 8];
        let pedal = vec![Beat::new(
            Duration::new(NoteValue::Whole),
            vec![Note::new(0, 44)],
        )];
        let staff = Staff {
            tuning: Tuning::new([0; 5]),
            capo: 0,
            bars: [groove.iter().map(|f| beat(f)).collect(), hats, pedal]
                .into_iter()
                .map(|beats| Bar {
                    voices: vec![Voice { beats }],
                })
                .collect(),
        };
        let masters = vec![MasterBar::new(TimeSignature::COMMON); 3];
        let kit = gtr_model::standard_kit();
        let split = split_feet(&staff, &kit, &masters).unwrap();

        // Only the bar with both is split; the others are as they were written.
        assert_eq!(split.staff.bars[1..], staff.bars[1..]);
        assert!(split.feet[0].is_some() && split.feet[1].is_none() && split.feet[2].is_none());
        let [hands, feet] = split.staff.bars[0].voices.as_slice() else {
            panic!("the bar should have two voices");
        };
        // The hands keep their eight eighth notes, without the bass drum.
        assert_eq!(hands.beats.len(), 8);
        assert!(hands.beats.iter().all(|b| b.duration == eighth));
        let frets = |b: &Beat| b.notes.iter().map(|n| n.fret).collect::<Vec<_>>();
        assert_eq!(frets(&hands.beats[0]), [42]);
        assert_eq!(frets(&hands.beats[2]), [42, 38]);
        // The feet: a quarter note, a rest that is not drawn, two eighths and a quarter,
        // and rests to the end of the bar. It still adds up to the bar.
        let written: Vec<(u32, usize, Option<usize>)> = (feet.beats.iter())
            .zip(split.feet[0].as_ref().unwrap())
            .map(|(b, under)| (b.duration.value as u32, b.notes.len(), *under))
            .collect();
        let (quarter, eighth) = (2, 3);
        assert_eq!(
            written,
            [
                (quarter, 1, Some(0)),
                (eighth, 0, None),
                (eighth, 1, Some(3)),
                (quarter, 1, Some(4)),
                (quarter, 0, None),
            ]
        );
        let length: Rational = feet.beats.iter().map(|b| b.duration.length()).sum();
        assert_eq!(length, Rational::from_integer(1));

        // Nothing to split where the feet, or the hands, have the bar to themselves, or
        // where it is already written in two voices.
        assert!(
            split_feet(
                &Staff {
                    bars: staff.bars[1..].to_vec(),
                    ..staff.clone()
                },
                &kit,
                &masters
            )
            .is_none()
        );
        let mut voiced = staff.clone();
        voiced.bars[0].voices.push(Voice {
            beats: vec![beat(&[36])],
        });
        assert!(split_feet(&voiced, &kit, &masters).is_none());
    }

    #[test]
    fn bass_clef_centres_on_d() {
        let mut staff = staff_of(&[(1, 0)]);
        staff.tuning = Tuning::bass_standard();
        // Open D on a bass, written an octave up, is the middle line of the bass clef.
        let bars = spell(
            &staff,
            Clef::Bass,
            12,
            &[MasterBar::new(TimeSignature::COMMON)],
            &[],
        );
        assert_eq!(bars[0][0][0][0].position, 0);
    }
}
