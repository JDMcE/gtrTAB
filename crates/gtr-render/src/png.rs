//! PNG: the display list filled in, pixel by pixel. Everything is a filled shape: a line
//! is a thin rectangle, a curve a ribbon, a glyph its outline.

use ab_glyph_rasterizer::{Point, Rasterizer, point};
use gtr_layout::{Color, Item, Layout};
use miniz_oxide::deflate::compress_to_vec_zlib;

use crate::fonts::{Fonts, Segment, patch};

/// The most pixels a picture is made with; a longer score is drawn smaller to fit.
const MAX_PIXELS: f64 = 48_000_000.0;

const PAPER: [u8; 3] = [255, 255, 255];

fn rgb(color: Color) -> [u8; 3] {
    match color {
        Color::Ink => [0x1b, 0x1b, 0x1f],
        Color::Muted | Color::Error => [0x8e, 0x8e, 0x96],
    }
}

/// A closed outline, in the picture's pixels.
type Contour = Vec<Step>;

#[derive(Clone, Copy)]
enum Step {
    To(Point),
    Quad(Point, Point),
    Cubic(Point, Point, Point),
}

struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl Canvas {
    /// Fills the shape the contours enclose, each starting at its first point.
    fn fill(&mut self, contours: &[(Point, Contour)], color: [u8; 3]) {
        // The box the shape is in, a pixel wider all round, kept to the picture.
        let mut low = point(f32::MAX, f32::MAX);
        let mut high = point(f32::MIN, f32::MIN);
        let mut take = |p: Point| {
            low = point(low.x.min(p.x), low.y.min(p.y));
            high = point(high.x.max(p.x), high.y.max(p.y));
        };
        for (start, steps) in contours {
            take(*start);
            for step in steps {
                match *step {
                    Step::To(p) => take(p),
                    Step::Quad(a, p) => [a, p].into_iter().for_each(&mut take),
                    Step::Cubic(a, b, p) => [a, b, p].into_iter().for_each(&mut take),
                }
            }
        }
        if low.x > high.x
            || !(low.x.is_finite() && high.x.is_finite() && low.y.is_finite() && high.y.is_finite())
        {
            return;
        }
        let left = (low.x.floor() as i64 - 1).clamp(0, self.width as i64);
        let top = (low.y.floor() as i64 - 1).clamp(0, self.height as i64);
        let right = (high.x.ceil() as i64 + 1).clamp(0, self.width as i64);
        let bottom = (high.y.ceil() as i64 + 1).clamp(0, self.height as i64);
        // A shape that is partly off the picture is drawn in a box of its own size, and
        // only what is on the picture kept.
        let (x0, y0) = (low.x.floor() as i64 - 1, low.y.floor() as i64 - 1);
        let wide = (high.x.ceil() as i64 + 1 - x0) as usize;
        let tall = (high.y.ceil() as i64 + 1 - y0) as usize;
        if right <= left || bottom <= top || wide == 0 || tall == 0 || wide * tall > 64_000_000 {
            return;
        }
        let mut raster = Rasterizer::new(wide, tall);
        let local = |p: Point| point(p.x - x0 as f32, p.y - y0 as f32);
        for (start, steps) in contours {
            let mut at = local(*start);
            let first = at;
            for step in steps {
                match *step {
                    Step::To(p) => {
                        raster.draw_line(at, local(p));
                        at = local(p);
                    }
                    Step::Quad(a, p) => {
                        raster.draw_quad(at, local(a), local(p));
                        at = local(p);
                    }
                    Step::Cubic(a, b, p) => {
                        raster.draw_cubic(at, local(a), local(b), local(p));
                        at = local(p);
                    }
                }
            }
            if at != first {
                raster.draw_line(at, first);
            }
        }
        let (width, pixels) = (self.width, &mut self.pixels);
        raster.for_each_pixel_2d(|x, y, cover| {
            let (x, y) = (i64::from(x) + x0, i64::from(y) + y0);
            if cover <= 0.0 || x < left || x >= right || y < top || y >= bottom {
                return;
            }
            let cover = cover.min(1.0);
            let at = (y as usize * width + x as usize) * 3;
            for (pixel, ink) in pixels[at..at + 3].iter_mut().zip(color) {
                *pixel = (f32::from(*pixel) * (1.0 - cover) + f32::from(ink) * cover).round() as u8;
            }
        });
    }

    fn polygon(&mut self, corners: &[(f64, f64)], color: [u8; 3]) {
        let mut points = corners.iter().map(|(x, y)| point(*x as f32, *y as f32));
        if let Some(start) = points.next() {
            self.fill(&[(start, points.map(Step::To).collect())], color);
        }
    }

    /// A line drawn through `points`, `width` thick: a ribbon with a side each way from
    /// the line.
    fn stroke(&mut self, points: &[(f64, f64)], width: f64, color: [u8; 3]) {
        let half = width / 2.0;
        let count = points.len();
        let mut left = Vec::with_capacity(count);
        let mut right = Vec::with_capacity(count);
        for (index, (x, y)) in points.iter().enumerate() {
            // Square to the line at this point: to the way from the point before to the
            // point after.
            let before = points[index.saturating_sub(1)];
            let after = points[(index + 1).min(count - 1)];
            let (dx, dy) = (after.0 - before.0, after.1 - before.1);
            let length = (dx * dx + dy * dy).sqrt();
            if length == 0.0 {
                continue;
            }
            let (nx, ny) = (-dy / length * half, dx / length * half);
            left.push((x + nx, y + ny));
            right.push((x - nx, y - ny));
        }
        left.extend(right.into_iter().rev());
        self.polygon(&left, color);
    }
}

