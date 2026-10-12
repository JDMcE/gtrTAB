//! The fonts a score is drawn with where there is no browser to draw its text: the music
//! font, a text font and a heavier one for fret numbers. Their glyphs are drawn as
//! outlines, so that a file made with them needs no font to be shown.

use gtr_layout::{Align, Font};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

#[derive(Debug, thiserror::Error)]
pub enum FontError {
    #[error("the {0} font could not be read")]
    Unreadable(&'static str),
}

/// The three fonts, as the bytes of their OpenType or TrueType files.
pub struct Fonts<'a> {
    faces: [Face<'a>; 3],
}

impl std::fmt::Debug for Fonts<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Fonts")
    }
}

/// A piece of a glyph's outline, in the font's units with y upwards.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Segment {
    Move(f32, f32),
    Line(f32, f32),
    Quad(f32, f32, f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

struct Outline(Vec<Segment>);

impl OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(Segment::Move(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(Segment::Line(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.push(Segment::Quad(x1, y1, x, y));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.push(Segment::Cubic(x1, y1, x2, y2, x, y));
    }

    fn close(&mut self) {
        self.0.push(Segment::Close);
    }
}

/// A glyph placed on the page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placed {
    /// Which of the fonts it is from, and which of that font's glyphs.
    pub face: usize,
    pub glyph: u16,
    /// Where its origin is, in the layout's pixels.
    pub x: f64,
    pub y: f64,
    /// Pixels to one of the font's units.
    pub scale: f64,
}

/// A line of text set in a font: its glyphs, and where it starts and how wide it is.
pub(crate) struct Line {
    pub glyphs: Vec<Placed>,
    pub left: f64,
    pub width: f64,
}

impl<'a> Fonts<'a> {
    /// Reads the fonts: Bravura or another SMuFL font for the music, and two faces of a
    /// text font, the second for fret numbers.
    pub fn parse(music: &'a [u8], text: &'a [u8], fret: &'a [u8]) -> Result<Fonts<'a>, FontError> {
        let face = |bytes, name| Face::parse(bytes, 0).map_err(|_| FontError::Unreadable(name));
        Ok(Fonts {
            faces: [
                face(music, "music")?,
                face(text, "text")?,
                face(fret, "fret number")?,
            ],
        })
    }

    /// Sets a line of text at `(x, y)` as the display list means it: music symbols by
    /// their origin, other text by the middle of its height.
    pub(crate) fn set(&self, s: &str, x: f64, y: f64, size: f64, font: Font, align: Align) -> Line {
        let first = match font {
            Font::Music => 0,
            Font::Text => 1,
            Font::Fret => 2,
        };
        let face = &self.faces[first];
        let scale = size / f64::from(face.units_per_em());
        let baseline = match font {
            Font::Music => y,
            // The middle of the line is half way between how far the font rises above
            // the line it is written on and how far it falls below.
            _ => y + f64::from(face.ascender() + face.descender()) / 2.0 * scale,
        };
        let mut glyphs = Vec::new();
        let mut pen = 0.0;
        for c in s.chars() {
            // A character the font has no glyph for is looked for in the others.
            let found = [first, 1, 2, 0].into_iter().find_map(|index| {
                let glyph = self.faces[index].glyph_index(c)?;
                Some((index, glyph))
            });
            let Some((index, glyph)) = found else {
                pen += size * 0.5;
                continue;
            };
            let face = &self.faces[index];
            let scale = size / f64::from(face.units_per_em());
            glyphs.push(Placed {
                face: index,
                glyph: glyph.0,
                x: pen,
                y: baseline,
                scale,
            });
            pen += f64::from(face.glyph_hor_advance(glyph).unwrap_or(0)) * scale;
        }
        let left = match align {
            Align::Left => x,
            Align::Center => x - pen / 2.0,
            Align::Right => x - pen,
        };
        glyphs.iter_mut().for_each(|glyph| glyph.x += left);
        Line {
            glyphs,
            left,
            width: pen,
        }
    }

    /// A glyph's outline; empty for one that is only space.
    pub(crate) fn outline(&self, face: usize, glyph: u16) -> Vec<Segment> {
        let mut outline = Outline(Vec::new());
        self.faces[face].outline_glyph(GlyphId(glyph), &mut outline);
        outline.0
    }
}

/// The patch a fret number clears behind itself on its string: where it starts, how wide
/// it is and how high.
pub(crate) fn patch(line: &Line, y: f64, size: f64) -> (f64, f64, f64, f64) {
    (line.left - 1.5, y - size * 0.5, line.width + 3.0, size)
}
