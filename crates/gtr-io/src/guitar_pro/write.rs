//! Guitar Pro export: the `.gp` file of versions 7 and 8, a zip archive whose
//! `Content/score.gpif` is the XML document that [`super::gp7`] reads. It is written the
//! way Guitar Pro 7 writes it, with the lists of bars, voices, beats, notes and rhythms
//! that refer to each other by number.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{Cursor, Write as _};

use gtr_model::{
    Accent, Bar, Beat, Bend, ChordDiagram, Clef, Duration, Dynamic, Fade, Feel, Finger, Hairpin,
    Harmonic, Instrument, Jump, MasterBar, Mix, Note, NoteValue, PickStroke, Rational, Score, Slap,
    Slide, Staff, Strum, Target, Track, Vibrato, bar_times,
};
use zip::{ZipWriter, write::SimpleFileOptions};

use super::rse;
use crate::util::esc;
use crate::{ExportError, Skipped};

/// The mixing desk's volume for a track that has not been moved; see the reader.
pub(super) const DESK_UNITY: f64 = 0.795;

pub(super) fn write(score: &Score, skipped: &mut Skipped) -> Result<Vec<u8>, ExportError> {
    let xml = gpif(score, skipped);
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    zip.start_file("VERSION", options)?;
    zip.write_all(b"7.0")?;
    zip.add_directory("Content/", options)?;
    zip.start_file("Content/PartConfiguration", options)?;
    zip.write_all(&part_configuration(score))?;
    zip.start_file("Content/score.gpif", options)?;
    zip.write_all(xml.as_bytes())?;
    Ok(zip.finish()?.into_inner())
}

/// Which staves each part shows: the whole score first and then every track by itself,
/// each with a byte for every track in it, standard notation and tablature both.
fn part_configuration(score: &Score) -> Vec<u8> {
    let shows =
        |track: &Track| match track.staves[0].is_pitched() || track.instrument.is_percussion() {
            true => 1,
            false => 3,
        };
    let mut out = Vec::new();
    out.extend((score.tracks.len() as u32 + 1).to_be_bytes());
    out.push(0);
    out.extend((score.tracks.len() as u32).to_be_bytes());
    out.extend(score.tracks.iter().map(shows));
    for track in &score.tracks {
        out.push(0);
        out.extend(1u32.to_be_bytes());
        out.push(shows(track));
    }
    // The part that is open: the first track's.
    out.extend(1u32.to_be_bytes());
    out
}

/// The lists the score's parts are kept in, each part named by its place in its list.
#[derive(Default)]
struct Lists {
    bars: String,
    bar_count: usize,
    voices: String,
    voice_count: usize,
    beats: String,
    beat_count: usize,
    notes: String,
    note_count: usize,
    rhythms: String,
    rhythm_ids: HashMap<Duration, usize>,
}

impl Lists {
    fn rhythm(&mut self, duration: Duration) -> usize {
        if let Some(id) = self.rhythm_ids.get(&duration) {
            return *id;
        }
        let id = self.rhythm_ids.len();
        self.rhythm_ids.insert(duration, id);
        let value = match duration.value {
            NoteValue::Whole => "Whole",
            NoteValue::Half => "Half",
            NoteValue::Quarter => "Quarter",
            NoteValue::Eighth => "Eighth",
            NoteValue::Sixteenth => "16th",
            NoteValue::ThirtySecond => "32nd",
            NoteValue::SixtyFourth => "64th",
            NoteValue::HundredTwentyEighth => "128th",
            NoteValue::TwoHundredFiftySixth => "256th",
        };
        let _ = write!(
            self.rhythms,
            "<Rhythm id=\"{id}\"><NoteValue>{value}</NoteValue>"
        );
        if duration.dots > 0 {
            let _ = write!(
                self.rhythms,
                "<AugmentationDot count=\"{}\"/>",
                duration.dots
            );
        }
        for (name, tuplet) in [
            ("PrimaryTuplet", duration.tuplet),
            ("SecondaryTuplet", duration.nested),
        ] {
            if let Some(tuplet) = tuplet {
                let _ = write!(
                    self.rhythms,
                    "<{name} num=\"{}\" den=\"{}\"/>",
                    tuplet.actual, tuplet.normal
                );
            }
        }
        self.rhythms.push_str("</Rhythm>\n");
        id
    }
}

