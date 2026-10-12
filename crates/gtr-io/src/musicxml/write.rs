//! MusicXML export: a `score-partwise` document of version 4.0, a part for each track.
//!
//! A track played on strings is written as two staves, the way programs for the guitar
//! write one: standard notation with everything that is written around the notes, and
//! under it tablature, whose notes say their strings and frets. A piano's two staves
//! are one part. What MusicXML has an element for is written with it; a few
//! guitar techniques it has none for are written as `other-technical` in plain words.

use std::fmt::Write as _;

use gtr_model::{
    Accent, Beat, Bend, Clef, DrumHead, Duration, Dynamic, Feel, Hairpin, Harmonic, MasterBar,
    Note, NoteValue, PickStroke, Rational, Score, Slap, Slide, Staff, Strum, Target, Track,
    Vibrato,
};

use crate::Skipped;
use crate::util::{esc, spell};

/// What `other-technical` says for the techniques MusicXML has no element for.
pub(super) const PALM_MUTE: &str = "palm mute";
pub(super) const SLAP: &str = "slap";
pub(super) const POP: &str = "pop";
pub(super) const WIDE_VIBRATO: &str = "wide vibrato";
pub(super) const TRILL_TO: &str = "trill to fret ";

/// The name of a kind of harmonic that is neither natural nor plainly artificial.
pub(super) fn harmonic_name(harmonic: Harmonic) -> Option<&'static str> {
    match harmonic {
        Harmonic::Pinch => Some("pinch harmonic"),
        Harmonic::Tapped => Some("tapped harmonic"),
        Harmonic::Semi => Some("semi harmonic"),
        Harmonic::Natural | Harmonic::Artificial => None,
    }
}

pub(super) const DYNAMICS: [(Dynamic, &str); 8] = [
    (Dynamic::Ppp, "ppp"),
    (Dynamic::Pp, "pp"),
    (Dynamic::P, "p"),
    (Dynamic::Mp, "mp"),
    (Dynamic::Mf, "mf"),
    (Dynamic::F, "f"),
    (Dynamic::Ff, "ff"),
    (Dynamic::Fff, "fff"),
];

pub(super) const TYPES: [(NoteValue, &str); 9] = [
    (NoteValue::Whole, "whole"),
    (NoteValue::Half, "half"),
    (NoteValue::Quarter, "quarter"),
    (NoteValue::Eighth, "eighth"),
    (NoteValue::Sixteenth, "16th"),
    (NoteValue::ThirtySecond, "32nd"),
    (NoteValue::SixtyFourth, "64th"),
    (NoteValue::HundredTwentyEighth, "128th"),
    (NoteValue::TwoHundredFiftySixth, "256th"),
];

/// How a feel is said: the shares of a pair of notes the first and the second take, and
/// whether the notes are eighths.
pub(super) const FEELS: [(Feel, u8, u8, bool); 6] = [
    (Feel::SwingEighths, 2, 1, true),
    (Feel::SwingSixteenths, 2, 1, false),
    (Feel::DottedEighths, 3, 1, true),
    (Feel::DottedSixteenths, 3, 1, false),
    (Feel::ScottishEighths, 1, 3, true),
    (Feel::ScottishSixteenths, 1, 3, false),
];

/// The kinds of chord MusicXML names, by what follows the root in the chord's name.
pub(super) const KINDS: [(&str, &str); 22] = [
    ("", "major"),
    ("m", "minor"),
    ("min", "minor"),
    ("-", "minor"),
    ("7", "dominant"),
    ("maj7", "major-seventh"),
    ("M7", "major-seventh"),
    ("m7", "minor-seventh"),
    ("min7", "minor-seventh"),
    ("dim", "diminished"),
    ("dim7", "diminished-seventh"),
    ("m7b5", "half-diminished"),
    ("aug", "augmented"),
    ("+", "augmented"),
    ("sus4", "suspended-fourth"),
    ("sus2", "suspended-second"),
    ("5", "power"),
    ("6", "major-sixth"),
    ("m6", "minor-sixth"),
    ("9", "dominant-ninth"),
    ("maj9", "major-ninth"),
    ("m9", "minor-ninth"),
];

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

/// How many parts a quarter note is counted in, so that every length in the score is a
/// whole number of them.
fn divisions(score: &Score) -> i64 {
    let mut divisions = 1i64;
    let mut take = |length: Rational| {
        let denominator = *(length * 4).denom();
        let next = divisions / gcd(divisions, denominator) * denominator;
        // A score of outlandish tuplets is rounded rather than counted in millions.
        if next <= 1_000_000 {
            divisions = next;
        }
    };
    for master in &score.master_bars {
        master.tempo_changes.iter().for_each(|c| take(c.position));
    }
    let beats = (score.tracks.iter().flat_map(|t| &t.staves))
        .flat_map(|s| &s.bars)
        .flat_map(|b| &b.voices)
        .flat_map(|v| &v.beats);
    for beat in beats {
        take(beat.duration.length());
    }
    divisions
}

struct Writer<'a> {
    score: &'a Score,
    divisions: i64,
    out: String,
}

impl Writer<'_> {
    fn ticks(&self, length: Rational) -> i64 {
        (length * 4 * self.divisions).round().to_integer()
    }
}

