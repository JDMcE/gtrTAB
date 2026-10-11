//! The route playback takes through a score, and how long it takes to get to each bar.

use crate::{Jump, MasterBar, Score, Target};

/// The order in which bars are played once repeats, alternate endings and jumps such as
/// D.S. al Coda are followed. A jump back is taken once, and the repeats it lands among are
/// played again.
pub fn play_order(masters: &[MasterBar]) -> Vec<usize> {
    let find = |target: Target| masters.iter().position(|m| m.target == Some(target));
    let mut order = Vec::with_capacity(masters.len());
    let mut bar = 0;
    // Where the current repeat goes back to, and which time through it this is.
    let mut repeat_start = 0;
    let mut pass: u8 = 1;
    // A repeat has just been completed: its last ending may still follow.
    let mut finished = false;
    let mut jumped = false;
    let (mut al_fine, mut al_coda) = (false, false);
    // Scores that loop for ever are cut short rather than followed for ever.
    while bar < masters.len() && order.len() < 20_000 {
        let master = &masters[bar];
        if master.repeat_start && repeat_start != bar {
            repeat_start = bar;
            pass = 1;
            finished = false;
        }
        if master.alternate_endings == 0 && finished {
            pass = 1;
            finished = false;
        }
        let ending = master.alternate_endings;
        if ending != 0 && ending & (1 << (pass - 1).min(7)) == 0 {
            bar += 1;
            continue;
        }
        order.push(bar);
        if al_fine && master.target == Some(Target::Fine) {
            break;
        }
        match master.jump {
            Some(Jump::ToCoda) if al_coda => {
                if let Some(coda) = find(Target::Coda) {
                    al_coda = false;
                    bar = coda;
                    continue;
                }
            }
            Some(jump) if !jumped && jump != Jump::ToCoda => {
                jumped = true;
                al_fine = matches!(jump, Jump::DaCapoAlFine | Jump::DalSegnoAlFine);
                al_coda = matches!(jump, Jump::DaCapoAlCoda | Jump::DalSegnoAlCoda);
                let to_segno = matches!(
                    jump,
                    Jump::DalSegno | Jump::DalSegnoAlFine | Jump::DalSegnoAlCoda
                );
                bar = to_segno.then(|| find(Target::Segno)).flatten().unwrap_or(0);
                repeat_start = bar;
                pass = 1;
                finished = false;
                continue;
            }
            _ => {}
        }
        if master.repeat_count >= 2 {
            if pass < master.repeat_count {
                pass += 1;
                bar = repeat_start;
                continue;
            }
            finished = true;
            repeat_start = bar + 1;
        }
        bar += 1;
    }
    order
}

/// Seconds into the piece at which each bar is first played, following repeats and jumps
/// at the tempo in force, or `None` for a bar that is never reached.
pub fn bar_times(score: &Score) -> Vec<Option<f64>> {
    let masters = &score.master_bars;
    // The tempo in force at each bar: the last change at or before it.
    let mut tempo = score.tempo;
    let tempos: Vec<u32> = (masters.iter())
        .map(|master| {
            tempo = master.tempo.unwrap_or(tempo);
            tempo
        })
        .collect();
    let mut times = vec![None; masters.len()];
    let mut time = 0.0;
    for bar in play_order(masters) {
        times[bar].get_or_insert(time);
        let length = masters[bar].time_signature.bar_length();
        let whole_notes = *length.numer() as f64 / *length.denom() as f64;
        time += whole_notes * 240.0 / f64::from(tempos[bar].max(1));
    }
    times
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo_score;

    #[test]
    fn bar_times_follow_repeats_and_tempo_changes() {
        let mut score = demo_score();
        assert_eq!(bar_times(&score), [Some(0.0), Some(2.0)]);
        // The first bar twice, at 120; then the second.
        score.master_bars[0].repeat_count = 2;
        assert_eq!(bar_times(&score), [Some(0.0), Some(4.0)]);
        // A faster first bar gets to the second sooner.
        score.master_bars[0].tempo = Some(240);
        score.master_bars[1].tempo = Some(120);
        assert_eq!(bar_times(&score), [Some(0.0), Some(2.0)]);
        // A bar that only an unreachable ending holds is never played.
        score.master_bars[1].alternate_endings = 0b100;
        assert_eq!(bar_times(&score)[1], None);
    }
}
