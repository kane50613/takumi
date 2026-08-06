// Vendored from krilla 0.8.2 (https://github.com/LaurenzV/krilla, MIT OR
// Apache-2.0), stripped of the `pdf`, `simple-text`, `comemo` and `rayon`
// features.

/*!
A high-level, ergonomic Rust library creating PDF documents.

[krilla] is a high-level Rust crate that allows for the creation of PDF files. It builds
on top of the [pdf-writer] crate, but abstracts away all complexities that are
involved in creating a PDF file, instead providing an interface with high-level primitives, such
as fills, strokes, gradient, glyphs and images which can be used and combined easily
without having to worry about low-level details.

To get started, take a look at the [`document`] module that explains how you can create
a document using krilla.

# Example

The following example shows some of the features of krilla in action.

The example creates a PDF file with two pages. On the first page,
we add two small pieces of text, and on the second page we draw a triangle
with a gradient fill.

For more examples, feel free to take a look at the [examples] directory of the GitHub repository.

```ignore
use std::path::{self, PathBuf};

use krilla::color::rgb;
use krilla::text::Font;
use krilla::geom::{Point, PathBuilder};
use krilla::paint::{SpreadMethod, LinearGradient, Stop, FillRule};
use krilla::text::TextDirection;
use krilla::paint::Fill;
use krilla::Document;
use krilla::page::PageSettings;
use krilla::num::NormalizedF32;

// Create a new document.
let mut document = Document::new();
// Load a font.
let font = {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts/NotoSans-Regular.ttf");
    let data = std::fs::read(&path).unwrap();
    Font::new(data.into(), 0).unwrap()
};

// Add a new page with dimensions 200x200.
let mut page = document.start_page_with(PageSettings::from_wh(200.0, 200.0).unwrap());
// Get the surface of the page.
let mut surface = page.surface();
// Draw some text.
surface.draw_text(
    Point::from_xy(0.0, 25.0),
    font.clone(),
    14.0,
    "This text has font size 14!",
    false,
    TextDirection::Auto
);

surface.set_fill(Some(Fill {
    paint: rgb::Color::new(255, 0, 0).into(),
    opacity: NormalizedF32::new(0.5).unwrap(),
    rule: Default::default(),
}));
// Draw some more text, in a different color with an opacity and bigger font size.
surface.draw_text(
    Point::from_xy(0.0, 50.0),
    font.clone(),
    16.0,
    "This text has font size 16!",
    false,
    TextDirection::Auto
);

// Finish the page.
surface.finish();
page.finish();

// Start a new page.
let mut page = document.start_page_with(PageSettings::from_wh(200.0, 200.0).unwrap());
// Create the triangle.
let triangle = {
    let mut pb = PathBuilder::new();
    pb.move_to(100.0, 20.0);
    pb.line_to(160.0, 160.0);
    pb.line_to(40.0, 160.0);
    pb.close();

    pb.finish().unwrap()
};

// Create the linear gradient.
let lg = LinearGradient {
    x1: 60.0,
    y1: 0.0,
    x2: 140.0,
    y2: 0.0,
    transform: Default::default(),
    spread_method: SpreadMethod::Repeat,
    stops: vec![
        Stop {
            offset: NormalizedF32::new(0.2).unwrap(),
            color: rgb::Color::new(255, 0, 0).into(),
            opacity: NormalizedF32::ONE,
        },
        Stop {
            offset: NormalizedF32::new(0.8).unwrap(),
            color: rgb::Color::new(255, 255, 0).into(),
            opacity: NormalizedF32::ONE,
        },
    ],
    anti_alias: false,
};
let mut surface = page.surface();

// Set the fill.
surface.set_fill(Some(Fill {
    paint: lg.into(),
    rule: FillRule::EvenOdd,
    opacity: NormalizedF32::ONE
}));

// Fill the path.
surface.draw_path(&triangle);

// Finish up and write the resulting PDF.
surface.finish();
page.finish();

let pdf = document.finish().unwrap();
let path = path::absolute("basic.pdf").unwrap();
eprintln!("Saved PDF to '{}'", path.display());

// Write the PDF to a file.
std::fs::write(path, &pdf).unwrap();
```

# Minimum Supported Rust Version (MSRV)

The minimum supported Rust version is **1.92**.

[krilla]: https://github.com/LaurenzV/krilla
[pdf-writer]: https://github.com/typst/pdf-writer
[examples]: https://github.com/LaurenzV/krilla/tree/main/crates/krilla/examples
[`document`]: crate::krilla::document
*/

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod chunk_container;
mod graphics;
mod interactive;
mod interchange;
mod object_stream;
mod resource;
mod serialize;
mod util;

pub(crate) mod content;
pub(crate) mod data;

pub mod configure;
pub mod document;
pub mod error;
pub mod geom;
pub mod num;
pub mod page;
pub mod stream;
pub mod surface;
pub mod text;

pub use data::*;
pub use document::*;
pub use graphics::*;
pub use interactive::*;
pub use interchange::*;
pub use serialize::SerializeSettings;