fn gpif(score: &Score, skipped: &mut Skipped) -> String {
    let mut lists = Lists::default();
    let times = bar_times(score);

    // What each track's beats refer to, and what is written inside the track.
    let mut tracks = String::new();
    let mut parts: Vec<Part> = Vec::new();
    let mut channel = 0;
    for (index, track) in score.tracks.iter().enumerate() {
        let part = Part::new(track);
        let channels = match track.instrument.is_percussion() {
            true => (9, 9),
            false => {
                // Two channels to a track, stepping over the one that is percussion's.
                if channel == 8 {
                    channel = 10;
                }
                channel += 2;
                ((channel - 2) % 16, (channel - 1) % 16)
            }
        };
        tracks.push_str(&track_xml(index, track, score, &part, channels, skipped));
        parts.push(part);
    }

    let mut masters = String::new();
    // The loudness in force on each staff, which every beat of the file states.
    let mut dynamics = vec![Dynamic::F; score.tracks.iter().map(|t| t.staves.len()).sum()];
    let mut feel = Feel::Straight;
    for (index, master) in score.master_bars.iter().enumerate() {
        feel = master.feel.unwrap_or(feel);
        let mut ids = Vec::new();
        let mut fermatas: Vec<Rational> = Vec::new();
        let mut staff_number = 0;
        for (track, part) in score.tracks.iter().zip(&parts) {
            for (staff_index, staff) in track.staves.iter().enumerate() {
                let at = Place {
                    track,
                    part,
                    staff,
                    bar: index,
                    // The time the piece has run for is written on one beat of the bar.
                    timer: (master.timer && staff_number == 0).then(|| times[index].unwrap_or(0.0)),
                };
                let dynamic = &mut dynamics[staff_number];
                ids.push(bar_xml(&at, staff_index, dynamic, &mut lists, skipped));
                for voice in &staff.bars[index].voices {
                    let mut time = Rational::from_integer(0);
                    for beat in &voice.beats {
                        if beat.effects.fermata && !fermatas.contains(&time) {
                            fermatas.push(time);
                        }
                        time += beat.duration.length();
                    }
                }
                staff_number += 1;
            }
        }
        masters.push_str(&master_xml(master, feel, &ids, &fermatas));
    }

    let (master, left_out) = rse::effects(&score.effects, true);
    skipped.add_if(left_out, "pedals Guitar Pro has no match for");
    let info = &score.info;
    let track_ids: Vec<String> = (0..score.tracks.len()).map(|i| i.to_string()).collect();
    let mut out = String::new();
    let _ = write!(
        out,
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<GPIF>\n<GPVersion>7</GPVersion>\n\
         <GPRevision>12020</GPRevision>\n\
         <Encoding><EncodingDescription>GP7</EncodingDescription></Encoding>\n\
         <Score>\n<Title>{}</Title>\n<SubTitle>{}</SubTitle>\n<Artist>{}</Artist>\n\
         <Album>{}</Album>\n<Words>{}</Words>\n<Music>{}</Music>\n\
         <WordsAndMusic></WordsAndMusic>\n<Copyright>{}</Copyright>\n<Tabber>{}</Tabber>\n\
         <Instructions></Instructions>\n<Notices></Notices>\n\
         <ScoreSystemsDefaultLayout>4</ScoreSystemsDefaultLayout>\n\
         <ScoreSystemsLayout>4</ScoreSystemsLayout>\n<ScoreZoomPolicy>Value</ScoreZoomPolicy>\n\
         <ScoreZoom>1</ScoreZoom>\n<MultiVoice>0&gt;</MultiVoice>\n</Score>\n\
         <MasterTrack>\n<Tracks>{}</Tracks>\n<Automations>\n{}</Automations>\n\
         <RSE><Master>\n{master}</Master></RSE>\n</MasterTrack>\n\
         <AudioTracks>\n</AudioTracks>\n<Tracks>\n{tracks}</Tracks>\n\
         <MasterBars>\n{masters}</MasterBars>\n",
        esc(&info.title),
        esc(&info.subtitle),
        esc(&info.artist),
        esc(&info.album),
        esc(&info.words),
        esc(&info.music),
        esc(&info.copyright),
        esc(&info.transcriber),
        track_ids.join(" "),
        tempo_automations(score),
    );
    let _ = write!(
        out,
        "<Bars>\n{}</Bars>\n<Voices>\n{}</Voices>\n<Beats>\n{}</Beats>\n<Notes>\n{}</Notes>\n\
         <Rhythms>\n{}</Rhythms>\n</GPIF>\n",
        lists.bars, lists.voices, lists.beats, lists.notes, lists.rhythms
    );
    out
}

/// A change to something that can change as the piece goes: its kind, whether it gets
/// there by degrees, the bar, how far through the bar from 0 to 1, and the new value.
fn automation(
    kind: &str,
    linear: bool,
    bar: usize,
    position: f64,
    visible: bool,
    value: &str,
) -> String {
    format!(
        "<Automation><Type>{kind}</Type><Linear>{linear}</Linear><Bar>{bar}</Bar>\
         <Position>{position}</Position><Visible>{visible}</Visible><Value>{value}</Value>\
         </Automation>\n"
    )
}

/// How far through its bar a moment is, from 0 to 1.
fn share(position: Rational, master: &MasterBar) -> f64 {
    let share = position / master.time_signature.bar_length();
    *share.numer() as f64 / *share.denom() as f64
}

/// The tempo the piece starts at, and every change to it, in quarter notes a minute.
fn tempo_automations(score: &Score) -> String {
    let mut out = String::new();
    let mut tempo = score.tempo;
    for (index, master) in score.master_bars.iter().enumerate() {
        tempo = master.tempo.unwrap_or(tempo);
        if index == 0 || master.tempo.is_some() || master.tempo_gradual {
            let value = format!("{tempo} 2");
            out.push_str(&automation(
                "Tempo",
                master.tempo_gradual,
                index,
                0.0,
                true,
                &value,
            ));
        }
        for change in &master.tempo_changes {
            tempo = change.tempo;
            let at = share(change.position, master);
            let value = format!("{tempo} 2");
            out.push_str(&automation(
                "Tempo",
                change.gradual,
                index,
                at,
                true,
                &value,
            ));
        }
    }
    out
}

/// What the beats and notes of a track refer to.
struct Part {
    /// For a drum track, the General MIDI number of each articulation of its kit, in the
    /// order the kit lists them.
    kit: Vec<u8>,
    /// The track's chords: a name and a fingering, each named by its place here.
    chords: Vec<(String, Option<ChordDiagram>)>,
}

impl Part {
    fn new(track: &Track) -> Part {
        let beats = || {
            (track.staves.iter().flat_map(|s| &s.bars))
                .flat_map(|b| &b.voices)
                .flat_map(|v| &v.beats)
        };
        let mut kit = Vec::new();
        if track.instrument.is_percussion() {
            kit.extend(track.drum_kit().iter().map(|piece| piece.number));
            // Pieces the kit does not have, that the track plays all the same.
            let mut others: Vec<u8> = (beats().flat_map(|b| &b.notes))
                .map(|n| n.fret)
                .filter(|fret| !kit.contains(fret))
                .collect();
            others.sort_unstable();
            others.dedup();
            kit.extend(others);
        }
        let mut chords = Vec::new();
        for beat in beats() {
            if let Some(chord) = chord_of(beat)
                && !chords.contains(&chord)
            {
                chords.push(chord);
            }
        }
        Part { kit, chords }
    }
}

