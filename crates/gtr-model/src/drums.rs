//! The drum kit: which General MIDI percussion number is which piece, and how each piece
//! is written on a percussion staff. A track can have a kit of its own in place of the
//! standard one.

use serde::{Deserialize, Serialize};

/// The shape of a note on a percussion staff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum DrumHead {
    /// An ordinary notehead: drums.
    Normal,
    /// A cross: cymbals and the closed hi-hat.
    Cross,
    /// A cross in a circle: the open hi-hat.
    CircledCross,
    /// A diamond: the bell of the ride.
    Diamond,
    /// A triangle: cowbell and other small percussion.
    Triangle,
}

/// One piece of the standard kit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrumPiece {
    /// General MIDI percussion number, which is the fret of a note on a drum track.
    pub number: u8,
    pub name: &'static str,
    /// Steps above the middle line of the staff: lines are even, spaces odd.
    pub position: i32,
    pub head: DrumHead,
    /// Played with a foot, and so written with its stem down, apart from the hands.
    pub foot: bool,
}

/// One piece of a track's own kit.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct KitPiece {
    /// General MIDI percussion number, which is the fret of a note on a drum track and
    /// decides what the piece sounds like.
    pub number: u8,
    pub name: String,
    /// Steps above the middle line of the staff: lines are even, spaces odd.
    pub position: i8,
    pub head: DrumHead,
    /// Played with a foot, and so written with its stem down, apart from the hands.
    #[serde(default)]
    pub foot: bool,
}

impl KitPiece {
    /// The most pieces a kit can have.
    pub const MAX_PIECES: usize = 64;
    /// Longest name of a piece, in characters.
    pub const MAX_NAME: usize = 40;
    /// The furthest from the middle line a piece can be written: three ledger lines out.
    pub const MAX_POSITION: i8 = 10;

    pub fn is_valid(&self) -> bool {
        self.number <= 127
            && self.name.chars().count() <= KitPiece::MAX_NAME
            && self.position.abs() <= KitPiece::MAX_POSITION
    }
}

/// Whether a kit can be a track's: no more pieces than a kit holds, each valid, and no
/// number twice.
pub fn kit_is_valid(kit: &[KitPiece]) -> bool {
    kit.len() <= KitPiece::MAX_PIECES
        && kit.iter().all(KitPiece::is_valid)
        && (kit.iter().enumerate()).all(|(i, p)| kit[..i].iter().all(|q| q.number != p.number))
}

const fn piece(number: u8, name: &'static str, position: i32, head: DrumHead) -> DrumPiece {
    DrumPiece {
        number,
        name,
        position,
        head,
        // The bass drums and the hi-hat's pedal.
        foot: matches!(number, 35 | 36 | 44),
    }
}

/// The kit, lowest number first, placed as drum notation usually has it: bass drum in the
/// bottom space, snare in the third space, cymbals above the staff.
pub const DRUM_KIT: [DrumPiece; 23] = [
    piece(35, "Bass drum 2", -3, DrumHead::Normal),
    piece(36, "Bass drum", -3, DrumHead::Normal),
    piece(37, "Side stick", 1, DrumHead::Cross),
    piece(38, "Snare", 1, DrumHead::Normal),
    piece(39, "Hand clap", 1, DrumHead::Triangle),
    piece(40, "Electric snare", 1, DrumHead::Normal),
    piece(41, "Low floor tom", -2, DrumHead::Normal),
    piece(42, "Closed hi-hat", 5, DrumHead::Cross),
    piece(43, "High floor tom", -1, DrumHead::Normal),
    piece(44, "Pedal hi-hat", -5, DrumHead::Cross),
    piece(45, "Low tom", 0, DrumHead::Normal),
    piece(46, "Open hi-hat", 5, DrumHead::CircledCross),
    piece(47, "Low-mid tom", 2, DrumHead::Normal),
    piece(48, "High-mid tom", 3, DrumHead::Normal),
    piece(49, "Crash cymbal", 6, DrumHead::Cross),
    piece(50, "High tom", 4, DrumHead::Normal),
    piece(51, "Ride cymbal", 4, DrumHead::Cross),
    piece(52, "China cymbal", 7, DrumHead::Cross),
    piece(53, "Ride bell", 4, DrumHead::Diamond),
    piece(54, "Tambourine", 3, DrumHead::Triangle),
    piece(55, "Splash cymbal", 7, DrumHead::Cross),
    piece(56, "Cowbell", 3, DrumHead::Triangle),
    piece(57, "Crash cymbal 2", 6, DrumHead::Cross),
];

/// The piece of the standard kit with a General MIDI percussion number, if it has it.
pub fn drum_piece(number: u8) -> Option<&'static DrumPiece> {
    DRUM_KIT.iter().find(|p| p.number == number)
}

/// The standard kit as a kit a track can have and change.
pub fn standard_kit() -> Vec<KitPiece> {
    (DRUM_KIT.iter())
        .map(|p| KitPiece {
            number: p.number,
            name: p.name.to_owned(),
            position: p.position as i8,
            head: p.head,
            foot: p.foot,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kit_is_in_order_and_found_by_number() {
        assert!(DRUM_KIT.windows(2).all(|w| w[0].number < w[1].number));
        assert_eq!(drum_piece(38).map(|p| p.name), Some("Snare"));
        assert_eq!(drum_piece(42).map(|p| p.head), Some(DrumHead::Cross));
        assert!(drum_piece(20).is_none());
        // The feet play the bass drums and the hi-hat's pedal.
        let feet: Vec<u8> = (DRUM_KIT.iter().filter(|p| p.foot))
            .map(|p| p.number)
            .collect();
        assert_eq!(feet, [35, 36, 44]);
    }

    #[test]
    fn a_kit_of_a_tracks_own_is_checked() {
        let mut kit = standard_kit();
        assert!(kit_is_valid(&kit) && kit_is_valid(&[]));
        assert_eq!(kit[1].name, "Bass drum");
        kit[0].number = 36;
        assert!(!kit_is_valid(&kit), "a number twice");
        let mut kit = standard_kit();
        kit[0].position = 11;
        assert!(!kit_is_valid(&kit), "too far from the staff");
        kit[0].position = -10;
        kit[0].name = "x".repeat(41);
        assert!(!kit_is_valid(&kit), "too long a name");
    }
}
