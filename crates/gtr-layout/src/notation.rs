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
/// The steepest a beam gets, as rise over run, and the furthest it rises or falls.
const BEAM_SLOPE: f64 = 0.3;
const BEAM_SLANT: f64 = 1.25 * SPACE;

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

/// Pitch class of each step of the scale, counted from C.
const NATURAL: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

/// A way of writing a pitch: the step of the scale it is written on, counted from C, and
/// whether that step is lowered or raised.
type Name = (i32, i8);

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

/// The names a pitch can go by in a key. A pitch of the key has the name the key gives it,
/// and any other white key is a natural. Only the five that are left have two names: a
/// sharp, given first, and a flat.
fn names(pitch: i32, in_key: &[i8; 7]) -> (Name, Option<Name>) {
    let class = pitch.rem_euclid(12);
    let named = |alters: &[i8; 7]| {
        (0..7)
            .find(|&step| (NATURAL[step] + i32::from(alters[step])).rem_euclid(12) == class)
            .map(|step| (step as i32, alters[step]))
    };
    match named(in_key).or_else(|| named(&[0; 7])) {
        Some(name) => (name, None),
        None => (named(&[1; 7]).unwrap_or((0, 0)), named(&[-1; 7])),
    }
}

/// The diatonic step, counted from C0, that a pitch is written on under a name.
fn step_of(pitch: i32, (step, alter): Name) -> i32 {
    ((pitch - i32::from(alter)).div_euclid(12) - 1) * 7 + step
}

/// Whether two notes of a chord, each a pitch and the step it is written on, are as far
/// apart on the staff as they sound: a third written as a third, not as an augmented second.
fn fits(a: (i32, i32), b: (i32, i32)) -> bool {
    const STEPS: [i32; 12] = [0, 1, 1, 2, 2, 3, 3, 4, 5, 5, 6, 6];
    let (low, high) = if a.0 <= b.0 { (a, b) } else { (b, a) };
    let semitones = (high.0 - low.0).rem_euclid(12) as usize;
    let steps = (high.1 - low.1).rem_euclid(7);
    // A tritone is as good a fourth as a fifth.
    steps == STEPS[semitones] || (semitones == 6 && steps == 4)
}

/// A note outside the key that can be written as a sharp or as a flat.
struct Open {
    pitch: i32,
    sharp: Name,
    flat: Name,
    /// Which of the two the line it is in asks for.
    prefers_sharp: bool,
}

/// The most notes of one chord whose names are weighed against each other; beyond that
/// each goes by its own line.
const MOST_WEIGHED: usize = 6;

/// Chooses between sharp and flat for the notes of a chord that could be either, given
/// the pitches and steps of its other notes. The names that keep the chord's intervals
/// what they sound like win, and never two different pitches on one step; between names
/// that do that equally well, each note takes the one its own line asks for. Returns
/// whether each is a sharp.
fn weigh(open: &[Open], fixed: &[(i32, i32)]) -> Vec<bool> {
    let wanted: Vec<bool> = open.iter().map(|o| o.prefers_sharp).collect();
    if open.len() > MOST_WEIGHED || open.len() + fixed.len() < 2 {
        return wanted;
    }
    let mut best = (i32::MIN, wanted);
    for mask in 0..1u32 << open.len() {
        let sharps: Vec<bool> = (0..open.len()).map(|k| mask & (1 << k) == 0).collect();
        let mut placed = fixed.to_vec();
        let mut score = 0;
        for (o, &sharp) in open.iter().zip(&sharps) {
            let name = if sharp { o.sharp } else { o.flat };
            placed.push((o.pitch, step_of(o.pitch, name)));
            score += i32::from(sharp == o.prefers_sharp);
        }
        for (i, &a) in placed.iter().enumerate() {
            for &b in &placed[i + 1..] {
                score += match (a.0 == b.0, a.1 == b.1) {
                    (true, _) => 0,
                    (false, true) => -160,
                    (false, false) if fits(a, b) => 16,
                    _ => 0,
                };
            }
        }
        if score > best.0 {
            best = (score, sharps);
        }
    }
    best.1
}