pub(super) fn write(score: &Score, skipped: &mut Skipped) -> String {
    let mut w = Writer {
        score,
        divisions: divisions(score),
        out: String::new(),
    };
    let info = &score.info;
    w.out.push_str(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n\
         <!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \
         \"http://www.musicxml.org/dtds/partwise.dtd\">\n<score-partwise version=\"4.0\">\n",
    );
    if !info.title.is_empty() || !info.album.is_empty() {
        // The work is the album, if there is one, and the movement the piece.
        let _ = writeln!(
            w.out,
            "<work><work-title>{}</work-title></work>",
            esc(&info.album)
        );
    }
    let _ = writeln!(
        w.out,
        "<movement-title>{}</movement-title>",
        esc(&info.title)
    );
    w.out.push_str("<identification>\n");
    for (kind, who) in [
        ("composer", &info.music),
        ("lyricist", &info.words),
        ("artist", &info.artist),
        ("transcriber", &info.transcriber),
        ("subtitle", &info.subtitle),
    ] {
        if !who.is_empty() {
            let _ = writeln!(w.out, "<creator type=\"{kind}\">{}</creator>", esc(who));
        }
    }
    if !info.copyright.is_empty() {
        let _ = writeln!(w.out, "<rights>{}</rights>", esc(&info.copyright));
    }
    w.out
        .push_str("<encoding><software>gtrTab</software></encoding>\n</identification>\n");

    w.out.push_str("<part-list>\n");
    for (index, track) in score.tracks.iter().enumerate() {
        skipped.add_if(track.mix.mute || track.mix.solo, "mute and solo");
        let id = index + 1;
        let _ = writeln!(
            w.out,
            "<score-part id=\"P{id}\"><part-name>{}</part-name>",
            esc(&track.name)
        );
        // A drum kit is an instrument for every piece that is played.
        let pieces = pieces(track);
        let volume = track.mix.volume;
        let pan = f64::from(track.mix.pan) * 0.9;
        if track.instrument.is_percussion() {
            let kit = track.drum_kit();
            for number in &pieces {
                let name = (kit.iter().find(|p| p.number == *number))
                    .map_or_else(|| format!("Percussion {number}"), |p| p.name.clone());
                let _ = writeln!(
                    w.out,
                    "<score-instrument id=\"P{id}-I{}\"><instrument-name>{}</instrument-name>\
                     </score-instrument>",
                    u32::from(*number) + 1,
                    esc(&name)
                );
            }
            for number in &pieces {
                let _ = writeln!(
                    w.out,
                    "<midi-instrument id=\"P{id}-I{n}\"><midi-channel>10</midi-channel>\
                     <midi-program>1</midi-program><midi-unpitched>{n}</midi-unpitched>\
                     <volume>{volume}</volume><pan>{pan:.1}</pan></midi-instrument>",
                    n = u32::from(*number) + 1
                );
            }
        } else {
            // Channels are counted from one, and the tenth is percussion's.
            let channel = match index % 15 {
                channel if channel >= 9 => channel + 2,
                channel => channel + 1,
            };
            let _ = writeln!(
                w.out,
                "<score-instrument id=\"P{id}-I1\"><instrument-name>{}</instrument-name>\
                 </score-instrument>\n\
                 <midi-instrument id=\"P{id}-I1\"><midi-channel>{channel}</midi-channel>\
                 <midi-program>{}</midi-program><volume>{volume}</volume><pan>{pan:.1}</pan>\
                 </midi-instrument>",
                track.instrument.name(),
                u32::from(track.instrument.midi_program()) + 1
            );
        }
        w.out.push_str("</score-part>\n");
    }
    w.out.push_str("</part-list>\n");

    for index in 0..score.tracks.len() {
        part(&mut w, index, skipped);
    }
    w.out.push_str("</score-partwise>\n");
    w.out
}

/// The pieces of the kit a drum track plays, lowest number first.
fn pieces(track: &Track) -> Vec<u8> {
    let mut pieces: Vec<u8> = (track.staves.iter().flat_map(|s| &s.bars))
        .flat_map(|b| &b.voices)
        .flat_map(|v| &v.beats)
        .flat_map(|b| &b.notes)
        .map(|n| n.fret)
        .collect();
    pieces.sort_unstable();
    pieces.dedup();
    pieces
}

/// The beat before or after one in its voice, in its bar or the nearest that has any.
fn neighbour(staff: &Staff, bar: usize, voice: usize, beat: usize, forward: bool) -> Option<&Beat> {
    let here = &staff.bars[bar].voices[voice].beats;
    match forward {
        true => here.get(beat + 1).or_else(|| {
            (staff.bars[bar + 1..].iter())
                .filter_map(|bar| bar.voices.get(voice))
                .find_map(|voice| voice.beats.first())
        }),
        false => match beat {
            0 => (staff.bars[..bar].iter().rev())
                .filter_map(|bar| bar.voices.get(voice))
                .find_map(|voice| voice.beats.last()),
            _ => here.get(beat - 1),
        },
    }
}

/// For each beat of a voice, whether a tuplet's bracket starts on it and whether one
/// ends on it. A bracket ends when its notes add up to a whole group, or when the next
/// note is not of the same tuplet.
fn brackets(beats: &[Beat]) -> Vec<(bool, bool)> {
    let mut out = vec![(false, false); beats.len()];
    let mut index = 0;
    while index < beats.len() {
        let Some(tuplet) = beats[index].duration.tuplet else {
            index += 1;
            continue;
        };
        let nested = beats[index].duration.nested;
        let actual =
            i64::from(tuplet.actual.get()) * nested.map_or(1, |n| i64::from(n.actual.get()));
        out[index].0 = true;
        let mut written = Rational::from_integer(0);
        let mut shortest = Rational::from_integer(1);
        loop {
            let duration = beats[index].duration;
            let plain = Duration {
                tuplet: None,
                nested: None,
                ..duration
            };
            written += plain.length();
            shortest = shortest.min(duration.value.length());
            let groups = written / (shortest * actual);
            let whole = groups.is_integer();
            let last = (beats.get(index + 1)).is_none_or(|next| {
                next.duration.tuplet != Some(tuplet) || next.duration.nested != nested
            });
            index += 1;
            if whole || last {
                out[index - 1].1 = true;
                break;
            }
        }
    }
    out
}

