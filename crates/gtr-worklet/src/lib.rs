//! The synth as a WebAssembly module for an AudioWorklet. A worklet has no way to load the
//! glue code `wasm-bindgen` generates, so this is a plain C interface with no imports: the
//! worklet copies bytes and samples in and out of the module's memory itself.

use gtr_audio::{Engine, Rack, SoundFont, SoundFontError, Timeline, chain_from_bytes};

/// The synth and the memory it shares with the worklet.
pub struct Synth {
    engine: Engine,
    /// An encoded timeline or a SoundFont file on its way in.
    input: Vec<u8>,
    /// The block of samples last made.
    left: Vec<f32>,
    right: Vec<f32>,
}

/// Makes a synth. It lives for as long as the worklet does and is never freed.
#[unsafe(no_mangle)]
pub extern "C" fn gtr_new(sample_rate: u32) -> *mut Synth {
    Box::into_raw(Box::new(Synth {
        engine: Engine::new(sample_rate),
        input: Vec::new(),
        left: Vec::new(),
        right: Vec::new(),
    }))
}

/// Makes room for `length` bytes of timeline or SoundFont and says where to write them.
///
/// # Safety
/// `synth` must come from [`gtr_new`], as for every function here.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_input(synth: *mut Synth, length: usize) -> *mut u8 {
    let synth = unsafe { &mut *synth };
    synth.input.clear();
    synth.input.resize(length, 0);
    synth.input.as_mut_ptr()
}

/// Plays the timeline written to [`gtr_input`]: from its start, or if `keep_place` is set
/// from the beat now sounding. Returns 0 if the bytes are not a timeline.
///
/// # Safety
/// See [`gtr_input`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_load(synth: *mut Synth, keep_place: u32) -> u32 {
    let synth = unsafe { &mut *synth };
    let Some(timeline) = Timeline::from_bytes(&synth.input) else {
        return 0;
    };
    match keep_place {
        0 => synth.engine.load(timeline),
        _ => synth.engine.replace(timeline),
    }
    1
}

/// Has a looped passage go on to the timeline written to [`gtr_input`] the next time it
/// comes round, or with `clear` set to itself again. Returns 0 if the bytes are not a
/// timeline, and the queue is then left as it was.
///
/// # Safety
/// See [`gtr_input`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_queue(synth: *mut Synth, clear: u32) -> u32 {
    let synth = unsafe { &mut *synth };
    if clear != 0 {
        synth.engine.queue(None);
        return 1;
    }
    let Some(timeline) = Timeline::from_bytes(&synth.input) else {
        return 0;
    };
    synth.engine.queue(Some(timeline));
    1
}

/// Times a looped passage has gone back to its start.
///
/// # Safety
/// See [`gtr_input`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_passes(synth: *const Synth) -> u32 {
    unsafe { &*synth }.engine.passes()
}

/// 1 while a passage queued by [`gtr_queue`] waits for the loop to come round.
///
/// # Safety
/// See [`gtr_input`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_queued(synth: *const Synth) -> u32 {
    unsafe { &*synth }.engine.queued().into()
}

/// Plays from the SoundFont file written to [`gtr_input`] from the next note on. Returns 0
/// if it was one, and otherwise 1 for a file of another kind, 2 for a compressed SoundFont
/// and 3 for a damaged one; the synth then plays as it did before.
///
/// # Safety
/// See [`gtr_input`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_soundfont(synth: *mut Synth) -> u32 {
    let synth = unsafe { &mut *synth };
    let font = SoundFont::parse(&synth.input);
    // The file is big, and the samples have been copied out of it.
    synth.input = Vec::new();
    match font {
        Ok(font) => {
            synth.engine.set_soundfont(Some(font));
            0
        }
        Err(SoundFontError::NotASoundFont) => 1,
        Err(SoundFontError::Compressed) => 2,
        Err(SoundFontError::Malformed(_)) => 3,
    }
}

/// Goes back to playing from the built-in models.
///
/// # Safety
/// See [`gtr_input`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_no_soundfont(synth: *mut Synth) {
    unsafe { &mut *synth }.engine.set_soundfont(None);
}

/// Moves a track in the mix.
///
/// # Safety
/// See [`gtr_input`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_set_mix(
    synth: *mut Synth,
    channel: usize,
    audible: u32,
    gain: f32,
    pan: f32,
) {
    let synth = unsafe { &mut *synth };
    synth.engine.set_mix(channel, audible != 0, gain, pan);
}

/// Changes the pedals of a track while it plays, or with a `channel` below zero those of
/// the whole mix, to the chain written to [`gtr_input`]. Returns 0 if the bytes
/// are not a chain.
///
/// # Safety
/// See [`gtr_input`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_set_effects(synth: *mut Synth, channel: i32) -> u32 {
    let synth = unsafe { &mut *synth };
    let Some(chain) = chain_from_bytes(&synth.input) else {
        return 0;
    };
    let channel = usize::try_from(channel).ok();
    synth.engine.set_effects(channel, chain);
    1
}

