//! Plays through a real SoundFont, which is too big to keep in the repository. Set
//! `GTR_SOUNDFONT` to the path of a General MIDI bank such as GeneralUser GS (fetched by
//! `npm run soundfont` in `app/ui`) to run it; without that it passes without doing anything.

use gtr_audio::{Channel, Engine, NoteEvent, SoundFont, Timbre, Timeline};

const RATE: u32 = 44_100;

/// One note on a track of the given timbre, rendered with the bank or with the models.
fn note(bytes: Option<&[u8]>, timbre: Timbre, pitch: u8) -> Vec<f32> {
    let timeline = Timeline {
        channels: vec![Channel {
            timbre,
            ..Channel::default()
        }],
        notes: vec![NoteEvent::plain(0.0, 1.0, pitch)],
        duration: 1.0,
        ..Timeline::default()
    };
    let mut engine = Engine::new(RATE);
    engine.set_soundfont(bytes.map(|bytes| SoundFont::parse(bytes).unwrap()));
    engine.load(timeline);
    let (mut left, mut right) = (vec![0.0; 2 * RATE as usize], vec![0.0; 2 * RATE as usize]);
    engine.process(&mut left, &mut right);
    left
}

/// The same note's right channel: a kit has its pieces to either side.
fn note_right(bytes: Option<&[u8]>, timbre: Timbre, pitch: u8) -> Vec<f32> {
    let timeline = Timeline {
        channels: vec![Channel {
            timbre,
            ..Channel::default()
        }],
        notes: vec![NoteEvent::plain(0.0, 1.0, pitch)],
        duration: 1.0,
        ..Timeline::default()
    };
    let mut engine = Engine::new(RATE);
    engine.set_soundfont(bytes.map(|bytes| SoundFont::parse(bytes).unwrap()));
    engine.load(timeline);
    let (mut left, mut right) = (vec![0.0; 2 * RATE as usize], vec![0.0; 2 * RATE as usize]);
    engine.process(&mut left, &mut right);
    right
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// The fundamental of `samples` in Hz, from the lag at which they best match themselves.
fn pitch(samples: &[f32], expected: f32) -> f32 {
    let guess = RATE as f32 / expected;
    let span = samples.len() / 2;
    let match_at = |lag: usize| -> f32 {
        (samples[..span].iter().zip(&samples[lag..]))
            .map(|(a, b)| a * b)
            .sum()
    };
    let lags = (guess * 0.94) as usize..=(guess * 1.06) as usize + 1;
    let best = lags
        .max_by(|&a, &b| match_at(a).total_cmp(&match_at(b)))
        .unwrap();
    RATE as f32 / best as f32
}

#[test]
fn a_general_midi_bank_plays_every_instrument_in_tune_and_at_a_like_level() {
    let Ok(path) = std::env::var("GTR_SOUNDFONT") else {
        return;
    };
    let bytes = std::fs::read(&path).unwrap();
    let font = SoundFont::parse(&bytes).unwrap();
    println!("{}: {} presets", font.name(), font.preset_count());

    for (timbre, key) in [
        (Timbre::Steel, 57),
        (Timbre::Nylon, 57),
        (Timbre::Clean, 57),
        (Timbre::Overdrive, 57),
        (Timbre::Distortion, 57),
        (Timbre::Bass, 33),
        (Timbre::Piano, 57),
        (Timbre::Banjo, 57),
    ] {
        let (bank, program) = timbre.preset().unwrap();
        assert!(font.has(bank, program), "{timbre:?} is missing");
        let sampled = note(Some(&bytes), timbre, key);
        let modelled = note(None, timbre, key);
        assert!(sampled != modelled, "{timbre:?} did not use the bank");
        assert!(sampled.iter().all(|s| s.abs() <= 1.0));
        let (ours, theirs) = (
            rms(&modelled[..RATE as usize]),
            rms(&sampled[..RATE as usize]),
        );
        let expected = 440.0 * 2f32.powf((f32::from(key) - 69.0) / 12.0);
        let heard = pitch(&sampled[4_000..16_000], expected);
        let cents = 1200.0 * (heard / expected).log2();
        println!("{timbre:?}: model {ours:.4}, bank {theirs:.4}, {cents:+.0} cents");
        assert!(cents.abs() < 25.0, "{timbre:?} is {cents} cents out");
        assert!(
            theirs > ours * 0.3 && theirs < ours * 3.0,
            "{timbre:?} is at the wrong level"
        );
        // Let go after a second, the note is over within another.
        assert!(
            rms(&sampled[sampled.len() - 4_000..]) < 1e-3,
            "{timbre:?} rings on"
        );
    }

    // A bank's brightness is in how it moves its filters. GeneralUser GS leaves the steel
    // guitar's at 500 Hz and has the key and the velocity open it: read without that, the
    // top of the neck is 25 dB down on the bottom of it, and it should be within 12.
    let (bottom, top) = (
        note(Some(&bytes), Timbre::Steel, 40),
        note(Some(&bytes), Timbre::Steel, 88),
    );
    let peak = |samples: &[f32]| samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    println!(
        "steel guitar: low E {:.3}, top E {:.3}",
        peak(&bottom),
        peak(&top)
    );
    assert!(
        peak(&top) > peak(&bottom) * 0.25,
        "the top of the neck is dull"
    );

    // Every piece of the kit that gtrTab names can be heard. The tambourine is all top,
    // and is lost if the envelope that opens its filter is not followed.
    for piece in 35..=81 {
        let heard = note(Some(&bytes), Timbre::Drums, piece);
        let right = note_right(Some(&bytes), Timbre::Drums, piece);
        let loudest = peak(&heard).max(peak(&right));
        assert!(loudest > 0.04, "drum {piece} peaks at {loudest}");
        assert!(loudest < 0.98, "drum {piece} clips");
    }

    // The kit: a bass drum is low and long over, a crash is still ringing.
    let kick = note(Some(&bytes), Timbre::Drums, 36);
    let crash = note(Some(&bytes), Timbre::Drums, 49);
    println!(
        "kick {:.4} (model {:.4}), crash {:.4} (model {:.4})",
        rms(&kick[..8_000]),
        rms(&note(None, Timbre::Drums, 36)[..8_000]),
        rms(&crash[..8_000]),
        rms(&note(None, Timbre::Drums, 49)[..8_000]),
    );
    assert!(rms(&kick[..8_000]) > 0.02 && rms(&crash[..8_000]) > 0.01);
    assert!(rms(&kick[60_000..]) < 1e-3);
    assert!(rms(&crash[30_000..40_000]) > 1e-4);
}