fn clef_xml(clef: Clef, number: usize, octave_down: bool) -> String {
    let (sign, line) = match clef {
        Clef::Treble => ("G", 2),
        Clef::Bass => ("F", 4),
        Clef::Percussion => ("percussion", 2),
    };
    let change = match octave_down {
        true => "<clef-octave-change>-1</clef-octave-change>",
        false => "",
    };
    format!("<clef number=\"{number}\"><sign>{sign}</sign><line>{line}</line>{change}</clef>\n")
}

/// A direction of words, placed above the staff, with what it does to playback.
fn words(text: &str, sound: &str, staff: usize) -> String {
    format!(
        "<direction placement=\"above\"><direction-type><words>{}</words></direction-type>\
         <staff>{staff}</staff>{sound}</direction>\n",
        esc(text)
    )
}

fn tempo_xml(tempo: u32, offset: Option<i64>) -> String {
    let offset = offset.map_or(String::new(), |ticks| format!("<offset>{ticks}</offset>"));
    format!(
        "<direction placement=\"above\"><direction-type><metronome><beat-unit>quarter</beat-unit>\
         <per-minute>{tempo}</per-minute></metronome></direction-type>{offset}<staff>1</staff>\
         <sound tempo=\"{tempo}\"/></direction>\n"
    )
}

/// What is written over the start of a bar in the first part: its section, its signs, a
/// new tempo and a new feel.
fn bar_marks(master: &MasterBar, start: Option<u32>, skipped: &mut Skipped) -> String {
    let mut out = String::new();
    if let Some(section) = &master.section {
        let _ = writeln!(
            out,
            "<direction placement=\"above\"><direction-type><rehearsal>{}</rehearsal>\
             </direction-type><staff>1</staff></direction>",
            esc(section)
        );
    }
    for target in &master.targets {
        let (mark, sound) = match target {
            Target::Segno => ("<segno/>", "segno=\"segno\""),
            Target::SegnoSegno => ("<segno/>", "segno=\"segnosegno\""),
            Target::Coda => ("<coda/>", "coda=\"coda\""),
            Target::DoubleCoda => ("<coda/>", "coda=\"doublecoda\""),
            Target::Fine => ("<words>Fine</words>", "fine=\"yes\""),
        };
        let _ = writeln!(
            out,
            "<direction placement=\"above\"><direction-type>{mark}</direction-type>\
             <staff>1</staff><sound {sound}/></direction>"
        );
    }
    // The first bar says the tempo the piece starts at, if it has none of its own.
    if let Some(tempo) = master.tempo.or(start) {
        out.push_str(&tempo_xml(tempo, None));
    }
    skipped.add_if(
        master.tempo_gradual || master.tempo_changes.iter().any(|c| c.gradual),
        "that a tempo changes by degrees",
    );
    skipped.add_if(master.timer, "timers");
    if let Some(feel) = master.feel {
        let swing = match FEELS.iter().find(|(named, ..)| *named == feel) {
            Some((_, first, second, eighths)) => format!(
                "<swing><first>{first}</first><second>{second}</second><swing-type>{}</swing-type>\
                 </swing>",
                if *eighths { "eighth" } else { "16th" }
            ),
            None => "<swing><straight/></swing>".to_owned(),
        };
        out.push_str(&words(feel.label(), &format!("<sound>{swing}</sound>"), 1));
    }
    out
}

/// What is written at the end of a bar in the first part: where to go on from.
fn bar_jumps(master: &MasterBar) -> String {
    let mut out = String::new();
    for jump in &master.jumps {
        let sign = |target: Option<Target>| match target {
            Some(Target::SegnoSegno) => "segnosegno",
            Some(Target::DoubleCoda) => "doublecoda",
            Some(Target::Coda) => "coda",
            _ => "segno",
        };
        let sound = match (jump.onward(), jump.back_to()) {
            (Some(coda), _) => format!("tocoda=\"{}\"", sign(Some(coda))),
            (_, Some(segno)) => format!("dalsegno=\"{}\"", sign(Some(segno))),
            _ => "dacapo=\"yes\"".to_owned(),
        };
        out.push_str(&words(jump.label(), &format!("<sound {sound}/>"), 1));
    }
    out
}