/// Points along a curve with one control point.
fn along(from: (f64, f64), control: (f64, f64), to: (f64, f64)) -> Vec<(f64, f64)> {
    const STEPS: usize = 24;
    (0..=STEPS)
        .map(|step| {
            let t = step as f64 / STEPS as f64;
            let part = |a: f64, c: f64, b: f64| {
                (1.0 - t) * (1.0 - t) * a + 2.0 * (1.0 - t) * t * c + t * t * b
            };
            (part(from.0, control.0, to.0), part(from.1, control.1, to.1))
        })
        .collect()
}

/// The size a picture of a layout is made, in pixels, and the scale it is drawn at: the
/// scale asked for, or less if that would make more pixels than a picture is allowed.
fn fitted(width: f64, height: f64, scale: f64) -> (usize, usize, f64) {
    let area = (width * height).max(1.0);
    let scale = scale.clamp(0.1, 8.0).min((MAX_PIXELS / area).sqrt());
    let pixels = |length: f64| ((length * scale).ceil() as usize).max(1);
    (pixels(width), pixels(height), scale)
}

/// Draws a laid-out score as a PNG, `scale` pixels of the picture to one of the layout:
/// two for a picture sharp enough to print small. A score too long for that is drawn as
/// large as the limit on a picture's size lets it be.
pub fn png(layout: &Layout, fonts: &Fonts, scale: f64) -> Vec<u8> {
    let (width, height, scale) = fitted(layout.width, layout.height, scale);
    let mut canvas = Canvas {
        width,
        height,
        pixels: vec![255; width * height * 3],
    };
    let s = |value: f64| value * scale;
    for item in &layout.items {
        match item {
            Item::Line {
                x1,
                y1,
                x2,
                y2,
                w,
                c,
            } => {
                canvas.stroke(&[(s(*x1), s(*y1)), (s(*x2), s(*y2))], s(*w), rgb(*c));
            }
            Item::Rect { x, y, w, h, c } => {
                let (x, y, w, h) = (s(*x), s(*y), s(*w), s(*h));
                canvas.polygon(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)], rgb(*c));
            }
            Item::Dot { x, y, r } => {
                let around: Vec<(f64, f64)> = (0..32)
                    .map(|step| {
                        let angle = f64::from(step) / 32.0 * std::f64::consts::TAU;
                        (s(x + r * angle.cos()), s(y + r * angle.sin()))
                    })
                    .collect();
                canvas.polygon(&around, rgb(Color::Ink));
            }
            Item::Beam { x1, y1, x2, y2, h } => canvas.polygon(
                &[
                    (s(*x1), s(*y1)),
                    (s(*x2), s(*y2)),
                    (s(*x2), s(y2 + h)),
                    (s(*x1), s(y1 + h)),
                ],
                rgb(Color::Ink),
            ),
            Item::Arc { x1, x2, y, h } => {
                let control = (s((x1 + x2) / 2.0), s(y - h * 2.0));
                let points = along((s(*x1), s(*y)), control, (s(*x2), s(*y)));
                canvas.stroke(&points, s(1.1), rgb(Color::Ink));
            }
            Item::Curve {
                x1,
                y1,
                cx,
                cy,
                x2,
                y2,
            } => {
                let points = along((s(*x1), s(*y1)), (s(*cx), s(*cy)), (s(*x2), s(*y2)));
                canvas.stroke(&points, s(1.1), rgb(Color::Ink));
            }
            Item::Text {
                x,
                y,
                s: text,
                size,
                font,
                align,
                knockout,
                c,
            } => {
                let line = fonts.set(text, *x, *y, *size, *font, *align);
                if *knockout {
                    let (left, top, wide, tall) = patch(&line, *y, *size);
                    let (x, y, w, h) = (s(left), s(top), s(wide), s(tall));
                    canvas.polygon(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)], PAPER);
                }
                for placed in &line.glyphs {
                    // The font's units go up the page; the picture's pixels go down it.
                    let to = |x: f32, y: f32| {
                        point(
                            s(placed.x + f64::from(x) * placed.scale) as f32,
                            s(placed.y - f64::from(y) * placed.scale) as f32,
                        )
                    };
                    let mut contours: Vec<(Point, Contour)> = Vec::new();
                    for segment in fonts.outline(placed.face, placed.glyph) {
                        let step = match segment {
                            Segment::Move(x, y) => {
                                contours.push((to(x, y), Vec::new()));
                                continue;
                            }
                            Segment::Line(x, y) => Step::To(to(x, y)),
                            Segment::Quad(ax, ay, x, y) => Step::Quad(to(ax, ay), to(x, y)),
                            Segment::Cubic(ax, ay, bx, by, x, y) => {
                                Step::Cubic(to(ax, ay), to(bx, by), to(x, y))
                            }
                            Segment::Close => continue,
                        };
                        if let Some((_, steps)) = contours.last_mut() {
                            steps.push(step);
                        }
                    }
                    canvas.fill(&contours, rgb(*c));
                }
            }
        }
    }
    encode(&canvas)
}