/// Makes the next `frames` samples, to be read from [`gtr_left`] and [`gtr_right`].
/// Returns 0 once the passage is over.
///
/// # Safety
/// See [`gtr_input`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_process(synth: *mut Synth, frames: usize) -> u32 {
    let synth = unsafe { &mut *synth };
    synth.left.resize(frames, 0.0);
    synth.right.resize(frames, 0.0);
    synth.engine.process(&mut synth.left, &mut synth.right);
    (!synth.engine.finished()).into()
}

/// # Safety
/// See [`gtr_input`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_left(synth: *const Synth) -> *const f32 {
    unsafe { &*synth }.left.as_ptr()
}

/// # Safety
/// See [`gtr_input`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_right(synth: *const Synth) -> *const f32 {
    unsafe { &*synth }.right.as_ptr()
}

/// Seconds of the timeline played so far.
///
/// # Safety
/// See [`gtr_input`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_seconds(synth: *const Synth) -> f64 {
    unsafe { &*synth }.engine.seconds()
}

/// A line in played through pedals, and the memory it shares with its worklet.
pub struct Line {
    rack: Rack,
    /// A chain on its way in.
    input: Vec<u8>,
    /// The block of samples heard on the line, and then the two sides of what comes out.
    heard: Vec<f32>,
    left: Vec<f32>,
    right: Vec<f32>,
}

/// Makes a line. It lives for as long as the worklet does and is never freed.
#[unsafe(no_mangle)]
pub extern "C" fn gtr_line_new(sample_rate: u32) -> *mut Line {
    Box::into_raw(Box::new(Line {
        rack: Rack::new(sample_rate),
        input: Vec::new(),
        heard: Vec::new(),
        left: Vec::new(),
        right: Vec::new(),
    }))
}

/// Makes room for `length` bytes of chain and says where to write them.
///
/// # Safety
/// `line` must come from [`gtr_line_new`], as for every `gtr_line_` function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_line_input(line: *mut Line, length: usize) -> *mut u8 {
    let line = unsafe { &mut *line };
    line.input.clear();
    line.input.resize(length, 0);
    line.input.as_mut_ptr()
}

/// Sets the pedals of the track, or with `master` set those of the mix, to the chain
/// written to [`gtr_line_input`]. Returns 0 if the bytes are not a chain.
///
/// # Safety
/// See [`gtr_line_input`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_line_set(line: *mut Line, master: u32) -> u32 {
    let line = unsafe { &mut *line };
    let Some(chain) = chain_from_bytes(&line.input) else {
        return 0;
    };
    line.rack.set(master != 0, &chain);
    1
}

/// Makes room for `frames` samples heard on the line and says where to write them.
///
/// # Safety
/// See [`gtr_line_input`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_line_heard(line: *mut Line, frames: usize) -> *mut f32 {
    let line = unsafe { &mut *line };
    line.heard.resize(frames, 0.0);
    line.heard.as_mut_ptr()
}

/// Runs what was written to [`gtr_line_heard`], brought up or down by `gain`, through the
/// pedals, to be read from [`gtr_line_left`] and [`gtr_line_right`].
///
/// # Safety
/// See [`gtr_line_input`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_line_process(line: *mut Line, gain: f32) {
    let line = unsafe { &mut *line };
    line.left.clear();
    line.left.extend(line.heard.iter().map(|s| s * gain));
    line.right.clone_from(&line.left);
    line.rack.process(&mut line.left, &mut line.right);
    // What comes out is rounded off as the score's playback is, rather than clipped.
    for sample in line.left.iter_mut().chain(&mut line.right) {
        *sample = sample.tanh();
    }
}

/// # Safety
/// See [`gtr_line_input`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_line_left(line: *const Line) -> *const f32 {
    unsafe { &*line }.left.as_ptr()
}