fn part(w: &mut Writer, index: usize, skipped: &mut Skipped) {
    let score = w.score;
    let track = &score.tracks[index];
    let drums = track.instrument.is_percussion();
    let strings = !drums && !track.staves[0].is_pitched();
    let _ = writeln!(w.out, "<part id=\"P{}\">", index + 1);
    // The hairpin open in each voice of each staff.
    let mut wedges = vec![[None::<Hairpin>; 4]; track.staves.len()];
    let mut previous: Option<&MasterBar> = None;
    for (bar, master) in score.master_bars.iter().enumerate() {
        let _ = writeln!(w.out, "<measure number=\"{}\">", bar + 1);

        // What the staves are, at the start and wherever the key or the time changes.
        let key = previous.is_none_or(|p| p.key_signature != master.key_signature);
        let time = previous.is_none_or(|p| p.time_signature != master.time_signature);
        if key || time {
            w.out.push_str("<attributes>\n");
            if bar == 0 {
                let _ = writeln!(w.out, "<divisions>{}</divisions>", w.divisions);
            }
            if key {
                let _ = writeln!(
                    w.out,
                    "<key><fifths>{}</fifths></key>",
                    master.key_signature
                );
            }
            if time {
                let _ = writeln!(
                    w.out,
                    "<time><beats>{}</beats><beat-type>{}</beat-type></time>",
                    master.time_signature.numerator, master.time_signature.denominator
                );
            }
            if bar == 0 {
                let staves = track.staves.len() + usize::from(strings);
                if staves > 1 {
                    let _ = writeln!(w.out, "<staves>{staves}</staves>");
                }
                // Guitars and basses are written an octave above where they sound.
                let down = track.instrument.written_transposition() == 12;
                for staff in 0..track.staves.len() {
                    w.out
                        .push_str(&clef_xml(track.instrument.clef(staff), staff + 1, down));
                }
                if strings {
                    let staff = &track.staves[0];
                    // The tablature: a line for each string, numbered from the lowest,
                    // and the capo.
                    let _ = writeln!(
                        w.out,
                        "<clef number=\"2\"><sign>TAB</sign><line>5</line></clef>\n\
                         <staff-details number=\"2\"><staff-lines>{}</staff-lines>",
                        staff.tuning.strings.len()
                    );
                    for (line, pitch) in staff.tuning.strings.iter().rev().enumerate() {
                        let (step, alter, octave) = spell(*pitch, false);
                        let alter = match alter {
                            0 => String::new(),
                            alter => format!("<tuning-alter>{alter}</tuning-alter>"),
                        };
                        let _ = writeln!(
                            w.out,
                            "<staff-tuning line=\"{}\"><tuning-step>{step}</tuning-step>{alter}\
                             <tuning-octave>{octave}</tuning-octave></staff-tuning>",
                            line + 1
                        );
                    }
                    if staff.capo > 0 {
                        let _ = writeln!(w.out, "<capo>{}</capo>", staff.capo);
                    }
                    w.out.push_str("</staff-details>\n");
                }
            }
            w.out.push_str("</attributes>\n");
        }

        // A repeat that starts here, and the passes this bar is played on.
        let starts_ending = master.alternate_endings != 0
            && previous.is_none_or(|p| p.alternate_endings != master.alternate_endings);
        let passes: Vec<String> = (0..8)
            .filter(|pass| master.alternate_endings & (1 << pass) != 0)
            .map(|pass| (pass + 1).to_string())
            .collect();
        if master.repeat_start || starts_ending {
            w.out.push_str("<barline location=\"left\">");
            if master.repeat_start {
                w.out.push_str("<bar-style>heavy-light</bar-style>");
            }
            if starts_ending {
                let _ = write!(
                    w.out,
                    "<ending number=\"{}\" type=\"start\"/>",
                    passes.join(",")
                );
            }
            if master.repeat_start {
                w.out.push_str("<repeat direction=\"forward\"/>");
            }
            w.out.push_str("</barline>\n");
        }
        if index == 0 {
            let start = (bar == 0).then_some(score.tempo);
            w.out.push_str(&bar_marks(master, start, skipped));
        }

        // The tempo changes inside the bar, each to be written before the beat it is on.
        let mut changes: Vec<(Rational, u32)> = match index {
            0 => (master.tempo_changes.iter())
                .map(|c| (c.position, c.tempo))
                .collect(),
            _ => Vec::new(),
        };

        let mut position = 0i64;
        let mut written = false;
        // A voice on strings is written twice: as notation, and again as tablature.
        let twice: &[bool] = match strings {
            true => &[false, true],
            false => &[false],
        };
        for (staff_index, staff) in track.staves.iter().enumerate() {
            let voices = staff.bars[bar].voices.iter().enumerate().take(4);
            for ((voice_index, voice), tab) in
                voices.flat_map(|voice| twice.iter().map(move |tab| (voice, *tab)))
            {
                if voice.beats.is_empty() {
                    continue;
                }
                if position > 0 {
                    let _ = writeln!(w.out, "<backup><duration>{position}</duration></backup>");
                    position = 0;
                }
                written = true;
                let brackets = brackets(&voice.beats);
                let mut time = Rational::from_integer(0);
                for (beat_index, beat) in voice.beats.iter().enumerate() {
                    if staff_index == 0 && voice_index == 0 && !tab {
                        // Changes that fall between two beats say how far on they are.
                        changes.retain(|(at, tempo)| {
                            let due = *at < time + beat.duration.length();
                            if due {
                                let offset = (*at != time).then(|| w.ticks(*at - time));
                                w.out.push_str(&tempo_xml(*tempo, offset));
                            }
                            !due
                        });
                    }
                    let at = At {
                        part: index,
                        track,
                        staff,
                        staff_index,
                        tab,
                        bar,
                        voice: voice_index,
                        beat: beat_index,
                        flats: master.key_signature < 0,
                    };
                    let wedge = &mut wedges[staff_index][voice_index];
                    let ticks = w.ticks(beat.duration.length());
                    beat_xml(
                        &mut w.out,
                        &at,
                        beat,
                        brackets[beat_index],
                        ticks,
                        wedge,
                        skipped,
                    );
                    position += ticks;
                    time += beat.duration.length();
                }
            }
        }
        if !written {
            // A bar with nothing in it is a bar's rest.
            let length = w.ticks(master.time_signature.bar_length());
            let _ = writeln!(
                w.out,
                "<note><rest measure=\"yes\"/><duration>{length}</duration><voice>1</voice>\
                 <staff>1</staff></note>"
            );
        }
        for (_, tempo) in changes {
            w.out.push_str(&tempo_xml(tempo, None));
        }
        // Hairpins end with the piece if they have not ended before.
        if bar + 1 == score.master_bars.len() {
            for (staff_index, staff) in wedges.iter_mut().enumerate() {
                for (voice_index, wedge) in
                    staff.iter_mut().enumerate().filter(|(_, w)| w.is_some())
                {
                    *wedge = None;
                    let _ = writeln!(
                        w.out,
                        "<direction><direction-type><wedge type=\"stop\"/></direction-type>\
                         <voice>{}</voice><staff>{}</staff></direction>",
                        staff_index * 4 + voice_index + 1,
                        staff_index + 1
                    );
                }
            }
        }
        if index == 0 {
            w.out.push_str(&bar_jumps(master));
        }

        // An ending that ends here, and a repeat that does.
        let next = score.master_bars.get(bar + 1);
        let ends_ending = master.alternate_endings != 0
            && next.is_none_or(|n| n.alternate_endings != master.alternate_endings);
        if master.repeat_count > 0 || ends_ending {
            w.out.push_str("<barline location=\"right\">");
            if master.repeat_count > 0 {
                w.out.push_str("<bar-style>light-heavy</bar-style>");
            }
            if ends_ending {
                let _ = write!(
                    w.out,
                    "<ending number=\"{}\" type=\"{}\"/>",
                    passes.join(","),
                    if master.repeat_count > 0 {
                        "stop"
                    } else {
                        "discontinue"
                    }
                );
            }
            if master.repeat_count > 0 {
                let _ = write!(
                    w.out,
                    "<repeat direction=\"backward\" times=\"{}\"/>",
                    master.repeat_count
                );
            }
            w.out.push_str("</barline>\n");
        }
        w.out.push_str("</measure>\n");
        previous = Some(master);
    }
    w.out.push_str("</part>\n");
}