/// The chord a beat names or draws, if it does either.
fn chord_of(beat: &Beat) -> Option<(String, Option<ChordDiagram>)> {
    let effects = &beat.effects;
    (effects.chord.is_some() || effects.diagram.is_some()).then(|| {
        (
            effects.chord.clone().unwrap_or_default(),
            effects.diagram.clone(),
        )
    })
}

fn track_xml(
    index: usize,
    track: &Track,
    score: &Score,
    part: &Part,
    channels: (u32, u32),
    skipped: &mut Skipped,
) -> String {
    let instrument = track.instrument;
    let mut out = String::new();
    let _ = write!(
        out,
        "<Track id=\"{index}\">\n<Name>{}</Name>\n<ShortName>{}</ShortName>\n\
         <Color>246 74 38</Color>\n<SystemsDefautLayout>3</SystemsDefautLayout>\n\
         <SystemsLayout>1</SystemsLayout>\n<PalmMute>0</PalmMute>\n\
         <PlayingStyle>Default</PlayingStyle>\n<IconId>{}</IconId>\n",
        esc(&track.name),
        esc(&track.name.chars().take(4).collect::<String>()),
        match instrument {
            Instrument::Drums => 18,
            Instrument::Bass => 5,
            Instrument::Piano => 10,
            _ => 1,
        },
    );

    // The kit, or the one sound of an instrument that has pitches.
    let articulation = |name: &str, line: i32, heads: &str, number: u8| {
        format!(
            "<Articulation><Name>{}</Name><StaffLine>{line}</StaffLine>\
             <Noteheads>{heads}</Noteheads><TechniquePlacement>outside</TechniquePlacement>\
             <TechniqueSymbol></TechniqueSymbol><InputMidiNumbers>{number}</InputMidiNumbers>\
             <OutputRSESound></OutputRSESound><OutputMidiNumber>{number}</OutputMidiNumber>\
             </Articulation>\n",
            esc(name)
        )
    };
    if instrument.is_percussion() {
        let kit = track.drum_kit();
        out.push_str("<InstrumentSet>\n<Name>Drums</Name>\n<Type>drumKit</Type>\n<LineCount>5</LineCount>\n<Elements>\n");
        for number in &part.kit {
            let piece = kit.iter().find(|piece| piece.number == *number);
            let name = piece.map_or_else(|| format!("Percussion {number}"), |p| p.name.clone());
            // The staff's lines and spaces are counted down from its top line.
            let line = 4 - piece.map_or(0, |p| i32::from(p.position));
            let heads = match piece.map(|p| p.head) {
                Some(gtr_model::DrumHead::Cross) => "noteheadXBlack noteheadXHalf noteheadXWhole",
                Some(gtr_model::DrumHead::CircledCross) => {
                    "noteheadCircleX noteheadCircleX noteheadCircleX"
                }
                Some(gtr_model::DrumHead::Diamond) => {
                    "noteheadDiamondWhite noteheadDiamondWhite noteheadDiamondWhite"
                }
                Some(gtr_model::DrumHead::Triangle) => {
                    "noteheadTriangleUpBlack noteheadTriangleUpHalf noteheadTriangleUpWhole"
                }
                _ => "noteheadBlack noteheadHalf noteheadWhole",
            };
            let _ = write!(
                out,
                "<Element><Name>{}</Name><Type>unpitched</Type><SoundbankName></SoundbankName>\
                 <Articulations>\n{}</Articulations></Element>\n",
                esc(&name),
                articulation(&name, line, heads, *number)
            );
        }
        out.push_str("</Elements>\n</InstrumentSet>\n");
    } else {
        let _ = write!(
            out,
            "<InstrumentSet>\n<Name>{}</Name>\n<Type>{}</Type>\n<LineCount>5</LineCount>\n\
             <Elements>\n<Element><Name>Pitched</Name><Type>pitched</Type>\
             <SoundbankName></SoundbankName><Articulations>\n{}</Articulations></Element>\n\
             </Elements>\n</InstrumentSet>\n",
            instrument.name(),
            match instrument {
                Instrument::Piano => "acousticPiano",
                Instrument::Bass => "electricBass",
                Instrument::SteelGuitar | Instrument::NylonGuitar => "steelGuitar",
                _ => "electricGuitar",
            },
            articulation("", 0, "noteheadBlack noteheadHalf noteheadWhole", 0)
        );
    }

    // Guitars and basses are written an octave above where they sound.
    let _ = writeln!(
        out,
        "<Transpose><Chromatic>0</Chromatic><Octave>{}</Octave></Transpose>",
        -i32::from(instrument.written_transposition() / 12)
    );

    // The mixing desk: the track's place between the speakers and its volume, and every
    // change the score makes to either.
    let pan = |pan: i8| 0.5 + f64::from(pan) / 200.0;
    let volume = |volume: u8, skipped: &mut Skipped| {
        let level = f64::from(volume) / f64::from(Mix::UNITY) * DESK_UNITY;
        skipped.add_if(level > 1.0, "volumes louder than Guitar Pro goes");
        level.min(1.0)
    };
    let mut desk = [0.5; 16];
    desk[11] = pan(track.mix.pan);
    desk[12] = volume(track.mix.volume, skipped);
    let numbers: Vec<String> = desk.iter().map(|value| format!("{value:.6}")).collect();
    let mut changes = String::new();
    changes.push_str(&automation(
        "DSPParam_11",
        false,
        0,
        0.0,
        false,
        &numbers[11],
    ));
    changes.push_str(&automation(
        "DSPParam_12",
        false,
        0,
        0.0,
        false,
        &numbers[12],
    ));
    for (bar, master) in score.master_bars.iter().enumerate() {
        for voice in (track.staves.iter()).flat_map(|staff| &staff.bars[bar].voices) {
            let mut time = Rational::from_integer(0);
            for beat in &voice.beats {
                let at = share(time, master).min(1.0);
                if let Some(to) = beat.effects.pan {
                    let value = format!("{:.6}", pan(to));
                    changes.push_str(&automation("DSPParam_11", false, bar, at, false, &value));
                }
                if let Some(to) = beat.effects.volume {
                    let value = format!("{:.6}", volume(to, skipped));
                    changes.push_str(&automation("DSPParam_12", false, bar, at, false, &value));
                }
                time += beat.duration.length();
            }
        }
    }
    // The track's own sound, then those it can switch to, each with its pedals.
    let mut sounds = String::new();
    let own = std::iter::once((instrument.name(), instrument, track.effects.as_slice()));
    let others =
        (track.sounds.iter()).map(|s| (s.name.as_str(), s.instrument, s.effects.as_slice()));
    for (name, instrument, chain) in own.chain(others) {
        let (effects, left_out) = rse::effects(chain, false);
        skipped.add_if(left_out, "pedals Guitar Pro has no match for");
        let _ = write!(
            sounds,
            "<Sound><Name>{name}</Name><Label>{name}</Label><Path></Path><Role>User</Role>\
             <MIDI><LSB>0</LSB><MSB>0</MSB><Program>{}</Program></MIDI>\
             <RSE><EffectChain>\n{effects}</EffectChain></RSE></Sound>\n",
            instrument.midi_program(),
            name = esc(name),
        );
    }
    let _ = write!(
        out,
        "<RSE><ChannelStrip version=\"E56\"><Parameters>{}</Parameters>\n\
         <Automations>\n{changes}</Automations></ChannelStrip></RSE>\n\
         <ForcedSound>-1</ForcedSound>\n\
         <Sounds>\n{sounds}</Sounds>\n\
         <MidiConnection><Port>0</Port><PrimaryChannel>{}</PrimaryChannel>\
         <SecondaryChannel>{}</SecondaryChannel>\
         <ForeOneChannelPerString>false</ForeOneChannelPerString></MidiConnection>\n\
         <PlaybackState>{}</PlaybackState>\n<AudioEngineState>MIDI</AudioEngineState>\n\
         <Lyrics dispatched=\"true\">{}</Lyrics>\n<Staves>\n",
        numbers.join(" "),
        channels.0,
        channels.1,
        match (track.mix.solo, track.mix.mute) {
            (true, _) => "Solo",
            (_, true) => "Mute",
            _ => "Default",
        },
        "<Line><Text></Text><Offset>0</Offset></Line>".repeat(5),
    );

    for (staff_index, staff) in track.staves.iter().enumerate() {
        let _ = write!(
            out,
            "<Staff><Properties>\n<Property name=\"CapoFret\"><Fret>{}</Fret></Property>\n\
             <Property name=\"FretCount\"><Number>24</Number></Property>\n\
             <Property name=\"PartialCapoFret\"><Fret>0</Fret></Property>\n\
             <Property name=\"PartialCapoStringFlags\"><Flags>0</Flags></Property>\n",
            staff.capo
        );
        if instrument.is_percussion() {
            // Guitar Pro gives a drum staff six strings of no pitch.
            out.push_str(
                "<Property name=\"Tuning\"><Pitches>0 0 0 0 0 0</Pitches>\
                 <Instrument>Undefined</Instrument></Property>\n",
            );
        } else if !staff.is_pitched() {
            // Listed from the lowest string.
            let pitches: Vec<String> = (staff.tuning.strings.iter().rev())
                .map(|pitch| pitch.to_string())
                .collect();
            let _ = writeln!(
                out,
                "<Property name=\"Tuning\"><Pitches>{}</Pitches><Instrument>1</Instrument>\
                 </Property>",
                pitches.join(" ")
            );
        }
        let mut items = String::new();
        // A track's chords are kept with its first staff.
        for (id, (name, diagram)) in part.chords.iter().enumerate().filter(|_| staff_index == 0) {
            let _ = write!(items, "<Item id=\"{id}\" name=\"{}\">", esc(name));
            match diagram {
                Some(diagram) => {
                    // Frets are counted from the diagram's first; one with an open string
                    // starts at the nut, so that open is told from stopped.
                    let open = diagram.frets.contains(&0);
                    let base = match open {
                        true => 0,
                        false => i32::from(diagram.base_fret()) - 1,
                    };
                    let _ = write!(
                        items,
                        "<Diagram stringCount=\"{}\" fretCount=\"5\" baseFret=\"{base}\">",
                        diagram.frets.len()
                    );
                    let count = diagram.frets.len();
                    for (string, fret) in diagram.frets.iter().enumerate().rev() {
                        if *fret >= 0 {
                            let _ = write!(
                                items,
                                "<Fret string=\"{}\" fret=\"{}\"/>",
                                count - 1 - string,
                                i32::from(*fret) - base
                            );
                        }
                    }
                    items.push_str("</Diagram>");
                }
                None => {
                    let count = staff.tuning.strings.len();
                    let _ = write!(items, "<Diagram stringCount=\"{count}\" fretCount=\"5\"/>");
                }
            }
            items.push_str("</Item>\n");
        }
        let _ = write!(
            out,
            "<Property name=\"ChordCollection\"><Items/></Property>\n\
             <Property name=\"ChordWorkingSet\"><Items/></Property>\n\
             <Property name=\"DiagramCollection\"><Items>\n{items}</Items></Property>\n\
             <Property name=\"DiagramWorkingSet\"><Items/></Property>\n\
             <Name>Standard</Name>\n</Properties></Staff>\n"
        );
    }
    // The sound the track starts with, and each switch to another, which a switch names
    // by its path, its name and its role.
    let named = |sound: u8| match usize::from(sound).checked_sub(1) {
        None => instrument.name().to_owned(),
        Some(other) => (track.sounds.get(other))
            .map_or_else(|| instrument.name().to_owned(), |s| s.name.clone()),
    };
    let mut switches = automation(
        "Sound",
        false,
        0,
        0.0,
        true,
        &esc(&format!(";{};User", named(0))),
    );
    for (bar, master) in score.master_bars.iter().enumerate() {
        let mut time = Rational::from_integer(0);
        for beat in &track.staves[0].bars[bar].voices[0].beats {
            if let Some(sound) = beat
                .effects
                .sound
                .filter(|_| bar > 0 || time > Rational::from_integer(0))
            {
                let at = share(time, master).min(1.0);
                let value = esc(&format!(";{};User", named(sound)));
                switches.push_str(&automation("Sound", false, bar, at, true, &value));
            }
            time += beat.duration.length();
        }
    }
    let _ = write!(
        out,
        "</Staves>\n<Automations>\n{switches}</Automations>\n</Track>\n"
    );
    out
}