/// Works out where every note of a staff goes in standard notation. A note of the key is
/// written as the key has it. One outside it that could be a sharp or a flat is spelled
/// by what is around it: by the chord it is in, so that a third reads as a third; then by
/// where its line goes, a sharp if it rises a semitone and a flat if it falls one; then
/// as it was last written in the bar; and failing all that with sharps in sharp keys and
/// flats in flat keys. A sign lasts to the end of its bar.
pub(crate) fn spell(
    staff: &Staff,
    clef: Clef,
    transposition: i8,
    masters: &[MasterBar],
    kit: &[KitPiece],
) -> Vec<SpelledBar> {
    if clef == Clef::Percussion {
        // The fret names a piece of the kit, which has its own place and shape.
        let placed = |note: &Note| {
            let piece = kit.iter().find(|p| p.number == note.fret);
            Spelled {
                position: piece.map_or(0, |p| p.position.into()),
                accidental: None,
                head: piece.and_then(|p| match p.head {
                    DrumHead::Normal => None,
                    DrumHead::Cross => Some(0xE0A9),
                    DrumHead::CircledCross => Some(0xE0B3),
                    DrumHead::Diamond => Some(0xE0DB),
                    DrumHead::Triangle => Some(0xE0BE),
                }),
            }
        };
        let beat = |beat: &Beat| beat.notes.iter().map(placed).collect();
        let voice = |voice: &Voice| voice.beats.iter().map(beat).collect();
        return (staff.bars.iter())
            .map(|bar| bar.voices.iter().map(voice).collect())
            .collect();
    }

    let middle = middle_line(clef);
    let pitch_of = |note: &Note| {
        let sounding = staff.pitch(note.string, note.fret).unwrap_or(60);
        i32::from(sounding) + i32::from(transposition)
    };
    let beats_of = |bar: usize, voice: usize| -> &[Beat] {
        (staff.bars.get(bar))
            .and_then(|b| b.voices.get(voice))
            .map_or(&[], |v| &v.beats)
    };
    // The pitches a line moves to from a note, or came from: those of the nearest beat of
    // its voice that does not just repeat the note. Nothing if a rest comes first.
    let beside = |pitch: i32, voice: usize, mut bar: usize, mut beat: usize, forward: bool| {
        for _ in 0..16 {
            if forward && beat + 1 < beats_of(bar, voice).len() {
                beat += 1;
            } else if forward {
                (bar, beat) = (bar + 1, 0);
            } else if beat > 0 {
                beat -= 1;
            } else {
                let Some(before) = bar.checked_sub(1) else {
                    break;
                };
                (bar, beat) = (before, beats_of(before, voice).len().saturating_sub(1));
            }
            let Some(found) = beats_of(bar, voice).get(beat) else {
                break;
            };
            let pitches: Vec<i32> = found.notes.iter().map(pitch_of).collect();
            if !pitches.contains(&pitch) {
                return pitches;
            }
        }
        Vec::new()
    };

    let voices = staff.bars.iter().map(|b| b.voices.len()).max().unwrap_or(0);
    // How the last beat of each voice was written, for the notes tied to it.
    let mut held: Vec<Vec<(i32, Name)>> = vec![Vec::new(); voices];
    let mut bars = Vec::with_capacity(staff.bars.len());
    for (b, (bar, master)) in staff.bars.iter().zip(masters).enumerate() {
        let key = master.key_signature;
        let in_key = key_alters(key);
        // Signs already written in this bar, by diatonic step.
        let mut written: Vec<(i32, i8)> = Vec::new();
        let mut spelled_bar = Vec::with_capacity(bar.voices.len());
        for (v, voice) in bar.voices.iter().enumerate() {
            let mut spelled_voice = Vec::with_capacity(voice.beats.len());
            for (i, beat) in voice.beats.iter().enumerate() {
                let pitches: Vec<i32> = beat.notes.iter().map(pitch_of).collect();
                let mut chosen: Vec<Option<Name>> = Vec::with_capacity(pitches.len());
                let mut open: Vec<Open> = Vec::new();
                for (note, &pitch) in beat.notes.iter().zip(&pitches) {
                    // A tied note is the note it is tied to, however that was written.
                    let tied = (held[v].iter().filter(|_| note.tied))
                        .find(|(p, _)| *p == pitch)
                        .map(|(_, name)| *name);
                    chosen.push(tied.or_else(|| match names(pitch, &in_key) {
                        (name, None) => Some(name),
                        (_, Some(_)) if open.iter().any(|o| o.pitch == pitch) => None,
                        (sharp, Some(flat)) => {
                            let towards = |up: bool, down: bool| (up != down).then_some(up);
                            let next = beside(pitch, v, b, i, true);
                            let last = beside(pitch, v, b, i, false);
                            let before = || {
                                written.iter().rev().find_map(|&(step, alter)| match alter {
                                    1 if step == step_of(pitch, sharp) => Some(true),
                                    -1 if step == step_of(pitch, flat) => Some(false),
                                    _ => None,
                                })
                            };
                            let prefers_sharp =
                                towards(next.contains(&(pitch + 1)), next.contains(&(pitch - 1)))
                                    .or_else(|| {
                                        let rose = last.contains(&(pitch - 1));
                                        towards(rose, last.contains(&(pitch + 1)))
                                    })
                                    .or_else(before)
                                    .unwrap_or(key >= 0);
                            open.push(Open {
                                pitch,
                                sharp,
                                flat,
                                prefers_sharp,
                            });
                            None
                        }
                    }));
                }
                let fixed: Vec<(i32, i32)> = (pitches.iter().zip(&chosen))
                    .filter_map(|(&pitch, name)| Some((pitch, step_of(pitch, (*name)?))))
                    .collect();
                let sharps = weigh(&open, &fixed);

                let mut names = Vec::with_capacity(pitches.len());
                let mut spelled_beat = Vec::with_capacity(pitches.len());
                for ((note, &pitch), name) in beat.notes.iter().zip(&pitches).zip(chosen) {
                    let name = name.unwrap_or_else(|| {
                        let found = open.iter().zip(&sharps).find(|(o, _)| o.pitch == pitch);
                        match found {
                            Some((o, true)) => o.sharp,
                            Some((o, false)) => o.flat,
                            None => (0, 0),
                        }
                    });
                    let (step, alter) = name;
                    let diatonic = step_of(pitch, name);
                    let current = (written.iter().rev())
                        .find(|(d, _)| *d == diatonic)
                        .map_or(in_key[step as usize], |(_, a)| *a);
                    let changed = alter != current && !note.tied;
                    if changed {
                        written.push((diatonic, alter));
                    }
                    names.push((pitch, name));
                    spelled_beat.push(Spelled {
                        position: diatonic - middle,
                        accidental: changed.then(|| Accidental::of(alter)),
                        head: None,
                    });
                }
                held[v] = names;
                spelled_voice.push(spelled_beat);
            }
            spelled_bar.push(spelled_voice);
        }
        bars.push(spelled_bar);
    }
    bars
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

/// How far a beam falls from its first stem to its last, given the staff position of the
/// note each stem ends beyond and how far apart the two stems are. A beam leans the way
/// its notes go, a quarter of a space for each step between the first and the last. It
/// stays level when they are on the same step, and when a note between them reaches
/// further towards the beam than either, so that the group has no one direction.
fn beam_fall(ends: &[i32], stem: Stem, run: f64) -> f64 {
    let (Some(&first), Some(&last)) = (ends.first(), ends.last()) else {
        return 0.0;
    };
    let inner = &ends[1..ends.len() - 1];
    let beyond = match stem {
        Stem::Up => inner.iter().any(|&p| p > first.max(last)),
        Stem::Down => inner.iter().any(|&p| p < first.min(last)),
    };
    if beyond {
        return 0.0;
    }
    let steps = f64::from(last - first);
    let slant = (steps.abs() * SPACE / 4.0)
        .min(BEAM_SLANT)
        .min(run * BEAM_SLOPE);
    // Higher on the staff is further up the page.
    -steps.signum() * slant
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Whether to draw here what a tab staff would write on its strings, because the
    /// track has none in this view: slides, and bends with [`Notation::draw_bends`].
    pub string_marks: bool,
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

    /// Draws the bends of the line, each from its notehead up into the lane centred on
    /// `lane`, as the tab staff would from its fret number.
    pub fn draw_bends(&self, lane: f64, items: &mut Vec<Item>) {
        let Some(spelled) = self.spelled.filter(|_| self.string_marks) else {
            return;
        };
        let bars = (spelled.iter().enumerate()).take(self.end).skip(self.first);
        for (bar, voices) in bars {
            for (voice, placed) in voices.iter().enumerate() {
                for (index, (beat, placed)) in self.beats(bar, voice).iter().zip(placed).enumerate()
                {
                    let Some(x) = self.x(bar, voice, index) else {
                        continue;
                    };
                    let room = match self.x(bar, voice, index + 1) {
                        Some(next) if next > x => next - x,
                        _ => self.bars[bar].x + self.bars[bar].w - x,
                    };
                    for (note, placed) in beat.notes.iter().zip(placed) {
                        if let Some(bend) = &note.effects.bend {
                            let y = y_of(placed.position);
                            crate::tab::draw_bend(bend, x, y, HEAD_HALF, lane, room, items);
                        }
                    }
                }
            }
        }
    }

    /// The note a slide from `note` arrives at: the one on the same string in the next
    /// beat. Returns its place on the staff, whether it has a sign before it, and its x
    /// if it is on this line.
    fn slide_target(
        &self,
        bar: usize,
        voice: usize,
        index: usize,
        note: &Note,
    ) -> Option<(i32, bool, Option<f64>)> {
        let (bar, index) = match index + 1 < self.beats(bar, voice).len() {
            true => (bar, index + 1),
            false => (bar + 1, 0),
        };
        let n = (self.beats(bar, voice).get(index)?.notes.iter())
            .position(|other| other.string == note.string)?;
        let placed = self.spelled?.get(bar)?.get(voice)?.get(index)?.get(n)?;
        let x = self.x(bar, voice, index).filter(|_| bar < self.end);
        Some((placed.position, placed.accidental.is_some(), x))
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
                let (ends, stem) = self.draw_group(bar, voice, start..end, forced, items, extent);
                for (slot, y) in edges[start..end].iter_mut().zip(ends) {
                    *slot = Some((y, stem));
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
    /// each stem ends, if they have stems, and the way they point.
    fn draw_group(
        &self,
        bar: usize,
        voice: usize,
        range: std::ops::Range<usize>,
        forced: Option<Stem>,
        items: &mut Vec<Item>,
        extent: &mut Extent,
    ) -> (Vec<f64>, Stem) {
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
        let stemmed = beats[0].duration.value != NoteValue::Whole;
        let stem_x = |chord: &Chord| match stem {
            Stem::Up => chord.x + HEAD_HALF - 0.6,
            Stem::Down => chord.x - HEAD_HALF + 0.6,
        };

        // The stems end on one line, the beam's, which leans the way the notes go. It is
        // placed so that no stem is shorter than it should be and all of them reach the
        // middle line of the staff.
        let xs: Vec<f64> = chords.iter().map(stem_x).collect();
        let ends: Vec<i32> = (chords.iter())
            .map(|chord| match stem {
                Stem::Up => chord.highest(),
                Stem::Down => chord.lowest(),
            })
            .collect();
        let run = xs[xs.len() - 1] - xs[0];
        let slope = match run > 0.0 {
            true => beam_fall(&ends, stem, run) / run,
            false => 0.0,
        };
        let lean = |x: f64| slope * (x - xs[0]);
        let reach = (xs.iter().zip(&ends)).map(|(&x, &end)| match stem {
            Stem::Up => (y_of(end) - length).min(0.0) - lean(x),
            Stem::Down => (y_of(end) + length).max(0.0) - lean(x),
        });
        let start = match stem {
            Stem::Up => reach.fold(f64::INFINITY, f64::min),
            Stem::Down => reach.fold(f64::NEG_INFINITY, f64::max),
        };
        let edge = |x: f64| start + lean(x);

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
                let x = xs[offset];
                let to = edge(x);
                items.push(Item::line(x, from, x, to, 1.2, Color::Ink));
                extent.include(to);
                // Tremolo picking: slashes across the stem.
                let slashes = beat.effects.tremolo.map_or(0, |v| v.flags());
                let middle = (from + to) / 2.0;
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
            return (Vec::new(), stem);
        }
        let stem_ends = xs.iter().map(|&x| edge(x)).collect();

        if chords.len() == 1 {
            // A note on its own carries flags instead of a beam.
            let count = beats[0].duration.value.flags();
            if count > 0 {
                let code = FLAG_8TH_UP + 2 * (count - 1) + u32::from(stem == Stem::Down);
                items.push(left(glyph(xs[0] - 0.6, edge(xs[0]), code)));
            }
            return (stem_ends, stem);
        }
        for level in 1..=deepest {
            let reaches = |i: usize| beats[i].duration.value.flags() >= level;
            let step = f64::from(level - 1) * BEAM_PITCH;
            // Top edge of this level's beam where the stem at `x` meets it.
            let top = |x: f64| match stem {
                Stem::Up => edge(x) + step,
                Stem::Down => edge(x) - BEAM_THICKNESS - step,
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
                let (from, to) = match end - start {
                    // A lone short note gets a stub pointing into the group.
                    1 if start == 0 => (xs[start], xs[start] + 7.0),
                    1 => (xs[start] - 7.0, xs[start]),
                    _ => (xs[start], xs[end - 1]),
                };
                let (from, to) = (from - 0.6, to + 0.6);
                items.push(Item::beam(from, top(from), to, top(to), BEAM_THICKNESS));
                start = end;
            }
        }
        (stem_ends, stem)
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
            if let (Some(slide), true) = (effects.slide, self.string_marks && !slash) {
                // Slides, as the tab has them: a line to the note slid to, or a short one
                // into or out of this note. They start clear of the dots and stop clear
                // of the signs.
                let dots = f64::from(beat.duration.dots.min(2));
                let after = right + if dots > 0.0 { 5.0 + 4.0 * dots } else { 2.0 };
                let before = left_edge
                    - if spelled.accidental.is_some() {
                        13.0
                    } else {
                        2.0
                    };
                let (from, to) = match slide {
                    Slide::Shift | Slide::Legato => {
                        let target = self.slide_target(bar, voice, index, note);
                        match target {
                            Some((position, sign, Some(to_x))) => {
                                // Along the line from one head to the other.
                                let stop = to_x - HEAD_HALF - if sign { 13.0 } else { 2.0 };
                                let stop = stop.max(after + 4.0);
                                let slope = (y_of(position) - y) / (to_x - x).max(1.0);
                                let along = |at: f64| (at, y + slope * (at - x));
                                (along(after), along(stop))
                            }
                            // The note slid to is on the next line, or there is none.
                            Some((position, ..)) => {
                                let lean = f64::from((position - spelled.position).signum());
                                let stop = self.bars[bar].x + self.bars[bar].w - 3.0;
                                ((after, y), (stop.max(after + 4.0), y - 3.0 * lean))
                            }
                            None => ((after, y), (after + 14.0, y)),
                        }
                    }
                    Slide::InFromBelow => ((before - 8.0, y + 4.0), (before, y)),
                    Slide::InFromAbove => ((before - 8.0, y - 4.0), (before, y)),
                    Slide::OutDown => ((after, y), (after + 8.0, y + 4.0)),
                    Slide::OutUp => ((after, y), (after + 8.0, y - 4.0)),
                };
                items.push(Item::line(from.0, from.1, to.0, to.1, 1.1, Color::Ink));
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
        // F sharp, F sharp again, G, F natural, then F natural again, in C major.
        let notes = spelled_in(0, &[(0, 2), (0, 2), (0, 3), (0, 1), (0, 1)]);
        let signs: Vec<_> = notes.iter().map(|n| n.accidental).collect();
        assert_eq!(
            signs,
            [
                Some(Accidental::Sharp),
                None,
                None,
                Some(Accidental::Natural),
                None
            ]
        );
        let on_f = [0, 1, 3, 4].map(|i| notes[i].position);
        assert!(
            on_f.iter()
                .all(|p| *p == on_f[0] && *p + 1 == notes[2].position)
        );

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

    /// The notes of one bar in C major as letters, each beat a chord of `(string, fret)`.
    fn letters(beats: &[&[(u8, u8)]], tied: &[usize]) -> Vec<String> {
        let mut staff = staff_of(&[]);
        staff.bars[0].voices[0].beats = (beats.iter().enumerate())
            .map(|(i, notes)| {
                let mut notes: Vec<Note> = notes.iter().map(|&(s, f)| Note::new(s, f)).collect();
                notes.iter_mut().for_each(|n| n.tied = tied.contains(&i));
                Beat::new(Duration::new(NoteValue::Eighth), notes)
            })
            .collect();
        let master = MasterBar::new(TimeSignature::COMMON);
        let bars = spell(&staff, Clef::Treble, 12, &[master], &[]);
        // Steps are counted from the B on the middle line.
        let letter = |n: &Spelled| {
            let name = b"CDEFGAB"[(n.position + 6).rem_euclid(7) as usize] as char;
            match n.accidental {
                Some(Accidental::Sharp) => format!("{name}#"),
                Some(Accidental::Flat) => format!("{name}b"),
                Some(Accidental::Natural) => format!("{name}n"),
                None => name.to_string(),
            }
        };
        (bars[0][0].iter())
            .map(|beat| beat.iter().map(letter).collect::<Vec<_>>().join(" "))
            .collect()
    }

    #[test]
    fn accidentals_follow_the_line_and_the_chord() {
        // Up the B string by semitones and down again: sharps going up, flats coming down.
        let line: Vec<&[(u8, u8)]> = vec![
            &[(1, 1)],
            &[(1, 2)],
            &[(1, 3)],
            &[(1, 4)],
            &[(1, 5)],
            &[(1, 4)],
            &[(1, 3)],
            &[(1, 2)],
            &[(1, 1)],
        ];
        assert_eq!(
            letters(&line, &[]),
            ["C", "C#", "D", "D#", "E", "Eb", "Dn", "Db", "Cn"]
        );
        // A note that is repeated before it moves is spelled by where it goes in the end,
        // and one that ends the line by where it came from.
        assert_eq!(
            letters(&[&[(2, 3)], &[(2, 3)], &[(2, 2)], &[(2, 1)]], &[]),
            ["Bb", "B", "A", "Ab"]
        );
        // A leap says nothing, so the key decides, and the note stays as it was written.
        assert_eq!(
            letters(&[&[(2, 3)], &[(0, 0)], &[(2, 3)], &[(1, 3)]], &[]),
            ["A#", "E", "A", "D"]
        );

        // E major, then A flat major, on the top three strings: the same pitch is a G
        // sharp over an E and an A flat under a C, and the chord outweighs the line.
        let chords: Vec<&[(u8, u8)]> = vec![&[(0, 0), (1, 0), (2, 1)], &[(0, 4), (1, 1), (2, 8)]];
        assert_eq!(letters(&chords, &[]), ["E B G#", "Ab C Eb"]);
        // Two notes a semitone apart never share a line: C sharp against a C is a D flat.
        assert_eq!(letters(&[&[(1, 1), (2, 6)]], &[]), ["C Db"]);
        // A tied note keeps the name of the note it is tied to, and needs no sign: a G
        // sharp held over from an E is still one when a C comes in under it.
        assert_eq!(
            letters(&[&[(0, 4), (1, 5)], &[(0, 4), (1, 1)]], &[1]),
            ["G# E", "G C"]
        );
    }

    #[test]
    fn sharp_keys_have_their_own_names_for_the_white_keys() {
        // F sharp major: the F on the top string is the key's E sharp, written without a
        // sign on the E's space, a step under the F sharp. The open E is what needs one.
        let notes = spelled_in(6, &[(0, 1), (0, 2), (0, 0)]);
        assert_eq!(notes[1].position, notes[0].position + 1);
        assert_eq!(notes[2].position, notes[0].position);
        let signs: Vec<_> = notes.iter().map(|n| n.accidental).collect();
        assert_eq!(signs, [None, None, Some(Accidental::Natural)]);
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