/// Where a beat is.
struct At<'a> {
    /// Which part of the score the track is.
    part: usize,
    track: &'a Track,
    staff: &'a Staff,
    staff_index: usize,
    /// Whether the beat is being written on the staff of tablature under its notation.
    tab: bool,
    bar: usize,
    voice: usize,
    beat: usize,
    /// Whether the key is one of flats, which decides how a note between two is named.
    flats: bool,
}

/// A note by its letter and how far it is raised or lowered.
type Named = (char, i8);

/// A chord's name as a root, what follows it, and a bass note if it names one.
fn chord_parts(name: &str) -> Option<(Named, &str, Option<Named>)> {
    fn note(text: &str) -> Option<(Named, &str)> {
        let mut chars = text.chars();
        let step = chars.next().filter(|c| ('A'..='G').contains(c))?;
        let rest = chars.as_str();
        match rest.chars().next() {
            Some('#') | Some('♯') => Some(((step, 1), &rest[rest.chars().next()?.len_utf8()..])),
            Some('b') | Some('♭') => Some(((step, -1), &rest[rest.chars().next()?.len_utf8()..])),
            _ => Some(((step, 0), rest)),
        }
    }
    let (root, rest) = note(name)?;
    match rest.rsplit_once('/') {
        Some((kind, bass)) => match note(bass) {
            Some((bass, "")) => Some((root, kind, Some(bass))),
            _ => Some((root, rest, None)),
        },
        None => Some((root, rest, None)),
    }
}

fn harmony_xml(beat: &Beat, staff: usize) -> String {
    let effects = &beat.effects;
    let name = effects.chord.as_deref().unwrap_or("");
    let mut out = String::from("<harmony>");
    let alter = |alter: i8, tag: &str| match alter {
        0 => String::new(),
        alter => format!("<{tag}>{alter}</{tag}>"),
    };
    match chord_parts(name) {
        Some(((step, change), kind, bass)) => {
            let value = (KINDS.iter().find(|(text, _)| *text == kind)).map_or("other", |k| k.1);
            let _ = write!(
                out,
                "<root><root-step>{step}</root-step>{}</root><kind text=\"{}\">{value}</kind>",
                alter(change, "root-alter"),
                esc(kind)
            );
            if let Some((step, change)) = bass {
                let _ = write!(
                    out,
                    "<bass><bass-step>{step}</bass-step>{}</bass>",
                    alter(change, "bass-alter")
                );
            }
        }
        // A name that is not a chord's, or a diagram without a name, is written out as
        // it stands.
        None => {
            let _ = write!(
                out,
                "<root><root-step>C</root-step></root><kind text=\"{}\">none</kind>",
                esc(name)
            );
        }
    }
    if let Some(diagram) = &effects.diagram {
        let base = diagram.base_fret();
        let count = diagram.frets.len();
        let _ = write!(
            out,
            "<frame><frame-strings>{count}</frame-strings><frame-frets>5</frame-frets>\
             <first-fret>{base}</first-fret>"
        );
        // Strings are numbered from the highest, and listed from the lowest.
        for (string, fret) in diagram.frets.iter().enumerate().rev() {
            if *fret >= 0 {
                let _ = write!(
                    out,
                    "<frame-note><string>{}</string><fret>{fret}</fret></frame-note>",
                    string + 1
                );
            }
        }
        out.push_str("</frame>");
    }
    let _ = writeln!(out, "<staff>{staff}</staff></harmony>");
    out
}

