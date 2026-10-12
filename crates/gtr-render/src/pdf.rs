//! PDF: a page of the file for each page of the score, drawn in lines and outlines. A
//! glyph is put in the file once and placed wherever it is used.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use gtr_layout::{Color, Item, Layout};
use miniz_oxide::deflate::compress_to_vec_zlib;

use crate::fonts::{Fonts, Segment, patch};

/// Points to a pixel: a layout is measured at 96 pixels to the inch, a PDF at 72 points.
const POINTS: f64 = 0.75;

fn rgb(color: Color) -> &'static str {
    match color {
        Color::Ink => "0.106 0.106 0.122",
        Color::Muted | Color::Error => "0.557 0.557 0.588",
    }
}

/// A number as short as it can be written to a hundredth.
fn n(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let mut text = format!("{rounded:.2}");
    while text.contains('.') && (text.ends_with('0') || text.ends_with('.')) {
        text.pop();
    }
    match text.as_str() {
        "-0" | "" => "0".to_owned(),
        _ => text,
    }
}

/// A curve with one control point as PDF draws curves, with two.
fn quad(from: (f64, f64), control: (f64, f64), to: (f64, f64)) -> String {
    let third = |a: f64, b: f64| a + (b - a) * 2.0 / 3.0;
    format!(
        "{} {} {} {} {} {} c",
        n(third(from.0, control.0)),
        n(third(from.1, control.1)),
        n(third(to.0, control.0)),
        n(third(to.1, control.1)),
        n(to.0),
        n(to.1)
    )
}

/// The file's objects, each numbered from one in the order it is added.
#[derive(Default)]
struct Objects(Vec<Vec<u8>>);

impl Objects {
    fn add(&mut self, body: Vec<u8>) -> usize {
        self.0.push(body);
        self.0.len()
    }

    /// Keeps a place for an object that is written later, and gives its number.
    fn reserve(&mut self) -> usize {
        self.add(Vec::new())
    }

    fn stream(&mut self, dictionary: &str, data: &[u8]) -> usize {
        let packed = compress_to_vec_zlib(data, 6);
        let mut body = format!(
            "<< {dictionary} /Length {} /Filter /FlateDecode >>\nstream\n",
            packed.len()
        )
        .into_bytes();
        body.extend(packed);
        body.extend(b"\nendstream");
        self.add(body)
    }
}

/// Text as a PDF string that can hold any character: its UTF-16, in hexadecimal.
fn string(text: &str) -> String {
    let mut out = String::from("<FEFF");
    for unit in text.encode_utf16() {
        let _ = write!(out, "{unit:04X}");
    }
    out.push('>');
    out
}

