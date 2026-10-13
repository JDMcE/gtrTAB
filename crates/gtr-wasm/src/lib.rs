//! JavaScript API over the core crates. Structured data crosses the boundary as JSON.

use gtr_audio::Options;
use gtr_edit::{Editor, Move, Technique};
use gtr_io::native;
use gtr_model::{
    Instrument, Mix, NoteValue, Pedal, PedalKind, Score, ScoreInfo, TimeSignature, Tuning, Tuplet,
    demo_score,
};
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;

/// An open score together with its cursor, selection, clipboard and undo history.
#[wasm_bindgen]
pub struct Document {
    editor: Editor,
    import_report: String,
    /// The files of the recordings the score has had, by name, kept outside the score
    /// and its history: an import undone and done again finds its recording here.
    recordings: std::collections::HashMap<String, Vec<u8>>,
}

/// A passage ready to be played.
#[wasm_bindgen]
pub struct Passage {
    events: Vec<u8>,
    timeline_json: String,
}

#[wasm_bindgen]
impl Passage {
    /// The passage as `gtr_audio::Timeline` bytes, for the synth in the AudioWorklet.
    #[wasm_bindgen(getter)]
    pub fn events(&self) -> Vec<u8> {
        self.events.clone()
    }

    /// JSON `{ beats: [{ bar, beat, time }], duration }`, times in seconds.
    #[wasm_bindgen(getter, js_name = timelineJson)]
    pub fn timeline_json(&self) -> String {
        self.timeline_json.clone()
    }
}

/// A score being made into sound for a file, a piece at a time so that the page that asked
/// for it is not held up until the whole of it is done.
#[wasm_bindgen]
pub struct AudioExport {
    engine: gtr_audio::Engine,
    left: Vec<f32>,
    right: Vec<f32>,
    sample_rate: u32,
}

#[wasm_bindgen]
impl AudioExport {
    /// Makes up to `frames` more samples. Returns how much of the piece is done, from 0
    /// to 1.
    pub fn step(&mut self, frames: usize) -> f64 {
        let total = self.engine.length();
        let from = self.left.len();
        let to = (from + frames).min(total);
        self.left.resize(to, 0.0);
        self.right.resize(to, 0.0);
        (self.engine).process(&mut self.left[from..], &mut self.right[from..]);
        match total {
            0 => 1.0,
            _ => to as f64 / total as f64,
        }
    }

    /// What has been made so far, as the bytes of a WAV file.
    pub fn wav(&self) -> Vec<u8> {
        gtr_audio::wav(&self.left, &self.right, self.sample_rate)
    }
}

/// A score written as another program's file.
#[wasm_bindgen]
pub struct Exported {
    bytes: Vec<u8>,
    extension: &'static str,
    media_type: &'static str,
    report: String,
}

#[wasm_bindgen]
impl Exported {
    #[wasm_bindgen(getter)]
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    /// How a file of this kind ends its name, without the dot.
    #[wasm_bindgen(getter)]
    pub fn extension(&self) -> String {
        self.extension.to_owned()
    }

    #[wasm_bindgen(getter, js_name = mediaType)]
    pub fn media_type(&self) -> String {
        self.media_type.to_owned()
    }

    /// Comma-separated kinds of content the format has no place for; empty if it has a
    /// place for everything in the score.
    #[wasm_bindgen(getter)]
    pub fn report(&self) -> String {
        self.report.clone()
    }
}

/// The paper a name stands for.
fn page(paper: &str) -> gtr_layout::Page {
    match paper {
        "letter" => gtr_layout::Page::LETTER,
        _ => gtr_layout::Page::A4,
    }
}

fn tuplet(actual: u8, normal: u8) -> Option<Tuplet> {
    Some(Tuplet {
        actual: actual.try_into().ok()?,
        normal: normal.try_into().ok()?,
    })
}

/// Parses a snake_case enum name such as `"eighth"` or `"prev_bar"`.
fn parse_name<T: DeserializeOwned>(name: &str) -> Result<T, JsError> {
    Ok(serde_json::from_value(serde_json::Value::String(
        name.to_owned(),
    ))?)
}