/// The steps of a bend as MusicXML says them: the level each reaches in semitones, with
/// whether it is there before the note is struck and whether it is let down to.
fn bend_xml(bend: &Bend) -> String {
    let mut out = String::new();
    let mut level = 0;
    for (index, point) in bend.points.iter().enumerate() {
        let kind = match (index, point.value.cmp(&level)) {
            (0, std::cmp::Ordering::Greater) => "<pre-bend/>",
            (_, std::cmp::Ordering::Greater) => "",
            (_, std::cmp::Ordering::Less) => "<release/>",
            _ => continue,
        };
        level = point.value;
        let _ = write!(
            out,
            "<bend><bend-alter>{}</bend-alter>{kind}</bend>",
            f64::from(level) / 2.0
        );
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn beat_xml(
    out: &mut String,
    at: &At,
    beat: &Beat,
    (starts, stops): (bool, bool),
    ticks: i64,
    wedge: &mut Option<Hairpin>,
    skipped: &mut Skipped,
) {
    let effects = &beat.effects;
    let staff_number = at.staff_index + 1 + usize::from(at.tab);
    let voice_number = (staff_number - 1) * 4 + at.voice + 1;
    let drums = at.track.instrument.is_percussion();
    skipped.add_if(effects.whammy.is_some(), "whammy bar");
    skipped.add_if(effects.fade.is_some(), "fades");
    skipped.add_if(
        effects.volume.is_some() || effects.pan.is_some(),
        "changes of volume and pan",
    );
    skipped.add_if(
        matches!(
            effects.strum,
            Some(Strum::BrushDown | Strum::BrushUp | Strum::Rasgueado)
        ),
        "strums",
    );

    // What is written over or under the beat comes before it.
    if !at.tab && *wedge != effects.hairpin {
        if wedge.is_some() {
            let _ = writeln!(
                out,
                "<direction><direction-type><wedge type=\"stop\"/></direction-type>\
                 <voice>{voice_number}</voice><staff>{staff_number}</staff></direction>"
            );
        }
        if let Some(hairpin) = effects.hairpin {
            let _ = writeln!(
                out,
                "<direction placement=\"below\"><direction-type><wedge type=\"{}\"/>\
                 </direction-type><voice>{voice_number}</voice><staff>{staff_number}</staff>\
                 </direction>",
                match hairpin {
                    Hairpin::Crescendo => "crescendo",
                    Hairpin::Decrescendo => "diminuendo",
                }
            );
        }
        *wedge = effects.hairpin;
    }
    if !at.tab && (effects.chord.is_some() || effects.diagram.is_some()) {
        out.push_str(&harmony_xml(beat, staff_number));
    }
    if let Some(text) = effects.text.as_ref().filter(|_| !at.tab) {
        out.push_str(&words(text, "", staff_number));
    }
    if let Some(dynamic) = effects.dynamic.filter(|_| !at.tab) {
        let name = DYNAMICS
            .iter()
            .find(|(d, _)| *d == dynamic)
            .map_or("f", |d| d.1);
        let _ = writeln!(
            out,
            "<direction placement=\"below\"><direction-type><dynamics><{name}/></dynamics>\
             </direction-type><staff>{staff_number}</staff></direction>"
        );
    }

    let previous = neighbour(at.staff, at.bar, at.voice, at.beat, false);
    let next = neighbour(at.staff, at.bar, at.voice, at.beat, true);
    let duration = beat.duration;
    let kind = TYPES
        .iter()
        .find(|(v, _)| *v == duration.value)
        .map_or("quarter", |t| t.1);
    let mut rhythm = format!(
        "<type>{kind}</type>{}",
        "<dot/>".repeat(duration.dots.into())
    );
    if let Some(tuplet) = duration.tuplet {
        let nested = duration.nested;
        let _ = write!(
            rhythm,
            "<time-modification><actual-notes>{}</actual-notes><normal-notes>{}</normal-notes>\
             </time-modification>",
            u32::from(tuplet.actual.get()) * nested.map_or(1, |n| u32::from(n.actual.get())),
            u32::from(tuplet.normal.get()) * nested.map_or(1, |n| u32::from(n.normal.get())),
        );
    }
    let pitch_xml = |pitch: u8| {
        let (step, alter, octave) = spell(pitch, at.flats);
        let alter = match alter {
            0 => String::new(),
            alter => format!("<alter>{alter}</alter>"),
        };
        format!("<pitch><step>{step}</step>{alter}<octave>{octave}</octave></pitch>")
    };
    let kit = match drums {
        true => at.track.drum_kit(),
        false => Vec::new(),
    };
    // A piece of the kit is written where its line or space would be in the treble clef.
    let drum_xml = |number: u8| {
        let position = kit
            .iter()
            .find(|p| p.number == number)
            .map_or(0, |p| i32::from(p.position));
        let step = 34 + position;
        format!(
            "<unpitched><display-step>{}</display-step><display-octave>{}</display-octave>\
             </unpitched>",
            ['C', 'D', 'E', 'F', 'G', 'A', 'B'][step.rem_euclid(7) as usize],
            step.div_euclid(7)
        )
    };

    // Grace notes come before the notes they lead into.
    for note in &beat.notes {
        let Some(grace) = &note.effects.grace else {
            continue;
        };
        for ornament in &grace.notes {
            let pitch = match drums {
                true => drum_xml(ornament.fret),
                false => (at.staff.pitch(ornament.string, ornament.fret))
                    .map_or(String::new(), pitch_xml),
            };
            // The tablature says where the note is played. That a grace note is hammered
            // is said on both staves, since it has no note of its own to be given to.
            let hammered = match grace.hammer && !at.staff.is_pitched() && !drums {
                true => "<hammer-on type=\"start\">H</hammer-on>",
                false => "",
            };
            let place = match (at.tab, hammered) {
                (true, _) => format!(
                    "<notations><technical><string>{}</string><fret>{}</fret>{hammered}\
                     </technical></notations>",
                    ornament.string + 1,
                    ornament.fret
                ),
                (false, "") => String::new(),
                (false, _) => format!("<notations><technical>{hammered}</technical></notations>"),
            };
            let _ = writeln!(
                out,
                "<note><grace{}/>{pitch}<voice>{voice_number}</voice><type>16th</type>\
                 <staff>{staff_number}</staff>{place}</note>",
                if grace.on_beat { "" } else { " slash=\"yes\"" }
            );
        }
    }

    if beat.notes.is_empty() {
        let fermata = match effects.fermata && !at.tab {
            true => "<notations><fermata/></notations>",
            false => "",
        };
        let _ = writeln!(
            out,
            "<note><rest/><duration>{ticks}</duration><voice>{voice_number}</voice>{rhythm}\
             <staff>{staff_number}</staff>{fermata}</note>"
        );
        return;
    }

    let count = beat.notes.len();
    // Chords are listed from the lowest note.
    let listed: Vec<&Note> = match drums {
        // The pieces of a kit have no order but the one they were put in.
        true => beat.notes.iter().collect(),
        false => beat.notes.iter().rev().collect(),
    };
    for (index, note) in listed.into_iter().enumerate() {
        let e = &note.effects;
        let before = previous.and_then(|b| b.note_on(note.string));
        let after = next.and_then(|b| b.note_on(note.string));
        let tied_on = after.is_some_and(|n| n.tied) && !drums;
        let tied = note.tied && !drums;
        skipped.add_if(e.harmonic_touch.is_some(), "where a harmonic is touched");
        skipped.add_if(
            e.trill.is_some_and(|t| t.speed != NoteValue::Sixteenth),
            "how fast a trill is played",
        );

        let mut xml = String::from("<note>");
        if index > 0 {
            xml.push_str("<chord/>");
        }
        match drums {
            true => xml.push_str(&drum_xml(note.fret)),
            false => xml.push_str(
                &(at.staff.pitch(note.string, note.fret)).map_or(String::new(), pitch_xml),
            ),
        }
        let _ = write!(xml, "<duration>{ticks}</duration>");
        if tied {
            xml.push_str("<tie type=\"stop\"/>");
        }
        if tied_on {
            xml.push_str("<tie type=\"start\"/>");
        }
        if drums {
            let _ = write!(
                xml,
                "<instrument id=\"P{}-I{}\"/>",
                at.part + 1,
                u32::from(note.fret) + 1
            );
        }
        let _ = write!(xml, "<voice>{voice_number}</voice>{rhythm}");
        if at.tab {
            // Under the notation, the tablature: the same notes with their strings and
            // frets, and no more than holds them together.
            let _ = write!(xml, "<staff>{staff_number}</staff><notations>");
            if tied {
                xml.push_str("<tied type=\"stop\"/>");
            }
            if tied_on {
                xml.push_str("<tied type=\"start\"/>");
            }
            if index == 0 && starts {
                xml.push_str("<tuplet type=\"start\"/>");
            }
            if index == 0 && stops {
                xml.push_str("<tuplet type=\"stop\"/>");
            }
            let _ = writeln!(
                xml,
                "<technical><string>{}</string><fret>{}</fret></technical></notations></note>",
                note.string + 1,
                note.fret
            );
            out.push_str(&xml);
            continue;
        }
        let head = match drums {
            true => kit.iter().find(|p| p.number == note.fret).map(|p| p.head),
            false => None,
        };
        let shape = match head {
            Some(DrumHead::Cross) => Some("x"),
            Some(DrumHead::CircledCross) => Some("circle-x"),
            Some(DrumHead::Diamond) => Some("diamond"),
            Some(DrumHead::Triangle) => Some("triangle"),
            _ if e.dead => Some("x"),
            _ if e.ghost => Some("normal"),
            _ => None,
        };
        if let Some(shape) = shape {
            let brackets = match e.ghost {
                true => " parentheses=\"yes\"",
                false => "",
            };
            let _ = write!(xml, "<notehead{brackets}>{shape}</notehead>");
        }
        let _ = write!(xml, "<staff>{staff_number}</staff>");

        let mut notations = String::new();
        if tied {
            notations.push_str("<tied type=\"stop\"/>");
        }
        if tied_on {
            notations.push_str("<tied type=\"start\"/>");
        }
        if e.let_ring {
            notations.push_str("<tied type=\"let-ring\"/>");
        }
        if index == 0 {
            if starts {
                notations.push_str("<tuplet type=\"start\"/>");
            }
            if stops {
                notations.push_str("<tuplet type=\"stop\"/>");
            }
        }
        match e.slide {
            Some(Slide::Shift) => notations.push_str("<slide type=\"start\" line-type=\"solid\"/>"),
            Some(Slide::Legato) => notations.push_str(
                "<slur type=\"start\" number=\"2\"/><slide type=\"start\" line-type=\"solid\"/>",
            ),
            _ => {}
        }
        match before.and_then(|n| n.effects.slide) {
            Some(Slide::Shift) => notations.push_str("<slide type=\"stop\"/>"),
            Some(Slide::Legato) => {
                notations.push_str("<slur type=\"stop\" number=\"2\"/><slide type=\"stop\"/>");
            }
            _ => {}
        }

        let mut ornaments = String::new();
        if e.trill.is_some() {
            ornaments.push_str("<trill-mark/>");
        }
        if e.vibrato.is_some() {
            ornaments.push_str("<wavy-line type=\"start\"/><wavy-line type=\"stop\"/>");
        }
        if index == 0
            && let Some(value) = effects.tremolo
        {
            let _ = write!(
                ornaments,
                "<tremolo type=\"single\">{}</tremolo>",
                value.flags()
            );
        }
        if !ornaments.is_empty() {
            let _ = write!(notations, "<ornaments>{ornaments}</ornaments>");
        }

        let mut technical = String::new();
        if index == 0 {
            match effects.stroke {
                Some(PickStroke::Up) => technical.push_str("<up-bow/>"),
                Some(PickStroke::Down) => technical.push_str("<down-bow/>"),
                None => {}
            }
        }
        if let Some(harmonic) = e.harmonic {
            let _ = write!(
                technical,
                "<harmonic>{}</harmonic>",
                if harmonic == Harmonic::Natural {
                    "<natural/>"
                } else {
                    "<artificial/>"
                }
            );
        }
        if let Some(finger) = e.left_finger {
            let _ = write!(technical, "<fingering>{}</fingering>", finger.left_label());
        }
        if let Some(finger) = e.right_finger {
            let _ = write!(technical, "<pluck>{}</pluck>", finger.right_label());
        }
        // A hammer-on if the next note is higher up the string, a pull-off if lower.
        let legato = |from: &Note, to: Option<&Note>| match to {
            Some(to) if to.fret < from.fret => ("pull-off", "P"),
            _ => ("hammer-on", "H"),
        };
        if e.hammer {
            let (tag, letter) = legato(note, after);
            let _ = write!(technical, "<{tag} type=\"start\">{letter}</{tag}>");
        }
        if let Some(from) = before.filter(|n| n.effects.hammer) {
            let (tag, _) = legato(from, Some(note));
            let _ = write!(technical, "<{tag} type=\"stop\"/>");
        }
        if e.tapped {
            technical.push_str("<tap/>");
        }
        if let Some(bend) = &e.bend {
            technical.push_str(&bend_xml(bend));
        }
        let mut other = |text: &str| {
            let _ = write!(
                technical,
                "<other-technical>{}</other-technical>",
                esc(text)
            );
        };
        if e.palm_mute {
            other(PALM_MUTE);
        }
        if let Some(name) = e.harmonic.and_then(harmonic_name) {
            other(name);
        }
        if e.vibrato == Some(Vibrato::Wide) {
            other(WIDE_VIBRATO);
        }
        if let Some(trill) = e.trill {
            other(&format!("{TRILL_TO}{}", trill.fret));
        }
        if index == 0 {
            match effects.slap {
                Some(Slap::Slap) => other(SLAP),
                Some(Slap::Pop) => other(POP),
                None => {}
            }
        }
        if !technical.is_empty() {
            let _ = write!(notations, "<technical>{technical}</technical>");
        }

        let mut articulations = String::new();
        match e.accent {
            Some(Accent::Normal) => articulations.push_str("<accent/>"),
            Some(Accent::Heavy) => articulations.push_str("<strong-accent/>"),
            None => {}
        }
        if e.staccato {
            articulations.push_str("<staccato/>");
        }
        match e.slide {
            Some(Slide::InFromBelow) => articulations.push_str("<scoop/>"),
            Some(Slide::InFromAbove) => articulations.push_str("<plop/>"),
            Some(Slide::OutUp) => articulations.push_str("<doit/>"),
            Some(Slide::OutDown) => articulations.push_str("<falloff/>"),
            _ => {}
        }
        if !articulations.is_empty() {
            let _ = write!(notations, "<articulations>{articulations}</articulations>");
        }
        if effects.fermata && index == 0 {
            notations.push_str("<fermata/>");
        }
        // An arpeggio from the lowest string is one that goes up.
        match effects.strum {
            Some(Strum::ArpeggioDown) if count > 1 => {
                notations.push_str("<arpeggiate direction=\"up\"/>");
            }
            Some(Strum::ArpeggioUp) if count > 1 => {
                notations.push_str("<arpeggiate direction=\"down\"/>");
            }
            Some(Strum::ArpeggioDown | Strum::ArpeggioUp) => skipped.add("strums"),
            _ => {}
        }
        if !notations.is_empty() {
            let _ = write!(xml, "<notations>{notations}</notations>");
        }
        if index == 0 {
            // A syllable that ends in a hyphen goes on into the next.
            for (line, sung) in effects.lyrics.iter().enumerate() {
                if sung.is_empty() {
                    continue;
                }
                let inside = previous
                    .and_then(|b| b.effects.lyrics.get(line))
                    .is_some_and(|before| before.ends_with('-'));
                let (text, on) = match sung.strip_suffix('-') {
                    Some(text) if !text.is_empty() => (text, true),
                    _ => (sung.as_str(), false),
                };
                let syllabic = match (inside, on) {
                    (false, false) => "single",
                    (false, true) => "begin",
                    (true, true) => "middle",
                    (true, false) => "end",
                };
                let _ = write!(
                    xml,
                    "<lyric number=\"{}\"><syllabic>{syllabic}</syllabic><text>{}</text></lyric>",
                    line + 1,
                    esc(text)
                );
            }
        }
        xml.push_str("</note>\n");
        out.push_str(&xml);
    }
}