fn master_xml(master: &MasterBar, feel: Feel, bars: &[usize], fermatas: &[Rational]) -> String {
    let mut out = String::new();
    let time = master.time_signature;
    let _ = write!(
        out,
        "<MasterBar>\n<Key><AccidentalCount>{}</AccidentalCount><Mode>Major</Mode>\
         <TransposeAs>{}</TransposeAs></Key>\n<Time>{}/{}</Time>\n",
        master.key_signature,
        if master.key_signature < 0 {
            "Flats"
        } else {
            "Sharps"
        },
        time.numerator,
        time.denominator
    );
    if master.repeat_start || master.repeat_count > 0 {
        let _ = writeln!(
            out,
            "<Repeat start=\"{}\" end=\"{}\" count=\"{}\"/>",
            master.repeat_start,
            master.repeat_count > 0,
            master.repeat_count
        );
    }
    if master.alternate_endings != 0 {
        let passes: Vec<String> = (0..8)
            .filter(|pass| master.alternate_endings & (1 << pass) != 0)
            .map(|pass| (pass + 1).to_string())
            .collect();
        let _ = writeln!(
            out,
            "<AlternateEndings>{}</AlternateEndings>",
            passes.join(" ")
        );
    }
    if let Some(section) = &master.section {
        let _ = writeln!(
            out,
            "<Section><Letter></Letter><Text>{}</Text></Section>",
            esc(section)
        );
    }
    let feel = match feel {
        Feel::Straight => None,
        Feel::SwingEighths => Some("Triplet8th"),
        Feel::SwingSixteenths => Some("Triplet16th"),
        Feel::DottedEighths => Some("Dotted8th"),
        Feel::DottedSixteenths => Some("Dotted16th"),
        Feel::ScottishEighths => Some("Scottish8th"),
        Feel::ScottishSixteenths => Some("Scottish16th"),
    };
    if let Some(feel) = feel {
        let _ = writeln!(out, "<TripletFeel>{feel}</TripletFeel>");
    }
    if !master.targets.is_empty() || !master.jumps.is_empty() {
        out.push_str("<Directions>");
        for target in &master.targets {
            let name = match target {
                Target::Segno => "Segno",
                Target::SegnoSegno => "SegnoSegno",
                Target::Coda => "Coda",
                Target::DoubleCoda => "DoubleCoda",
                Target::Fine => "Fine",
            };
            let _ = write!(out, "<Target>{name}</Target>");
        }
        for jump in &master.jumps {
            let name = match jump {
                Jump::DaCapo => "DaCapo",
                Jump::DaCapoAlFine => "DaCapoAlFine",
                Jump::DaCapoAlCoda => "DaCapoAlCoda",
                Jump::DaCapoAlDoubleCoda => "DaCapoAlDoubleCoda",
                Jump::DalSegno => "DaSegno",
                Jump::DalSegnoAlFine => "DaSegnoAlFine",
                Jump::DalSegnoAlCoda => "DaSegnoAlCoda",
                Jump::DalSegnoAlDoubleCoda => "DaSegnoAlDoubleCoda",
                Jump::DalSegnoSegno => "DaSegnoSegno",
                Jump::DalSegnoSegnoAlFine => "DaSegnoSegnoAlFine",
                Jump::DalSegnoSegnoAlCoda => "DaSegnoSegnoAlCoda",
                Jump::DalSegnoSegnoAlDoubleCoda => "DaSegnoSegnoAlDoubleCoda",
                Jump::ToCoda => "DaCoda",
                Jump::ToDoubleCoda => "DaDoubleCoda",
            };
            let _ = write!(out, "<Jump>{name}</Jump>");
        }
        out.push_str("</Directions>\n");
    }
    if !fermatas.is_empty() {
        out.push_str("<Fermatas>");
        for position in fermatas {
            // Counted in quarter notes from the start of the bar.
            let quarters = position * 4;
            let _ = write!(
                out,
                "<Fermata><Type>Medium</Type><Length>0.5</Length><Offset>{}/{}</Offset></Fermata>",
                quarters.numer(),
                quarters.denom()
            );
        }
        out.push_str("</Fermatas>\n");
    }
    let ids: Vec<String> = bars.iter().map(|id| id.to_string()).collect();
    let _ = write!(out, "<Bars>{}</Bars>\n</MasterBar>\n", ids.join(" "));
    out
}

