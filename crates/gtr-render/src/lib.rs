//! Display list backends: a laid-out score written as SVG, as a PDF of its pages, or as
//! a PNG picture. The PDF and the PNG draw their text from font files given to them, as
//! the SVG can, so that what is made looks the same wherever it is opened.

mod fonts;
mod pdf;
mod png;
mod svg;

pub use fonts::{FontError, Fonts};
pub use pdf::pdf;
pub use png::png;
pub use svg::{SvgOptions, svg};