fn crc(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                crc >> 1 ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// The picture as a PNG file: eight bits each of red, green and blue.
fn encode(canvas: &Canvas) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8; 4], data: &[u8]| {
        out.extend((data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend(data);
        out.extend(&body);
        out.extend(crc(&body).to_be_bytes());
    };
    let mut header = Vec::new();
    header.extend((canvas.width as u32).to_be_bytes());
    header.extend((canvas.height as u32).to_be_bytes());
    header.extend([8, 2, 0, 0, 0]);
    chunk(b"IHDR", &header);
    // Each row of pixels, with a byte before it to say it is stored as it stands.
    let row = canvas.width * 3;
    let mut rows = Vec::with_capacity((row + 1) * canvas.height);
    for line in canvas.pixels.chunks_exact(row) {
        rows.push(0);
        rows.extend(line);
    }
    chunk(b"IDAT", &compress_to_vec_zlib(&rows, 6));
    chunk(b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_no_bigger_than_the_limit() {
        assert_eq!(fitted(800.0, 337.5, 2.0), (1600, 675, 2.0));
        // A score far too long for a picture at the scale asked for is drawn smaller.
        let (width, height, scale) = fitted(1000.0, 400_000.0, 2.0);
        assert!(scale < 0.5 && (width * height) as f64 <= MAX_PIXELS * 1.01);
        assert_eq!(fitted(0.0, 0.0, 2.0), (1, 1, 2.0));
        // The check value every PNG ends with.
        assert_eq!(crc(b"IEND"), 0xAE42_6082);
    }
}
