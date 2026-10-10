//! JavaScript API over the core crates. Structured data crosses the boundary as JSON.

use gtr_audio::Options;
use gtr_edit::{Editor, Move, Technique};
use gtr_io::{guitar_pro, native};
use gtr_model::{NoteValue, Score, ScoreInfo, TimeSignature, Tuning, Tuplet, demo_score};
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;

/// An open score together with its cursor, selection, clipboard and undo history.
#[wasm_bindgen]
pub struct Document {
    editor: Editor,
    import_report: String,
}

/// Audio rendered for playback, with the times at which each beat sounds.
#[wasm_bindgen]
pub struct RenderedAudio {
    samples: Vec<f32>,
    timeline_json: String,
}

#[wasm_bindgen]
impl RenderedAudio {
    /// Mono samples. Can be taken once; later calls return an empty array.
    #[wasm_bindgen(js_name = takeSamples)]
    pub fn take_samples(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.samples)
    }

    /// JSON `{ beats: [{ bar, beat, time }], duration }`, times in seconds.
    #[wasm_bindgen(getter, js_name = timelineJson)]
    pub fn timeline_json(&self) -> String {
        self.timeline_json.clone()
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
        Ok(Document::from_score(native::load(bytes)?))
    }

    /// Imports a file written by another program. Guitar Pro 5 (`.gp5`) and Guitar Pro 7/8
    /// (`.gp`) are supported; `name` is only used in error messages.
    #[wasm_bindgen(js_name = importFile)]
    pub fn import_file(name: &str, bytes: &[u8]) -> Result<Document, JsError> {
        let imported = guitar_pro::import(bytes)
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
        Ok(native::save(self.editor.score())?)
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

    /// Display list for the cursor's track, wrapped to `width` pixels.
    #[wasm_bindgen(js_name = layoutJson)]
    pub fn layout_json(&self, width: f64) -> Result<String, JsError> {
        let layout =
            gtr_layout::layout_track(self.editor.score(), self.editor.cursor().track, width);
        Ok(serde_json::to_string(&layout)?)
    }

    /// Renders audio for playback. `options_json` holds `gtr_audio::Options` fields.
    #[wasm_bindgen(js_name = renderAudio)]
    pub fn render_audio(
        &self,
        options_json: &str,
        sample_rate: u32,
    ) -> Result<RenderedAudio, JsError> {
        let options: Options = serde_json::from_str(options_json)?;
        let timeline =
            gtr_audio::compile(self.editor.score(), self.editor.cursor().track, &options);
        Ok(RenderedAudio {
            samples: gtr_audio::render(&timeline, sample_rate),
            timeline_json: serde_json::to_string(&serde_json::json!({
                "beats": timeline.beats,
                "duration": timeline.duration,
            }))?,
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

    #[wasm_bindgen(js_name = setTrack)]
    pub fn set_track(&mut self, track: usize) {
        self.editor.set_track(track);
    }

    // --- Editing ---

    #[wasm_bindgen(js_name = typeDigit)]
    pub fn type_digit(&mut self, digit: u8, combine: bool) -> Result<(), JsError> {
        Ok(self.editor.type_digit(digit, combine)?)
    }

    pub fn rest(&mut self) -> Result<(), JsError> {
        Ok(self.editor.rest()?)
    }

    #[wasm_bindgen(js_name = toggleTie)]
    pub fn toggle_tie(&mut self) -> Result<(), JsError> {
        Ok(self.editor.toggle_tie()?)
    }

    /// `value` is one of whole, half, quarter, eighth, sixteenth, thirty_second, sixty_fourth.
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

    #[wasm_bindgen(js_name = setTempo)]
    pub fn set_tempo(&mut self, tempo: u32) -> Result<(), JsError> {
        Ok(self.editor.set_tempo(tempo)?)
    }

    /// `strings` are open-string MIDI pitches from the highest string to the lowest.
    #[wasm_bindgen(js_name = setTuning)]
    pub fn set_tuning(&mut self, strings: Vec<u8>, capo: u8) -> Result<(), JsError> {
        Ok(self.editor.set_tuning(Tuning::new(strings), capo)?)
    }

    #[wasm_bindgen(js_name = setInfo)]
    pub fn set_info(
        &mut self,
        title: String,
        artist: String,
        album: String,
    ) -> Result<(), JsError> {
        Ok(self.editor.set_info(ScoreInfo {
            title,
            artist,
            album,
        })?)
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
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Document::new()
    }
}