/// Where a bar is, with what its beats need to know of it.
struct Place<'a> {
    track: &'a Track,
    part: &'a Part,
    staff: &'a Staff,
    bar: usize,
    /// Seconds the piece has run for when it gets here, if the bar shows them.
    timer: Option<f64>,
}

impl Place<'_> {
    /// The beat that follows one in its voice, in the same bar or the next that has any.
    fn next(&self, voice: usize, beat: usize) -> Option<&Beat> {
        let here = &self.staff.bars[self.bar].voices[voice].beats;
        here.get(beat + 1).or_else(|| {
            (self.staff.bars[self.bar + 1..].iter())
                .filter_map(|bar| bar.voices.get(voice))
                .find_map(|voice| voice.beats.first())
        })
    }

    /// The beat before one in its voice.
    fn previous(&self, voice: usize, beat: usize) -> Option<&Beat> {
        let here = &self.staff.bars[self.bar].voices[voice].beats;
        match beat {
            0 => (self.staff.bars[..self.bar].iter().rev())
                .filter_map(|bar| bar.voices.get(voice))
                .find_map(|voice| voice.beats.last()),
            _ => here.get(beat - 1),
        }
    }
}

/// Writes a staff's bar with its voices and their beats, and gives its number.
fn bar_xml(
    at: &Place,
    staff_index: usize,
    dynamic: &mut Dynamic,
    lists: &mut Lists,
    skipped: &mut Skipped,
) -> usize {
    let bar: &Bar = &at.staff.bars[at.bar];
    let mut ids = Vec::new();
    for (voice_index, voice) in bar.voices.iter().enumerate().take(Bar::MAX_VOICES) {
        // A voice with nothing in it is left out, as Guitar Pro leaves it.
        if voice.beats.is_empty() {
            ids.push("-1".to_owned());
            continue;
        }
        let mut beat_ids = Vec::new();
        for (index, beat) in voice.beats.iter().enumerate() {
            if voice_index == 0
                && !beat.is_rest()
                && let Some(marked) = beat.effects.dynamic
            {
                *dynamic = marked;
            }
            let around = (at.previous(voice_index, index), at.next(voice_index, index));
            let timer = at.timer.filter(|_| voice_index == 0 && index == 0);
            beat_xml(
                at,
                beat,
                around,
                *dynamic,
                timer,
                lists,
                &mut beat_ids,
                skipped,
            );
        }
        let id = lists.voice_count;
        lists.voice_count += 1;
        let _ = writeln!(
            lists.voices,
            "<Voice id=\"{id}\"><Beats>{}</Beats></Voice>",
            beat_ids.join(" ")
        );
        ids.push(id.to_string());
    }
    ids.resize(Bar::MAX_VOICES, "-1".to_owned());
    let id = lists.bar_count;
    lists.bar_count += 1;
    let clef = match at.track.instrument.clef(staff_index) {
        Clef::Treble => "G2",
        Clef::Bass => "F4",
        Clef::Percussion => "Neutral",
    };
    let _ = writeln!(
        lists.bars,
        "<Bar id=\"{id}\"><Clef>{clef}</Clef><Voices>{}</Voices></Bar>",
        ids.join(" ")
    );
    id
}