/// Writes the pages of a score as a PDF. Each page is a laid-out page, as
/// [`Layout::pages`] makes them.
pub fn pdf(pages: &[Layout], fonts: &Fonts, title: &str) -> Vec<u8> {
    let mut objects = Objects::default();
    let catalog = objects.reserve();
    let tree = objects.reserve();
    let resources = objects.reserve();
    let info = objects.add(
        format!(
            "<< /Title {} /Creator (gtrTab) /Producer (gtrTab) >>",
            string(title)
        )
        .into_bytes(),
    );

    // The glyphs used, each drawn once in its font's units.
    let mut glyphs: BTreeMap<(usize, u16), String> = BTreeMap::new();
    let mut page_ids = Vec::new();
    for page in pages {
        let (width, height) = (page.width * POINTS, page.height * POINTS);
        // Pixels from the top left, as the layout counts them.
        let mut content = format!("{POINTS} 0 0 -{POINTS} 0 {} cm\n", n(height));
        for item in &page.items {
            let _ = match item {
                Item::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    w,
                    c,
                } => writeln!(
                    content,
                    "{} RG {} w {} {} m {} {} l S",
                    rgb(*c),
                    n(*w),
                    n(*x1),
                    n(*y1),
                    n(*x2),
                    n(*y2)
                ),
                Item::Rect { x, y, w, h, c } => writeln!(
                    content,
                    "{} rg {} {} {} {} re f",
                    rgb(*c),
                    n(*x),
                    n(*y),
                    n(*w),
                    n(*h)
                ),
                Item::Dot { x, y, r } => {
                    // A circle, in four curves.
                    let k = r * 0.5523;
                    writeln!(
                        content,
                        "{ink} rg {} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c \
                         {} {} {} {} {} {} c f",
                        n(x + r),
                        n(*y),
                        n(x + r),
                        n(y + k),
                        n(x + k),
                        n(y + r),
                        n(*x),
                        n(y + r),
                        n(x - k),
                        n(y + r),
                        n(x - r),
                        n(y + k),
                        n(x - r),
                        n(*y),
                        n(x - r),
                        n(y - k),
                        n(x - k),
                        n(y - r),
                        n(*x),
                        n(y - r),
                        n(x + k),
                        n(y - r),
                        n(x + r),
                        n(y - k),
                        n(x + r),
                        n(*y),
                        ink = rgb(Color::Ink)
                    )
                }
                Item::Beam { x1, y1, x2, y2, h } => writeln!(
                    content,
                    "{} rg {} {} m {} {} l {} {} l {} {} l h f",
                    rgb(Color::Ink),
                    n(*x1),
                    n(*y1),
                    n(*x2),
                    n(*y2),
                    n(*x2),
                    n(y2 + h),
                    n(*x1),
                    n(y1 + h)
                ),
                Item::Arc { x1, x2, y, h } => writeln!(
                    content,
                    "{} RG 1.1 w {} {} m {} S",
                    rgb(Color::Ink),
                    n(*x1),
                    n(*y),
                    quad((*x1, *y), ((x1 + x2) / 2.0, y - h * 2.0), (*x2, *y))
                ),
                Item::Curve {
                    x1,
                    y1,
                    cx,
                    cy,
                    x2,
                    y2,
                } => writeln!(
                    content,
                    "{} RG 1.1 w {} {} m {} S",
                    rgb(Color::Ink),
                    n(*x1),
                    n(*y1),
                    quad((*x1, *y1), (*cx, *cy), (*x2, *y2))
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
                    let line = fonts.set(s, *x, *y, *size, *font, *align);
                    if *knockout {
                        let (left, top, width, height) = patch(&line, *y, *size);
                        let _ = writeln!(
                            content,
                            "1 1 1 rg {} {} {} {} re f",
                            n(left),
                            n(top),
                            n(width),
                            n(height)
                        );
                    }
                    let _ = writeln!(content, "{} rg", rgb(*c));
                    for placed in &line.glyphs {
                        let key = (placed.face, placed.glyph);
                        glyphs
                            .entry(key)
                            .or_insert_with(|| path(&fonts.outline(key.0, key.1)));
                        // The font's units go up the page; the layout's pixels go down it.
                        let _ = writeln!(
                            content,
                            "q {s:.5} 0 0 -{s:.5} {} {} cm /G{}_{} Do Q",
                            n(placed.x),
                            n(placed.y),
                            key.0,
                            key.1,
                            s = placed.scale
                        );
                    }
                    Ok(())
                }
            };
        }
        let stream = objects.stream("", content.as_bytes());
        page_ids.push(objects.add(
            format!(
                "<< /Type /Page /Parent {tree} 0 R /MediaBox [0 0 {} {}] /Contents {stream} 0 R \
                 /Resources {resources} 0 R >>",
                n(width),
                n(height)
            )
            .into_bytes(),
        ));
    }

    let mut named = String::new();
    for ((face, glyph), path) in &glyphs {
        // A box no glyph of a music or text font reaches the edge of.
        let id = objects.stream(
            "/Type /XObject /Subtype /Form /BBox [-8192 -8192 8192 8192]",
            path.as_bytes(),
        );
        let _ = write!(named, "/G{face}_{glyph} {id} 0 R ");
    }
    objects.0[resources - 1] = format!("<< /XObject << {named}>> >>").into_bytes();
    let kids: Vec<String> = page_ids.iter().map(|id| format!("{id} 0 R")).collect();
    objects.0[tree - 1] = format!(
        "<< /Type /Pages /Count {} /Kids [{}] >>",
        page_ids.len(),
        kids.join(" ")
    )
    .into_bytes();
    objects.0[catalog - 1] = format!("<< /Type /Catalog /Pages {tree} 0 R >>").into_bytes();

    // The file: the objects one after another, and then where each one starts.
    let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (index, body) in objects.0.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n", index + 1).into_bytes());
        out.extend(body);
        out.extend(b"\nendobj\n");
    }
    let table = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).into_bytes());
    for offset in offsets {
        out.extend(format!("{offset:010} 00000 n \n").into_bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root {catalog} 0 R /Info {info} 0 R >>\nstartxref\n{table}\n%%EOF\n",
            objects.0.len() + 1
        )
        .into_bytes(),
    );
    out
}

/// A glyph's outline as PDF draws paths, filled.
fn path(outline: &[Segment]) -> String {
    let mut out = String::new();
    let mut at = (0.0, 0.0);
    for segment in outline {
        let _ = match *segment {
            Segment::Move(x, y) => {
                at = (x.into(), y.into());
                writeln!(out, "{} {} m", n(at.0), n(at.1))
            }
            Segment::Line(x, y) => {
                at = (x.into(), y.into());
                writeln!(out, "{} {} l", n(at.0), n(at.1))
            }
            Segment::Quad(cx, cy, x, y) => {
                let to = (f64::from(x), f64::from(y));
                let curve = quad(at, (cx.into(), cy.into()), to);
                at = to;
                writeln!(out, "{curve}")
            }
            Segment::Cubic(ax, ay, bx, by, x, y) => {
                at = (x.into(), y.into());
                writeln!(
                    out,
                    "{} {} {} {} {} {} c",
                    n(ax.into()),
                    n(ay.into()),
                    n(bx.into()),
                    n(by.into()),
                    n(at.0),
                    n(at.1)
                )
            }
            Segment::Close => writeln!(out, "h"),
        };
    }
    out.push_str("f\n");
    out
}
