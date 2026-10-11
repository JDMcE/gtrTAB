//! The drum kit: which General MIDI percussion number is which piece, and how each piece
//! is written on a percussion staff.

/// The shape of a note on a percussion staff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// One piece of the kit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrumPiece {
    /// General MIDI percussion number, which is the fret of a note on a drum track.
    pub number: u8,
    pub name: &'static str,
    /// Steps above the middle line of the staff: lines are even, spaces odd.
    pub position: i32,
    pub head: DrumHead,
}

const fn piece(number: u8, name: &'static str, position: i32, head: DrumHead) -> DrumPiece {
    DrumPiece {
        number,
        name,
        position,
        head,
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

/// The piece of the kit with a General MIDI percussion number, if the kit has it.
pub fn drum_piece(number: u8) -> Option<&'static DrumPiece> {
    DRUM_KIT.iter().find(|p| p.number == number)
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
    }
}