/// A pitch curve as Guitar Pro keeps one: where it starts, a level it holds in the middle
/// and where it ends, levels in hundredths of a whole tone and moments in hundredths of
/// the note. A curve of more points than that keeps its start, its furthest reach and its
/// end, and `exact` says that it did not fit.
struct Curve {
    origin: (f64, f64),
    middle: Option<(f64, f64, f64)>,
    destination: (f64, f64),
    exact: bool,
}

fn curve(bend: &Bend) -> Curve {
    let point = |index: usize| {
        let p = bend.points[index];
        (f64::from(p.position) / 0.6, f64::from(p.value) * 25.0)
    };
    let mut count = bend.points.len();
    let fits =
        |count: usize| count <= 3 || (count == 4 && bend.points[1].value == bend.points[2].value);
    // A level held to the end of the note need not be said twice.
    while !fits(count) && bend.points[count - 1].value == bend.points[count - 2].value {
        count -= 1;
    }
    let last = point(count - 1);
    let (middle, exact) = match count {
        1 | 2 => (None, true),
        3 => (Some((point(1).0, point(1).0, point(1).1)), true),
        4 if fits(4) => (Some((point(1).0, point(2).0, point(1).1)), true),
        _ => {
            let peak = bend.peak();
            let reached = |p: &gtr_model::CurvePoint| p.value == peak;
            let first = bend.points.iter().position(reached).unwrap_or(0);
            let end = bend.points.iter().rposition(reached).unwrap_or(first);
            (Some((point(first).0, point(end).0, point(first).1)), false)
        }
    };
    Curve {
        origin: point(0),
        middle,
        destination: last,
        exact,
    }
}

