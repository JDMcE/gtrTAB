//! SVG: one element for every item of the display list, drawn as the canvas draws it.

use std::fmt::Write;

use gtr_layout::{Align, Color, Font, Item, Layout};

use crate::fonts::{Fonts, Segment, patch};

/// How to write a score as SVG.
#[derive(Debug, Clone, Copy, Default)]
pub struct SvgOptions<'a> {
    /// Where the music font, Bravura, is to be loaded from. Without it the file names the
    /// font and leaves finding it to whatever shows the file.
    pub music_font_url: Option<&'a str>,
    /// Fonts to draw the text with. Given these, every glyph is written as its outline
    /// and the file needs no font to be shown; without them text is written as text,
    /// in whatever fonts the thing that shows it has.
    pub fonts: Option<&'a Fonts<'a>>,
}

const INK: &str = "#1b1b1f";
const MUTED: &str = "#8e8e96";
const ERROR: &str = "#c62828";
const PAPER: &str = "#ffffff";
/// Width of a character as a share of the font size, for the patch a fret number clears
/// behind itself. The canvas measures the text; a file cannot.
const CHARACTER_WIDTH: f64 = 0.6;

fn color(color: Color) -> &'static str {
    match color {
        Color::Ink => INK,
        Color::Muted => MUTED,
        Color::Error => ERROR,
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Writes a laid-out score as an SVG document.
pub fn svg(layout: &Layout, options: SvgOptions) -> String {
    let mut out = String::new();
    let (w, h) = (layout.width, layout.height);
    let _ = writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#
    );
    if let Some(url) = options.music_font_url {
        let url = escape(url);
        let _ = writeln!(
            out,
            r#"<style>@font-face {{ font-family: Bravura; src: url("{url}"); }}</style>"#
        );
    }
    let _ = writeln!(out, r#"<rect width="{w}" height="{h}" fill="{PAPER}"/>"#);
    for item in &layout.items {
        let _ = match item {
            Item::Line {
                x1,
                y1,
                x2,
                y2,
                w,
                c,
            } => writeln!(
                out,
                r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{}" stroke-width="{w}"/>"#,
                color(*c)
            ),
            Item::Rect { x, y, w, h, c } => writeln!(
                out,
                r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{}"/>"#,
                color(*c)
            ),
            Item::Dot { x, y, r } => {
                writeln!(out, r#"<circle cx="{x}" cy="{y}" r="{r}" fill="{INK}"/>"#)
            }
            Item::Beam { x1, y1, x2, y2, h } => writeln!(
                out,
                r#"<polygon points="{x1},{y1} {x2},{y2} {x2},{} {x1},{}" fill="{INK}"/>"#,
                y2 + h,
                y1 + h
            ),
            Item::Arc { x1, x2, y, h } => writeln!(
                out,
                r#"<path d="M{x1} {y} Q{} {} {x2} {y}" fill="none" stroke="{INK}" stroke-width="1.1"/>"#,
                (x1 + x2) / 2.0,
                y - h * 2.0
            ),
            Item::Curve {
                x1,
                y1,
                cx,
                cy,
                x2,
                y2,
            } => writeln!(
                out,
                r#"<path d="M{x1} {y1} Q{cx} {cy} {x2} {y2}" fill="none" stroke="{INK}" stroke-width="1.1"/>"#
            ),
            Item::Text {
                x,
                y,
                s,
                size,
                font,
                align,
                knockout,
                c,
            } => {
                if let Some(fonts) = options.fonts {
                    let line = fonts.set(s, *x, *y, *size, *font, *align);
                    if *knockout {
                        let (left, top, width, height) = patch(&line, *y, *size);
                        let _ = writeln!(
                            out,
                            r#"<rect x="{left:.1}" y="{top:.1}" width="{width:.1}" height="{height:.1}" fill="{PAPER}"/>"#,
                        );
                    }
                    let _ = writeln!(
                        out,
                        r#"<path d="{}" fill="{}"/>"#,
                        outlines(fonts, &line),
                        color(*c)
                    );
                    continue;
                }
                if *knockout {
                    let width = s.chars().count() as f64 * size * CHARACTER_WIDTH;
                    let left = match align {
                        Align::Left => *x,
                        Align::Center => x - width / 2.0,
                        Align::Right => x - width,
                    };
                    let _ = writeln!(
                        out,
                        r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{PAPER}"/>"#,
                        left - 1.5,
                        y - size * 0.5,
                        width + 3.0,
                        size
                    );
                }
                let anchor = match align {
                    Align::Left => "start",
                    Align::Center => "middle",
                    Align::Right => "end",
                };
                // Music symbols are placed by their origin, other text by its middle.
                let (family, weight, baseline) = match font {
                    Font::Music => ("Bravura", 400, "alphabetic"),
                    Font::Fret => ("system-ui, sans-serif", 600, "central"),
                    Font::Text => ("system-ui, sans-serif", 400, "central"),
                };
                writeln!(
                    out,
                    r#"<text x="{x}" y="{y}" font-family="{family}" font-size="{size}" font-weight="{weight}" text-anchor="{anchor}" dominant-baseline="{baseline}" fill="{}">{}</text>"#,
                    color(*c),
                    escape(s)
                )
            }
        };
    }
    out.push_str("</svg>\n");
    out
}

/// The outlines of a line of text as the data of one path, in the layout's pixels.
fn outlines(fonts: &Fonts, line: &crate::fonts::Line) -> String {
    let mut d = String::new();
    for placed in &line.glyphs {
        // The font's units go up the page; the layout's pixels go down it.
        let x = |x: f32| placed.x + f64::from(x) * placed.scale;
        let y = |y: f32| placed.y - f64::from(y) * placed.scale;
        for segment in fonts.outline(placed.face, placed.glyph) {
            let _ = match segment {
                Segment::Move(px, py) => write!(d, "M{:.2} {:.2}", x(px), y(py)),
                Segment::Line(px, py) => write!(d, "L{:.2} {:.2}", x(px), y(py)),
                Segment::Quad(ax, ay, px, py) => {
                    write!(d, "Q{:.2} {:.2} {:.2} {:.2}", x(ax), y(ay), x(px), y(py))
                }
                Segment::Cubic(ax, ay, bx, by, px, py) => write!(
                    d,
                    "C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2}",
                    x(ax),
                    y(ay),
                    x(bx),
                    y(by),
                    x(px),
                    y(py)
                ),
                Segment::Close => write!(d, "Z"),
            };
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_item_of_the_display_list_is_written() {
        let score = gtr_model::demo_score();
        let layout = gtr_layout::layout(&score, &gtr_layout::View::default(), 1000.0);
        let written = svg(&layout, SvgOptions::default());
        assert!(written.starts_with("<svg ") && written.ends_with("</svg>\n"));
        // The page, one element for each item, and a patch behind each fret number.
        let patches = (layout.items.iter())
            .filter(|i| matches!(i, Item::Text { knockout: true, .. }))
            .count();
        let elements = written.lines().count() - 2;
        assert_eq!(elements, 1 + layout.items.len() + patches);
        assert!(!written.contains("@font-face"));
        let named = svg(
            &layout,
            SvgOptions {
                music_font_url: Some("fonts/Bravura.woff2"),
                ..SvgOptions::default()
            },
        );
        assert!(named.contains(r#"url("fonts/Bravura.woff2")"#));
        // Text that would be read as markup is escaped.
        assert_eq!(escape("<5> & up"), "&lt;5&gt; &amp; up");
    }
}