#[wasm_bindgen]
impl Document {
    /// Creates a blank score.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Document {
        Document::from_score(Score::blank())
    }

    /// Creates a document holding the demo riff.
    pub fn demo() -> Document {
        Document::from_score(demo_score())
    }

    /// Opens a native `.gtrtab` file.
    pub fn load(bytes: &[u8]) -> Result<Document, JsError> {
        let (score, recording) = native::load_with(bytes)?;
        let name = score.audio.as_ref().map(|a| a.name.clone());
        let mut document = Document::from_score(score);
        if let (Some(name), Some(recording)) = (name, recording) {
            document.recordings.insert(name, recording);
        }
        Ok(document)
    }

    /// Imports a file written by another program: Guitar Pro of any version, MusicXML,
    /// MIDI, or tablature typed as text. What kind it is, is worked out from its
    /// contents, and from `name` where they do not say.
    #[wasm_bindgen(js_name = importFile)]
    pub fn import_file(name: &str, bytes: &[u8]) -> Result<Document, JsError> {
        let imported = gtr_io::import(name, bytes)
            .map_err(|e| JsError::new(&format!("Could not import {name}: {e}")))?;
        let mut document = Document::from_score(imported.score);
        document.import_report = imported.skipped.join(", ");
        Ok(document)
    }

    /// Comma-separated kinds of content the import left out; empty if nothing was.
    #[wasm_bindgen(getter, js_name = importReport)]
    pub fn import_report(&self) -> String {
        self.import_report.clone()
    }

    /// Serialises the score as a native `.gtrtab` file.
    pub fn save(&self) -> Result<Vec<u8>, JsError> {
        Ok(native::save_with(
            self.editor.score(),
            self.recording().map(Vec::as_slice),
        )?)
    }

    /// Saves the score without its recording, which can be large, as for keeping a copy
    /// of the work in case the app closes before it is saved.
    #[wasm_bindgen(js_name = saveWithoutRecording)]
    pub fn save_without_recording(&self) -> Result<Vec<u8>, JsError> {
        Ok(native::save(self.editor.score())?)
    }

    /// Gives the score an audio track playing the file `name`, its first bar at the
    /// recording's start and the rest as written, in place of any it had. `bytes` is the
    /// file as it is, which is saved with the score.
    #[wasm_bindgen(js_name = importRecording)]
    pub fn import_recording(&mut self, name: &str, bytes: Vec<u8>) -> Result<(), JsError> {
        let name: String = name.chars().take(gtr_model::AudioTrack::MAX_NAME).collect();
        self.recordings.insert(name.clone(), bytes);
        Ok(self
            .editor
            .set_audio(Some(gtr_model::AudioTrack::new(name)))?)
    }

    /// The file of the score's recording, if it has one and the file is at hand.
    #[wasm_bindgen(js_name = recordingFile)]
    pub fn recording_file(&self) -> Option<Vec<u8>> {
        self.recording().cloned()
    }

    /// Changes the audio track's mix or sync points, or with `"null"` takes it away.
    /// `audio_json` holds a `gtr_model::AudioTrack`.
    #[wasm_bindgen(js_name = setAudio)]
    pub fn set_audio(&mut self, audio_json: &str) -> Result<(), JsError> {
        let audio: Option<gtr_model::AudioTrack> = serde_json::from_str(audio_json)?;
        Ok(self.editor.set_audio(audio)?)
    }

    /// Writes the score as another program's file. `format` is one of `guitar_pro`,
    /// `musicxml`, `midi` and `ascii`.
    #[wasm_bindgen(js_name = exportFile)]
    pub fn export_file(&self, format: &str) -> Result<Exported, JsError> {
        let format = match format {
            "guitar_pro" => gtr_io::Format::GuitarPro,
            "musicxml" => gtr_io::Format::MusicXml,
            "midi" => gtr_io::Format::Midi,
            "ascii" => gtr_io::Format::Ascii,
            other => {
                return Err(JsError::new(&format!(
                    "{other} is not a format to export to"
                )));
            }
        };
        let written = gtr_io::export(self.editor.score(), format)?;
        Ok(Exported {
            bytes: written.bytes,
            extension: format.extension(),
            media_type: format.media_type(),
            report: written.skipped.join(", "),
        })
    }

    /// The score laid out for paper or a picture, `width` pixels wide, as `view_json`
    /// asks; see [`Document::layout_json`].
    fn printed(&self, width: f64, view_json: &str) -> Result<gtr_layout::Layout, JsError> {
        let cursor = self.editor.cursor();
        let view = gtr_layout::View {
            track: cursor.track,
            staff: cursor.staff,
            voice: cursor.voice,
            ..serde_json::from_str(view_json)?
        };
        Ok(gtr_layout::layout(self.editor.score(), &view, width).printed())
    }

    /// The score as a PDF, on paper of the size `paper` names: `letter`, or else A4. The
    /// fonts are the bytes of the music font and of the two faces of the text font.
    #[wasm_bindgen(js_name = exportPdf)]
    pub fn export_pdf(
        &self,
        view_json: &str,
        paper: &str,
        music: &[u8],
        text: &[u8],
        fret: &[u8],
    ) -> Result<Vec<u8>, JsError> {
        let fonts = gtr_render::Fonts::parse(music, text, fret)?;
        let page = page(paper);
        let pages = self.printed(page.width, view_json)?.pages(page);
        let title = &self.editor.score().info.title;
        Ok(gtr_render::pdf(&pages, &fonts, title))
    }

    /// The pages of the score as SVG documents drawn in outlines, for printing: a JSON
    /// array of strings.
    #[wasm_bindgen(js_name = pagesJson)]
    pub fn pages_json(
        &self,
        view_json: &str,
        paper: &str,
        music: &[u8],
        text: &[u8],
        fret: &[u8],
    ) -> Result<String, JsError> {
        let fonts = gtr_render::Fonts::parse(music, text, fret)?;
        let page = page(paper);
        let options = gtr_render::SvgOptions {
            fonts: Some(&fonts),
            ..gtr_render::SvgOptions::default()
        };
        let pages: Vec<String> = (self.printed(page.width, view_json)?.pages(page).iter())
            .map(|page| gtr_render::svg(page, options))
            .collect();
        Ok(serde_json::to_string(&pages)?)
    }

    /// The whole score as one SVG picture, `width` pixels wide, drawn in outlines so
    /// that it needs no fonts where it is shown.
    #[wasm_bindgen(js_name = exportSvg)]
    pub fn export_svg(
        &self,
        view_json: &str,
        width: f64,
        music: &[u8],
        text: &[u8],
        fret: &[u8],
    ) -> Result<String, JsError> {
        let fonts = gtr_render::Fonts::parse(music, text, fret)?;
        let options = gtr_render::SvgOptions {
            fonts: Some(&fonts),
            ..gtr_render::SvgOptions::default()
        };
        Ok(gtr_render::svg(&self.printed(width, view_json)?, options))
    }

    /// The whole score as one PNG picture: a layout `width` pixels wide, drawn with
    /// `scale` pixels of the picture to each of those.
    #[wasm_bindgen(js_name = exportPng)]
    pub fn export_png(
        &self,
        view_json: &str,
        width: f64,
        scale: f64,
        music: &[u8],
        text: &[u8],
        fret: &[u8],
    ) -> Result<Vec<u8>, JsError> {
        let fonts = gtr_render::Fonts::parse(music, text, fret)?;
        Ok(gtr_render::png(
            &self.printed(width, view_json)?,
            &fonts,
            scale,
        ))
    }

    /// Records that the current state has been written to disk.
    #[wasm_bindgen(js_name = markSaved)]
    pub fn mark_saved(&mut self) {
        self.editor.mark_saved();
    }

    /// Records that the score has unsaved changes, as after an import or a recovery.
    #[wasm_bindgen(js_name = markUnsaved)]
    pub fn mark_unsaved(&mut self) {
        self.editor.mark_unsaved();
    }

    #[wasm_bindgen(js_name = scoreJson)]
    pub fn score_json(&self) -> Result<String, JsError> {
        Ok(serde_json::to_string(self.editor.score())?)
    }

    /// Cursor, selection, input duration and undo/dirty flags.
    #[wasm_bindgen(js_name = stateJson)]
    pub fn state_json(&self) -> Result<String, JsError> {
        Ok(serde_json::to_string(&self.editor.state())?)
    }

    /// Display list for the score, wrapped to `width` pixels. `view_json` holds the
    /// `notation` and `multitrack` fields of `gtr_layout::View`; the track and voice shown
    /// are the cursor's.
    #[wasm_bindgen(js_name = layoutJson)]
    pub fn layout_json(&self, width: f64, view_json: &str) -> Result<String, JsError> {
        let cursor = self.editor.cursor();
        let view = gtr_layout::View {
            track: cursor.track,
            staff: cursor.staff,
            voice: cursor.voice,
            ..serde_json::from_str(view_json)?
        };
        let layout = gtr_layout::layout(self.editor.score(), &view, width);
        Ok(serde_json::to_string(&layout)?)
    }

    /// Compiles the score to the events the synth plays. `options_json` holds
    /// `gtr_audio::Options` fields; `follow` is the track whose beats move the playback
    /// cursor.
    #[wasm_bindgen(js_name = compileAudio)]
    pub fn compile_audio(&self, options_json: &str, follow: usize) -> Result<Passage, JsError> {
        let options: Options = serde_json::from_str(options_json)?;
        let timeline = gtr_audio::compile(self.editor.score(), follow, &options);
        Ok(Passage {
            timeline_json: serde_json::to_string(&serde_json::json!({
                "beats": timeline.beats,
                "pulses": timeline.pulses,
                "duration": timeline.duration,
            }))?,
            events: timeline.to_bytes(),
        })
    }

    /// A few notes to hear on their own, as when a chord or a scale is tried out: `pitches`
    /// on `instrument`, `gap` seconds apart, each `length` seconds long. See
    /// [`gtr_audio::audition`].
    pub fn audition(
        instrument: &str,
        pitches: Vec<u8>,
        gap: f64,
        length: f64,
    ) -> Result<Passage, JsError> {
        let instrument: Instrument = parse_name(instrument)?;
        let timeline = gtr_audio::audition(instrument, &pitches, gap, length);
        Ok(Passage {
            timeline_json: serde_json::to_string(&serde_json::json!({
                "beats": timeline.beats,
                "pulses": timeline.pulses,
                "duration": timeline.duration,
            }))?,
            events: timeline.to_bytes(),
        })
    }

    /// Starts making the whole score into sound for a file, as it plays: every track
    /// that is not muted, through the SoundFont whose bytes are given or through the
    /// built-in models if none is. `options_json` is as for [`Document::compile_audio`].
    #[wasm_bindgen(js_name = exportAudio)]
    pub fn export_audio(
        &self,
        options_json: &str,
        sample_rate: u32,
        soundfont: Option<Vec<u8>>,
    ) -> Result<AudioExport, JsError> {
        let options: Options = serde_json::from_str(options_json)?;
        let font = match soundfont {
            Some(bytes) => Some(
                gtr_audio::SoundFont::parse(&bytes)
                    .map_err(|e| JsError::new(&format!("The SoundFont could not be used: {e}")))?,
            ),
            None => None,
        };
        let sample_rate = sample_rate.clamp(8_000, 192_000);
        let mut engine = gtr_audio::Engine::new(sample_rate);
        engine.set_soundfont(font);
        engine.load(gtr_audio::compile(self.editor.score(), 0, &options));
        Ok(AudioExport {
            engine,
            left: Vec::new(),
            right: Vec::new(),
            sample_rate,
        })
    }

    // --- Cursor ---

    /// `direction` is one of left, right, up, down, prev_bar, next_bar, first, last.
    #[wasm_bindgen(js_name = moveCursor)]
    pub fn move_cursor(&mut self, direction: &str, extend: bool) -> Result<(), JsError> {
        let direction: Move = parse_name(direction)?;
        Ok(self.editor.move_cursor(direction, extend)?)
    }

    #[wasm_bindgen(js_name = setCursor)]
    pub fn set_cursor(&mut self, bar: usize, beat: usize, string: u8, extend: bool) {
        self.editor.set_cursor(bar, beat, string, extend);
    }

    /// Moves the cursor to the start of a bar, staying on its string or pitch.
    #[wasm_bindgen(js_name = goToBar)]
    pub fn go_to_bar(&mut self, bar: usize) {
        self.editor.go_to_bar(bar);
    }

    #[wasm_bindgen(js_name = setTrack)]
    pub fn set_track(&mut self, track: usize) {
        self.editor.set_track(track);
    }

    /// Switches to another staff of the cursor's track, counted from the top.
    #[wasm_bindgen(js_name = setStaff)]
    pub fn set_staff(&mut self, staff: usize) {
        self.editor.set_staff(staff);
    }

    /// Switches to another voice of the staff, counted from zero.
    #[wasm_bindgen(js_name = setVoice)]
    pub fn set_voice(&mut self, voice: usize) {
        self.editor.set_voice(voice);
    }

    // --- Editing ---

    #[wasm_bindgen(js_name = typeDigit)]
    pub fn type_digit(&mut self, digit: u8, combine: bool) -> Result<(), JsError> {
        Ok(self.editor.type_digit(digit, combine)?)
    }

    /// Puts a note on a fret under the cursor in one go; on a drum track the fret is the
    /// General MIDI number of a piece of the kit.
    #[wasm_bindgen(js_name = setFret)]
    pub fn set_fret(&mut self, fret: u8) -> Result<(), JsError> {
        Ok(self.editor.set_fret(fret)?)
    }

    /// The pieces of the standard drum kit as JSON: `gtr_model::KitPiece` values.
    /// Puts a chord shape on the beat at the cursor in one edit; see
    /// [`gtr_edit::Editor::put_shape`].
    #[wasm_bindgen(js_name = putShape)]
    pub fn put_shape(&mut self, frets: Vec<i8>, name: Option<String>) -> Result<(), JsError> {
        Ok(self.editor.put_shape(&frets, name.as_deref())?)
    }

    #[wasm_bindgen(js_name = drumKitJson)]
    pub fn drum_kit_json() -> Result<String, JsError> {
        Ok(serde_json::to_string(&gtr_model::standard_kit())?)
    }

    /// Gives a drum track a kit of its own. `kit_json` holds `gtr_model::KitPiece` values;
    /// an empty list gives the standard kit back.
    #[wasm_bindgen(js_name = setDrumKit)]
    pub fn set_drum_kit(&mut self, track: usize, kit_json: &str) -> Result<(), JsError> {
        let kit: Vec<gtr_model::KitPiece> = serde_json::from_str(kit_json)?;
        Ok(self.editor.set_kit(track, kit)?)
    }

    /// On a staff without strings, enters the note named by `letter`, from 0 for C to 6
    /// for B, in the octave nearest the cursor.
    #[wasm_bindgen(js_name = enterLetter)]
    pub fn enter_letter(&mut self, letter: u8) -> Result<(), JsError> {
        Ok(self.editor.enter_letter(letter)?)
    }

    /// On a staff without strings, puts a note at the cursor's pitch.
    #[wasm_bindgen(js_name = enterNote)]
    pub fn enter_note(&mut self) -> Result<(), JsError> {
        Ok(self.editor.enter_note()?)
    }

    /// On a staff without strings, moves the note under the cursor, or just the cursor,
    /// by `semitones`.
    #[wasm_bindgen(js_name = shiftPitch)]
    pub fn shift_pitch(&mut self, semitones: i8) -> Result<(), JsError> {
        Ok(self.editor.shift_pitch(semitones)?)
    }

    pub fn rest(&mut self) -> Result<(), JsError> {
        Ok(self.editor.rest()?)
    }

    #[wasm_bindgen(js_name = toggleTie)]
    pub fn toggle_tie(&mut self) -> Result<(), JsError> {
        Ok(self.editor.toggle_tie()?)
    }

    /// `value` is one of whole, half, quarter, eighth, sixteenth, thirty_second, sixty_fourth,
    /// hundred_twenty_eighth, two_hundred_fifty_sixth.
    #[wasm_bindgen(js_name = setNoteValue)]
    pub fn set_note_value(&mut self, value: &str) -> Result<(), JsError> {
        let value: NoteValue = parse_name(value)?;
        Ok(self.editor.set_note_value(value)?)
    }

    pub fn shorter(&mut self) -> Result<(), JsError> {
        Ok(self.editor.shorter()?)
    }

    pub fn longer(&mut self) -> Result<(), JsError> {
        Ok(self.editor.longer()?)
    }

    #[wasm_bindgen(js_name = toggleDot)]
    pub fn toggle_dot(&mut self) -> Result<(), JsError> {
        Ok(self.editor.toggle_dot()?)
    }

    #[wasm_bindgen(js_name = toggleTriplet)]
    pub fn toggle_triplet(&mut self) -> Result<(), JsError> {
        Ok(self.editor.toggle_triplet()?)
    }

    /// Switches a playing technique on or off, e.g. `"palm_mute"`, `"hammer"` or
    /// `"slide_shift"`. See `gtr_edit::Technique` for the full list.
    #[wasm_bindgen(js_name = toggleTechnique)]
    pub fn toggle_technique(&mut self, technique: &str) -> Result<(), JsError> {
        let technique: Technique = parse_name(technique)?;
        Ok(self.editor.toggle_technique(technique)?)
    }

    /// Sets one effect of the note under the cursor, or of every selected note. `field` is
    /// the effect's name in the saved file, such as `"bend"`, `"trill"` or `"left_finger"`;
    /// `value_json` is its JSON, with `null` removing it.
    #[wasm_bindgen(js_name = setNoteEffect)]
    pub fn set_note_effect(&mut self, field: &str, value_json: &str) -> Result<(), JsError> {
        let value = serde_json::from_str(value_json)?;
        Ok(self.editor.set_note_effect(field, &value)?)
    }

    /// Sets one effect of the beat under the cursor, or of every selected beat, such as
    /// `"dynamic"` or `"whammy"`. See [`Self::set_note_effect`].
    #[wasm_bindgen(js_name = setBeatEffect)]
    pub fn set_beat_effect(&mut self, field: &str, value_json: &str) -> Result<(), JsError> {
        let value = serde_json::from_str(value_json)?;
        Ok(self.editor.set_beat_effect(field, &value)?)
    }

    /// Sets a tuplet of `actual` notes in the time of `normal`; zero removes it.
    #[wasm_bindgen(js_name = setTuplet)]
    pub fn set_tuplet(&mut self, actual: u8, normal: u8) -> Result<(), JsError> {
        Ok(self.editor.set_tuplet(tuplet(actual, normal))?)
    }

    /// Sets a second tuplet inside the first; zero removes it.
    #[wasm_bindgen(js_name = setNestedTuplet)]
    pub fn set_nested_tuplet(&mut self, actual: u8, normal: u8) -> Result<(), JsError> {
        Ok(self.editor.set_nested_tuplet(tuplet(actual, normal))?)
    }

    pub fn delete(&mut self) -> Result<(), JsError> {
        Ok(self.editor.delete()?)
    }

    #[wasm_bindgen(js_name = deleteBeats)]
    pub fn delete_beats(&mut self) -> Result<(), JsError> {
        Ok(self.editor.delete_beats()?)
    }

    #[wasm_bindgen(js_name = insertBeat)]
    pub fn insert_beat(&mut self) -> Result<(), JsError> {
        Ok(self.editor.insert_beat()?)
    }

    #[wasm_bindgen(js_name = insertBar)]
    pub fn insert_bar(&mut self) -> Result<(), JsError> {
        Ok(self.editor.insert_bar()?)
    }

    #[wasm_bindgen(js_name = removeBar)]
    pub fn remove_bar(&mut self) -> Result<(), JsError> {
        Ok(self.editor.remove_bar()?)
    }

    #[wasm_bindgen(js_name = setTimeSignature)]
    pub fn set_time_signature(&mut self, numerator: u8, denominator: u8) -> Result<(), JsError> {
        Ok(self.editor.set_time_signature(TimeSignature {
            numerator,
            denominator,
        })?)
    }

    /// Sets one shared property of the cursor's bar, such as `"repeat_start"`,
    /// `"repeat_count"`, `"alternate_endings"`, `"section"`, `"targets"`, `"jumps"`,
    /// `"tempo"` or `"feel"`. `value_json` is its JSON, with `null` clearing it.
    #[wasm_bindgen(js_name = setBarProperty)]
    pub fn set_bar_property(&mut self, field: &str, value_json: &str) -> Result<(), JsError> {
        let value = serde_json::from_str(value_json)?;
        Ok(self.editor.set_bar_property(field, &value)?)
    }

    /// Sets the key signature from the cursor's bar: `key` sharps, or flats if negative.
    #[wasm_bindgen(js_name = setKeySignature)]
    pub fn set_key_signature(&mut self, key: i8) -> Result<(), JsError> {
        Ok(self.editor.set_key_signature(key)?)
    }

    /// Changes the tempo from the cursor's beat on; zero takes away the change there.
    #[wasm_bindgen(js_name = setTempoChange)]
    pub fn set_tempo_change(&mut self, tempo: u32, gradual: bool) -> Result<(), JsError> {
        Ok(self
            .editor
            .set_tempo_change((tempo > 0).then_some(tempo), gradual)?)
    }

    #[wasm_bindgen(js_name = setTempo)]
    pub fn set_tempo(&mut self, tempo: u32) -> Result<(), JsError> {
        Ok(self.editor.set_tempo(tempo)?)
    }

    /// `strings` are open-string MIDI pitches from the highest string to the lowest.
    #[wasm_bindgen(js_name = setTuning)]
    pub fn set_tuning(&mut self, strings: Vec<u8>, capo: u8) -> Result<(), JsError> {
        Ok(self.editor.set_tuning(Tuning::new(strings), capo)?)
    }

    /// Sets the title, credits and copyright. `info_json` holds `gtr_model::ScoreInfo`
    /// fields; the ones left out are cleared.
    #[wasm_bindgen(js_name = setInfo)]
    pub fn set_info(&mut self, info_json: &str) -> Result<(), JsError> {
        let info: ScoreInfo = serde_json::from_str(info_json)?;
        Ok(self.editor.set_info(info)?)
    }

    // --- Tracks ---

    /// Adds an empty track below the cursor's track and moves to it. `instrument` is a
    /// `gtr_model::Instrument` name such as `"bass"` or `"distortion_guitar"`; `strings`
    /// are its open-string MIDI pitches from the highest string to the lowest.
    #[wasm_bindgen(js_name = addTrack)]
    pub fn add_track(
        &mut self,
        name: &str,
        instrument: &str,
        strings: Vec<u8>,
        capo: u8,
    ) -> Result<(), JsError> {
        let instrument: Instrument = parse_name(instrument)?;
        // Without strings the track gets the staves and tunings its instrument usually has.
        let tuning = (!strings.is_empty()).then(|| (Tuning::new(strings), capo));
        Ok(self.editor.add_track(name, instrument, tuning)?)
    }

    #[wasm_bindgen(js_name = removeTrack)]
    pub fn remove_track(&mut self, track: usize) -> Result<(), JsError> {
        Ok(self.editor.remove_track(track)?)
    }

    /// Moves a track to position `to` among the others.
    #[wasm_bindgen(js_name = moveTrack)]
    pub fn move_track(&mut self, from: usize, to: usize) -> Result<(), JsError> {
        Ok(self.editor.move_track(from, to)?)
    }

    #[wasm_bindgen(js_name = setTrackInfo)]
    pub fn set_track_info(
        &mut self,
        track: usize,
        name: &str,
        instrument: &str,
    ) -> Result<(), JsError> {
        let instrument: Instrument = parse_name(instrument)?;
        Ok(self.editor.set_track_info(track, name, instrument)?)
    }

    /// Sets a track's mixer settings. `mix_json` holds `gtr_model::Mix` fields; the ones
    /// left out keep their defaults.
    #[wasm_bindgen(js_name = setMix)]
    pub fn set_mix(&mut self, track: usize, mix_json: &str) -> Result<(), JsError> {
        let mix: Mix = serde_json::from_str(mix_json)?;
        Ok(self.editor.set_mix(track, mix)?)
    }

    /// Sets the pedals a track goes through, or with no track those of the whole mix.
    /// `effects_json` holds a list of `gtr_model::Pedal` values.
    #[wasm_bindgen(js_name = setEffects)]
    pub fn set_effects(&mut self, track: Option<usize>, effects_json: &str) -> Result<(), JsError> {
        let effects: Vec<Pedal> = serde_json::from_str(effects_json)?;
        Ok(self.editor.set_effects(track, effects)?)
    }

    /// Sets the other sounds a track can switch to. `sounds_json` holds a list of
    /// `gtr_model::Sound` values.
    #[wasm_bindgen(js_name = setSounds)]
    pub fn set_sounds(&mut self, track: usize, sounds_json: &str) -> Result<(), JsError> {
        let sounds: Vec<gtr_model::Sound> = serde_json::from_str(sounds_json)?;
        Ok(self.editor.set_sounds(track, sounds)?)
    }

    /// Takes away a track's other sound `index`, counted from 1, keeping the beats that
    /// switch to the others right.
    #[wasm_bindgen(js_name = removeSound)]
    pub fn remove_sound(&mut self, track: usize, index: u8) -> Result<(), JsError> {
        Ok(self.editor.remove_sound(track, index)?)
    }

    /// Every kind of pedal, in the order the synth numbers them, with its name and its
    /// knobs: `[{ kind, name, knobs: [{ name, default }] }]`.
    #[wasm_bindgen(js_name = pedalKindsJson)]
    pub fn pedal_kinds_json() -> String {
        let kinds: Vec<_> = (PedalKind::ALL.iter())
            .map(|kind| {
                let knobs: Vec<_> = (kind.knobs().iter())
                    .map(|k| serde_json::json!({ "name": k.name, "default": k.default }))
                    .collect();
                serde_json::json!({ "kind": kind, "name": kind.name(), "knobs": knobs })
            })
            .collect();
        serde_json::Value::from(kinds).to_string()
    }

    /// The chains the app comes with: `[{ name, master, pedals }]`, `master` true for one
    /// meant for the whole mix.
    #[wasm_bindgen(js_name = pedalPresetsJson)]
    pub fn pedal_presets_json() -> Result<String, JsError> {
        let presets: Vec<_> = (gtr_model::presets().into_iter())
            .map(|p| serde_json::json!({ "name": p.name, "master": p.master, "pedals": p.pedals }))
            .collect();
        Ok(serde_json::to_string(&presets)?)
    }

    /// A chain of pedals as the synth takes it while a knob is turned, from a list of
    /// `gtr_model::Pedal` values.
    #[wasm_bindgen(js_name = chainBytes)]
    pub fn chain_bytes(effects_json: &str) -> Result<Vec<u8>, JsError> {
        let effects: Vec<Pedal> = serde_json::from_str(effects_json)?;
        Ok(gtr_audio::chain_to_bytes(&effects))
    }

    // --- Clipboard and history ---

    pub fn copy(&mut self) {
        self.editor.copy();
    }

    pub fn cut(&mut self) -> Result<(), JsError> {
        Ok(self.editor.cut()?)
    }

    pub fn paste(&mut self) -> Result<(), JsError> {
        Ok(self.editor.paste()?)
    }

    pub fn undo(&mut self) -> Result<bool, JsError> {
        Ok(self.editor.undo()?)
    }

    pub fn redo(&mut self) -> Result<bool, JsError> {
        Ok(self.editor.redo()?)
    }
}

impl Document {
    fn from_score(score: Score) -> Document {
        Document {
            editor: Editor::new(score),
            import_report: String::new(),
            recordings: Default::default(),
        }
    }

    /// The file of the score's recording, if it has one and the file is at hand.
    fn recording(&self) -> Option<&Vec<u8>> {
        let audio = self.editor.score().audio.as_ref()?;
        self.recordings.get(&audio.name)
    }
}

impl Default for Document {
    fn default() -> Self {
        Document::new()
    }
}