#[allow(clippy::too_many_arguments)]
fn beat_xml(
    at: &Place,
    beat: &Beat,
    (previous, next): (Option<&Beat>, Option<&Beat>),
    dynamic: Dynamic,
    timer: Option<f64>,
    lists: &mut Lists,
    ids: &mut Vec<String>,
    skipped: &mut Skipped,
) {
    let dynamic = match dynamic {
        Dynamic::Ppp => "PPP",
        Dynamic::Pp => "PP",
        Dynamic::P => "P",
        Dynamic::Mp => "MP",
        Dynamic::Mf => "MF",
        Dynamic::F => "F",
        Dynamic::Ff => "FF",
        Dynamic::Fff => "FFF",
    };
    // Grace notes are beats of their own, before the beat they lead into.
    for note in &beat.notes {
        let Some(grace) = &note.effects.grace else {
            continue;
        };
        for ornament in &grace.notes {
            let mut lead = Note::new(ornament.string, ornament.fret);
            lead.effects.hammer = grace.hammer;
            let note_id = note_xml(at, &lead, (false, false), lists);
            let id = lists.beat_count;
            lists.beat_count += 1;
            let rhythm = lists.rhythm(Duration::new(NoteValue::ThirtySecond));
            let _ = writeln!(
                lists.beats,
                "<Beat id=\"{id}\"><GraceNotes>{}</GraceNotes><Dynamic>{dynamic}</Dynamic>\
                 <Rhythm ref=\"{rhythm}\"/><Notes>{note_id}</Notes></Beat>",
                if grace.on_beat {
                    "OnBeat"
                } else {
                    "BeforeBeat"
                }
            );
            ids.push(id.to_string());
        }
    }

    let effects = &beat.effects;
    let id = lists.beat_count;
    lists.beat_count += 1;
    let rhythm = lists.rhythm(beat.duration);
    let mut out =
        format!("<Beat id=\"{id}\"><Dynamic>{dynamic}</Dynamic><Rhythm ref=\"{rhythm}\"/>");
    if let Some(chord) = chord_of(beat)
        && let Some(number) = at.part.chords.iter().position(|c| *c == chord)
    {
        let _ = write!(out, "<Chord>{number}</Chord>");
    }
    if let Some(text) = &effects.text {
        let _ = write!(out, "<FreeText>{}</FreeText>", esc(text));
    }
    if let Some(value) = effects.tremolo {
        let speed = match value {
            NoteValue::Eighth => "1/2",
            NoteValue::Sixteenth => "1/4",
            _ => "1/8",
        };
        let _ = write!(out, "<Tremolo>{speed}</Tremolo>");
    }
    match effects.strum {
        Some(Strum::ArpeggioUp) => out.push_str("<Arpeggio>Up</Arpeggio>"),
        Some(Strum::ArpeggioDown) => out.push_str("<Arpeggio>Down</Arpeggio>"),
        Some(Strum::Rasgueado) => out.push_str("<Rasgueado>ii_1</Rasgueado>"),
        _ => {}
    }
    match effects.hairpin {
        Some(Hairpin::Crescendo) => out.push_str("<Hairpin>Crescendo</Hairpin>"),
        Some(Hairpin::Decrescendo) => out.push_str("<Hairpin>Decrescendo</Hairpin>"),
        None => {}
    }
    match effects.fade {
        Some(Fade::In) => out.push_str("<Fadding>FadeIn</Fadding>"),
        Some(Fade::Out) => out.push_str("<Fadding>FadeOut</Fadding>"),
        Some(Fade::Swell) => out.push_str("<Fadding>VolumeSwell</Fadding>"),
        None => {}
    }
    if let Some(whammy) = &effects.whammy {
        let shape = curve(whammy);
        skipped.add_if(
            !shape.exact,
            "the exact shape of curves with more than four points",
        );
        let _ = write!(
            out,
            "<Whammy originValue=\"{:.6}\" originOffset=\"{:.6}\"",
            shape.origin.1, shape.origin.0
        );
        if let Some((from, to, level)) = shape.middle {
            let _ = write!(
                out,
                " middleValue=\"{level:.6}\" middleOffset1=\"{from:.6}\" middleOffset2=\"{to:.6}\""
            );
        }
        let _ = write!(
            out,
            " destinationValue=\"{:.6}\" destinationOffset=\"{:.6}\"/>",
            shape.destination.1, shape.destination.0
        );
    }
    if let Some(seconds) = timer {
        let _ = write!(out, "<Timer>{}</Timer>", seconds.round() as u64);
    }
    if !beat.notes.is_empty() {
        let mut note_ids = Vec::new();
        for note in &beat.notes {
            let fits = (note.effects.bend.as_ref()).is_none_or(|bend| curve(bend).exact);
            skipped.add_if(
                !fits,
                "the exact shape of curves with more than four points",
            );
            // What the notes before and after on the same string say of this one: that it
            // is hammered on to, and that it is tied on from.
            let hammered =
                (previous.and_then(|b| b.note_on(note.string))).is_some_and(|n| n.effects.hammer);
            let tied_on = (next.and_then(|b| b.note_on(note.string))).is_some_and(|n| n.tied);
            let around = (hammered, tied_on && !at.track.instrument.is_percussion());
            note_ids.push(note_xml(at, note, around, lists).to_string());
        }
        let _ = write!(out, "<Notes>{}</Notes>", note_ids.join(" "));
    }
    let mut properties = String::new();
    match effects.strum {
        Some(Strum::BrushUp) => {
            properties.push_str("<Property name=\"Brush\"><Direction>Up</Direction></Property>");
        }
        Some(Strum::BrushDown) => {
            properties.push_str("<Property name=\"Brush\"><Direction>Down</Direction></Property>");
        }
        _ => {}
    }
    match effects.stroke {
        Some(PickStroke::Up) => properties
            .push_str("<Property name=\"PickStroke\"><Direction>Up</Direction></Property>"),
        Some(PickStroke::Down) => properties
            .push_str("<Property name=\"PickStroke\"><Direction>Down</Direction></Property>"),
        None => {}
    }
    match effects.slap {
        Some(Slap::Slap) => properties.push_str("<Property name=\"Slapped\"><Enable/></Property>"),
        Some(Slap::Pop) => properties.push_str("<Property name=\"Popped\"><Enable/></Property>"),
        None => {}
    }
    if !properties.is_empty() {
        let _ = write!(out, "<Properties>{properties}</Properties>");
    }
    if !effects.lyrics.is_empty() {
        out.push_str("<Lyrics>");
        for line in 0..gtr_model::BeatEffects::MAX_LYRIC_LINES {
            let sung = effects.lyrics.get(line).map_or("", String::as_str);
            let _ = write!(out, "<Line>{}</Line>", esc(sung));
        }
        out.push_str("</Lyrics>");
    }
    out.push_str("</Beat>\n");
    lists.beats.push_str(&out);
    ids.push(id.to_string());
}