/// # Safety
/// See [`gtr_line_input`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtr_line_right(line: *const Line) -> *const f32 {
    unsafe { &*line }.right.as_ptr()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtr_audio::NoteEvent;

    #[test]
    fn a_timeline_goes_in_as_bytes_and_comes_out_as_samples() {
        let bytes = Timeline {
            notes: vec![NoteEvent::plain(0.0, 0.2, 45)],
            duration: 0.2,
            ..Timeline::default()
        }
        .to_bytes();
        unsafe {
            let synth = gtr_new(44_100);
            // Nothing loaded: silence, and over at once.
            assert_eq!(gtr_process(synth, 128), 0);
            assert_eq!(gtr_load(synth, 0), 0);
            // A timeline is not a SoundFont, and neither is nothing at all.
            assert_eq!(gtr_soundfont(synth), 1);
            let input = gtr_input(synth, bytes.len());
            std::slice::from_raw_parts_mut(input, bytes.len()).copy_from_slice(&bytes);
            assert_eq!(gtr_soundfont(synth), 1);
            gtr_no_soundfont(synth);

            let input = gtr_input(synth, bytes.len());
            std::slice::from_raw_parts_mut(input, bytes.len()).copy_from_slice(&bytes);
            assert_eq!(gtr_load(synth, 0), 1);
            assert_eq!(gtr_process(synth, 128), 1);
            let left = std::slice::from_raw_parts(gtr_left(synth), 128);
            let right = std::slice::from_raw_parts(gtr_right(synth), 128);
            assert!(left.iter().any(|&s| s != 0.0) && left == right);
            assert_eq!(gtr_seconds(synth), 128.0 / 44_100.0);

            gtr_set_mix(synth, 0, 0, 1.0, 0.0);
            let mut blocks = 1;
            while gtr_process(synth, 128) == 1 {
                blocks += 1;
            }
            // A fifth of a second and the time allowed for the last notes to die away.
            assert_eq!(blocks, (0.5f64 * 44_100.0 / 128.0).ceil() as usize - 1);
            assert!((std::slice::from_raw_parts(gtr_left(synth), 128).iter()).all(|&s| s == 0.0));
            drop(Box::from_raw(synth));
        }
    }

    #[test]
    fn a_line_goes_through_its_pedals() {
        use gtr_audio::chain_to_bytes;
        use gtr_model::{Pedal, PedalKind};
        unsafe {
            let line = gtr_line_new(44_100);
            let write = |bytes: &[u8]| {
                let input = gtr_line_input(line, bytes.len());
                std::slice::from_raw_parts_mut(input, bytes.len()).copy_from_slice(bytes);
            };
            let play = |gain| {
                let heard = std::slice::from_raw_parts_mut(gtr_line_heard(line, 128), 128);
                for (i, sample) in heard.iter_mut().enumerate() {
                    *sample = if i % 64 < 32 { 0.1 } else { -0.1 };
                }
                gtr_line_process(line, gain);
                let left = std::slice::from_raw_parts(gtr_line_left(line), 128);
                let right = std::slice::from_raw_parts(gtr_line_right(line), 128);
                (left.to_vec(), right.to_vec())
            };
            // With no pedals, what is heard comes out on both sides, brought up by the gain.
            let (left, right) = play(2.0);
            assert_eq!(left, right);
            assert!((left[0] - 0.2f32.tanh()).abs() < 1e-6);
            // A chorus on the track and nothing on the mix: the sides part.
            write(&chain_to_bytes(&[Pedal::with(
                PedalKind::Chorus,
                &[50, 100, 100],
            )]));
            assert_eq!(gtr_line_set(line, 0), 1);
            write(&chain_to_bytes(&[]));
            assert_eq!(gtr_line_set(line, 1), 1);
            for _ in 0..20 {
                play(1.0);
            }
            let (left, right) = play(1.0);
            assert!(left != right);
            // Bytes that are not a chain are refused.
            write(b"no");
            assert_eq!(gtr_line_set(line, 0), 0);
            drop(Box::from_raw(line));
        }
    }

    #[test]
    fn a_looped_passage_goes_on_to_the_one_queued() {
        let looped = |pitch| {
            Timeline {
                notes: vec![NoteEvent::plain(0.0, 0.1, pitch)],
                duration: 0.1,
                looped: true,
                ..Timeline::default()
            }
            .to_bytes()
        };
        let write = |synth, bytes: &[u8]| unsafe {
            let input = gtr_input(synth, bytes.len());
            std::slice::from_raw_parts_mut(input, bytes.len()).copy_from_slice(bytes);
        };
        unsafe {
            let synth = gtr_new(44_100);
            write(synth, &looped(45));
            assert_eq!(gtr_load(synth, 0), 1);
            write(synth, b"not a timeline");
            assert_eq!(gtr_queue(synth, 0), 0);
            assert_eq!(gtr_queued(synth), 0);
            write(synth, &looped(52));
            assert_eq!(gtr_queue(synth, 0), 1);
            assert_eq!(gtr_queued(synth), 1);
            // A tenth of a second is 4410 samples, which 35 blocks pass.
            for _ in 0..35 {
                assert_eq!(gtr_process(synth, 128), 1);
            }
            assert_eq!((gtr_passes(synth), gtr_queued(synth)), (1, 0));
            write(synth, &looped(57));
            gtr_queue(synth, 0);
            assert_eq!(gtr_queue(synth, 1), 1);
            assert_eq!(gtr_queued(synth), 0);
            drop(Box::from_raw(synth));
        }
    }
}