/// Writes a note and gives its number. `around` says whether it is hammered on to and
/// whether it is tied on from.
fn note_xml(
    at: &Place,
    note: &Note,
    (hammered, tied_on): (bool, bool),
    lists: &mut Lists,
) -> usize {
    let effects = &note.effects;
    let staff = at.staff;
    let drums = at.track.instrument.is_percussion();
    let id = lists.note_count;
    lists.note_count += 1;
    let mut out = format!("<Note id=\"{id}\">");
    let articulation = match drums {
        true => at
            .part
            .kit
            .iter()
            .position(|n| *n == note.fret)
            .unwrap_or(0),
        false => 0,
    };
    let _ = write!(
        out,
        "<InstrumentArticulation>{articulation}</InstrumentArticulation>"
    );
    if (note.tied || tied_on) && !drums {
        let _ = write!(
            out,
            "<Tie origin=\"{tied_on}\" destination=\"{}\"/>",
            note.tied
        );
    }
    match effects.vibrato {
        Some(Vibrato::Slight) => out.push_str("<Vibrato>Slight</Vibrato>"),
        Some(Vibrato::Wide) => out.push_str("<Vibrato>Wide</Vibrato>"),
        None => {}
    }
    if effects.let_ring {
        out.push_str("<LetRing/>");
    }
    if effects.ghost {
        out.push_str("<AntiAccent>Normal</AntiAccent>");
    }
    let accent = u8::from(effects.staccato)
        | match effects.accent {
            Some(Accent::Heavy) => 0x04,
            Some(Accent::Normal) => 0x08,
            None => 0,
        };
    if accent != 0 {
        let _ = write!(out, "<Accent>{accent}</Accent>");
    }
    let open = staff.tuning.pitch(note.string, 0, staff.capo);
    if let (Some(trill), Some(open)) = (effects.trill, open) {
        let _ = write!(
            out,
            "<Trill>{}</Trill>",
            u32::from(open) + u32::from(trill.fret)
        );
    }
    let finger = |finger: Finger| match finger {
        Finger::Thumb => "P",
        Finger::Index => "I",
        Finger::Middle => "M",
        Finger::Ring => "A",
        Finger::Pinky => "C",
    };
    if let Some(left) = effects.left_finger {
        let _ = write!(out, "<LeftFingering>{}</LeftFingering>", finger(left));
    }
    if let Some(right) = effects.right_finger {
        let _ = write!(out, "<RightFingering>{}</RightFingering>", finger(right));
    }

    out.push_str("<Properties>");
    let mut property = |name: &str, inner: &str| {
        let _ = write!(out, "<Property name=\"{name}\">{inner}</Property>");
    };
    let enable = "<Enable/>";
    if let Some(bend) = &effects.bend {
        let shape = curve(bend);
        property("Bended", enable);
        let float = |value: f64| format!("<Float>{value:.6}</Float>");
        property("BendOriginValue", &float(shape.origin.1));
        property("BendOriginOffset", &float(shape.origin.0));
        if let Some((from, to, level)) = shape.middle {
            property("BendMiddleValue", &float(level));
            property("BendMiddleOffset1", &float(from));
            property("BendMiddleOffset2", &float(to));
        }
        property("BendDestinationValue", &float(shape.destination.1));
        property("BendDestinationOffset", &float(shape.destination.0));
    }
    // Strings are counted from the lowest.
    let strings = staff.tuning.strings.len();
    let from_low = strings.saturating_sub(usize::from(note.string) + 1);
    if drums {
        // A piece of the kit is its articulation and nothing else.
    } else if staff.is_pitched() {
        property("Octave", &format!("<Number>{}</Number>", note.string / 12));
        property("Tone", &format!("<Step>{}</Step>", note.string % 12));
        property("Midi", &format!("<Number>{}</Number>", note.string));
    } else {
        property("Fret", &format!("<Fret>{}</Fret>", note.fret));
        if let Some(pitch) = staff.pitch(note.string, note.fret) {
            property("Midi", &format!("<Number>{pitch}</Number>"));
        }
        property("String", &format!("<String>{from_low}</String>"));
    }
    if let Some(harmonic) = effects.harmonic {
        let kind = match harmonic {
            Harmonic::Natural => "Natural",
            Harmonic::Artificial => "Artificial",
            Harmonic::Pinch => "Pinch",
            Harmonic::Tapped => "Tap",
            Harmonic::Semi => "Semi",
        };
        // Where the string is touched: for a natural harmonic that is the note's fret.
        let touch = match harmonic {
            Harmonic::Natural => note.fret,
            _ => effects.harmonic_touch.unwrap_or(harmonic.usual_touch()),
        };
        property("Harmonic", enable);
        property("HarmonicFret", &format!("<HFret>{touch}</HFret>"));
        property("HarmonicType", &format!("<HType>{kind}</HType>"));
    }
    if hammered {
        property("HopoDestination", enable);
    }
    if effects.hammer {
        property("HopoOrigin", enable);
    }
    if effects.dead {
        property("Muted", enable);
    }
    if effects.palm_mute {
        property("PalmMuted", enable);
    }
    if let Some(slide) = effects.slide {
        let flags = match slide {
            Slide::Shift => 0x01,
            Slide::Legato => 0x02,
            Slide::OutDown => 0x04,
            Slide::OutUp => 0x08,
            Slide::InFromBelow => 0x10,
            Slide::InFromAbove => 0x20,
        };
        property("Slide", &format!("<Flags>{flags}</Flags>"));
    }
    if effects.tapped {
        property("Tapped", enable);
    }
    out.push_str("</Properties>");
    if let Some(trill) = effects.trill {
        // How long each note of the trill lasts, with 480 to the quarter note.
        let ticks = match trill.speed {
            NoteValue::SixtyFourth => 30,
            NoteValue::ThirtySecond => 60,
            _ => 120,
        };
        let _ = write!(
            out,
            "<XProperties><XProperty id=\"688062467\"><Int>{ticks}</Int></XProperty></XProperties>"
        );
    }
    out.push_str("</Note>\n");
    lists.notes.push_str(&out);
    id
}
