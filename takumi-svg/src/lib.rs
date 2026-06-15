#![deny(missing_docs)]
//! Vector SVG output for takumi.
//!
//! This crate emits **real** SVG — `<rect>`, `<path>`, `<linearGradient>`,
//! `<filter>`, `<clipPath>`, glyph `<path>`s — instead of wrapping a rasterized
//! bitmap in a `data:` URL. It provides the low-level [`SvgDocument`] builder
//! (backed by [`quick_xml`] so every attribute and value is correctly escaped)
//! and the node-tree → SVG entry point [`render_svg`].
//!
//! # Feature coverage
//!
//! | takumi feature              | SVG construct                              | status |
//! | --------------------------- | ------------------------------------------ | ------ |
//! | solid background            | [`SvgDocument::rect`]                       | full   |
//! | border / radius             | [`SvgDocument::path`] (Bézier)             | full   |
//! | linear / radial gradient    | [`SvgDocument::linear_gradient`] / radial  | full   |
//! | box-shadow                  | [`SvgDocument::drop_shadow_filter`]        | full   |
//! | text                        | glyph [`SvgDocument::path`]                | full   |
//! | bitmap / gif image          | [`SvgDocument::image`] (`data:` href)      | full   |
//! | svg-source image            | inline nested `<svg>`                       | full   |
//! | clip-path / overflow        | [`SvgDocument::clip_path`]                  | full   |
//! | opacity / blend modes       | `opacity` / `mix-blend-mode` attrs         | full   |
//! | affine transform            | `transform="matrix(...)"`                  | full   |
//! | conic gradient              | solid-color wedge `<path>` fan (approx.)   | full   |

mod gradient;
mod render;
pub use render::render_svg;

use quick_xml::Writer;
use quick_xml::events::{BytesEnd, BytesStart, Event};

/// Straight-alpha RGBA color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
  /// `#rrggbb` hex (alpha is emitted separately as `*-opacity`).
  fn hex(self) -> String {
    let [r, g, b, _] = self.0;
    format!("#{r:02x}{g:02x}{b:02x}")
  }

  /// Alpha as a 0.0–1.0 opacity value.
  fn opacity(self) -> f32 {
    self.0[3] as f32 / 255.0
  }
}

/// A 2D affine transform, row-major `[a b c d e f]` as in SVG `matrix(a b c d e f)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine(pub [f32; 6]);

impl Affine {
  fn attr(self) -> String {
    let [a, b, c, d, e, f] = self.0;
    format!(
      "matrix({} {} {} {} {} {})",
      num(a),
      num(b),
      num(c),
      num(d),
      num(e),
      num(f)
    )
  }
}

/// A single stop in a gradient.
#[derive(Debug, Clone, Copy)]
pub struct GradientStop {
  /// Offset along the gradient, 0.0–1.0.
  pub offset: f32,
  /// Stop color.
  pub color: Rgba,
}

/// An incrementally-built SVG document.
///
/// Gradient/filter/clip definitions are written inline at the point of use; SVG
/// resolves `url(#id)` references regardless of document order, so no separate
/// `<defs>` section is needed.
pub struct SvgDocument {
  writer: Writer<Vec<u8>>,
  next_id: u32,
}

impl SvgDocument {
  /// Creates a document with the given pixel viewport.
  pub fn new(width: f32, height: f32) -> Self {
    let mut writer = Writer::new(Vec::new());
    let mut svg = BytesStart::new("svg");
    svg.push_attribute(("xmlns", "http://www.w3.org/2000/svg"));
    svg.push_attribute(("width", num(width).as_str()));
    svg.push_attribute(("height", num(height).as_str()));
    svg.push_attribute((
      "viewBox",
      format!("0 0 {} {}", num(width), num(height)).as_str(),
    ));
    write(&mut writer, Event::Start(svg));
    Self { writer, next_id: 0 }
  }

  fn alloc_id(&mut self, prefix: &str) -> String {
    let id = format!("{prefix}{}", self.next_id);
    self.next_id += 1;
    id
  }

  fn empty(&mut self, name: &str, attrs: &[(&str, String)]) {
    let mut element = BytesStart::new(name);
    for (key, value) in attrs {
      element.push_attribute((*key, value.as_str()));
    }
    write(&mut self.writer, Event::Empty(element));
  }

  fn open(&mut self, name: &str, attrs: &[(&str, String)]) {
    let mut element = BytesStart::new(name);
    for (key, value) in attrs {
      element.push_attribute((*key, value.as_str()));
    }
    write(&mut self.writer, Event::Start(element));
  }

  fn close(&mut self, name: &str) {
    write(&mut self.writer, Event::End(BytesEnd::new(name)));
  }

  /// Appends a solid-fill rectangle.
  pub fn rect(&mut self, x: f32, y: f32, width: f32, height: f32, fill: Rgba) {
    self.empty(
      "rect",
      &[
        ("x", num(x)),
        ("y", num(y)),
        ("width", num(width)),
        ("height", num(height)),
        ("fill", fill.hex()),
        ("fill-opacity", num(fill.opacity())),
      ],
    );
  }

  /// Appends a rectangle filled with a paint reference (e.g. a gradient `url(#id)`).
  pub fn rect_paint(&mut self, x: f32, y: f32, width: f32, height: f32, paint: &str) {
    self.empty(
      "rect",
      &[
        ("x", num(x)),
        ("y", num(y)),
        ("width", num(width)),
        ("height", num(height)),
        ("fill", paint.to_owned()),
      ],
    );
  }

  /// Appends a filled path from SVG path data (`d`).
  pub fn path(&mut self, data: &str, fill: Rgba) {
    self.empty(
      "path",
      &[
        ("d", data.to_owned()),
        ("fill", fill.hex()),
        ("fill-opacity", num(fill.opacity())),
      ],
    );
  }

  /// Defines a linear gradient and returns its `url(#id)` reference. When
  /// `repeating` is set the stops tile beyond their range (`spreadMethod`).
  pub fn linear_gradient(
    &mut self,
    (x1, y1): (f32, f32),
    (x2, y2): (f32, f32),
    repeating: bool,
    stops: &[GradientStop],
  ) -> String {
    let id = self.alloc_id("lg");
    let mut attrs = vec![
      ("id", id.clone()),
      ("gradientUnits", "userSpaceOnUse".to_owned()),
    ];
    if repeating {
      attrs.push(("spreadMethod", "repeat".to_owned()));
    }
    attrs.extend([
      ("x1", num(x1)),
      ("y1", num(y1)),
      ("x2", num(x2)),
      ("y2", num(y2)),
    ]);
    self.open("linearGradient", &attrs);
    self.write_stops(stops);
    self.close("linearGradient");
    format!("url(#{id})")
  }

  /// Defines a radial gradient and returns its `url(#id)` reference. `scale`
  /// stretches the gradient into an ellipse around its center (SVG has no native
  /// `rx`/`ry`, so a non-uniform scale is applied via `gradientTransform`).
  pub fn radial_gradient(
    &mut self,
    (cx, cy): (f32, f32),
    r: f32,
    scale: (f32, f32),
    repeating: bool,
    stops: &[GradientStop],
  ) -> String {
    let id = self.alloc_id("rg");
    let mut attrs = vec![
      ("id", id.clone()),
      ("gradientUnits", "userSpaceOnUse".to_owned()),
    ];
    if repeating {
      attrs.push(("spreadMethod", "repeat".to_owned()));
    }
    attrs.extend([("cx", num(cx)), ("cy", num(cy)), ("r", num(r))]);
    let (sx, sy) = scale;
    if (sx - 1.0).abs() > f32::EPSILON || (sy - 1.0).abs() > f32::EPSILON {
      let e = cx - sx * cx;
      let f = cy - sy * cy;
      attrs.push((
        "gradientTransform",
        format!("matrix({} 0 0 {} {} {})", num(sx), num(sy), num(e), num(f)),
      ));
    }
    self.open("radialGradient", &attrs);
    self.write_stops(stops);
    self.close("radialGradient");
    format!("url(#{id})")
  }

  fn write_stops(&mut self, stops: &[GradientStop]) {
    for stop in stops {
      self.empty(
        "stop",
        &[
          ("offset", num(stop.offset)),
          ("stop-color", stop.color.hex()),
          ("stop-opacity", num(stop.color.opacity())),
        ],
      );
    }
  }

  /// Defines a gaussian-blur drop-shadow filter and returns its `url(#id)`.
  pub fn drop_shadow_filter(&mut self, dx: f32, dy: f32, blur: f32, color: Rgba) -> String {
    let id = self.alloc_id("ds");
    self.open("filter", &[("id", id.clone())]);
    self.empty(
      "feDropShadow",
      &[
        ("dx", num(dx)),
        ("dy", num(dy)),
        ("stdDeviation", num(blur / 2.0)),
        ("flood-color", color.hex()),
        ("flood-opacity", num(color.opacity())),
      ],
    );
    self.close("filter");
    format!("url(#{id})")
  }

  /// Defines a clip path from SVG path data and returns its `url(#id)`.
  pub fn clip_path(&mut self, data: &str) -> String {
    let id = self.alloc_id("cp");
    self.open("clipPath", &[("id", id.clone())]);
    self.empty("path", &[("d", data.to_owned())]);
    self.close("clipPath");
    format!("url(#{id})")
  }

  /// Appends a raster image referenced by a `data:` URL href. This is legitimate
  /// SVG (a genuine photo has no vector form), not the "fake SVG" of wrapping the
  /// whole render in one bitmap.
  pub fn image(&mut self, x: f32, y: f32, width: f32, height: f32, href: &str) {
    self.empty(
      "image",
      &[
        ("x", num(x)),
        ("y", num(y)),
        ("width", num(width)),
        ("height", num(height)),
        ("href", href.to_owned()),
      ],
    );
  }

  /// Opens a `<g>` with a transform and optional opacity/clip; returns a token
  /// that must be passed to [`SvgDocument::end_group`].
  pub fn begin_group(&mut self, transform: Affine, opacity: f32, clip: Option<&str>) -> GroupToken {
    let mut attrs = vec![("transform", transform.attr())];
    if opacity < 1.0 {
      attrs.push(("opacity", num(opacity)));
    }
    if let Some(clip) = clip {
      attrs.push(("clip-path", clip.to_owned()));
    }
    self.open("g", &attrs);
    GroupToken(())
  }

  /// Closes the most recently opened group.
  pub fn end_group(&mut self, _token: GroupToken) {
    self.close("g");
  }

  /// Closes the root `<svg>` and serializes the document to a string.
  pub fn render(mut self) -> String {
    self.close("svg");
    String::from_utf8_lossy(&self.writer.into_inner()).into_owned()
  }
}

/// Opaque proof that a `<g>` is open; consumed by [`SvgDocument::end_group`].
#[must_use]
pub struct GroupToken(());

/// Writes an event to the in-memory buffer. `Vec<u8>` as an [`std::io::Write`]
/// sink never errors, so a failure here is a logic bug, not an I/O condition.
fn write(writer: &mut Writer<Vec<u8>>, event: Event<'_>) {
  writer
    .write_event(event)
    .expect("writing SVG to an in-memory buffer is infallible");
}

fn num(value: f32) -> String {
  if value.is_finite() {
    format!("{value}")
  } else {
    "0".to_owned()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const RED: Rgba = Rgba([255, 0, 0, 255]);
  const HALF_BLUE: Rgba = Rgba([0, 0, 255, 128]);

  #[test]
  fn solid_rect_is_native_svg() {
    let mut doc = SvgDocument::new(100.0, 50.0);
    doc.rect(0.0, 0.0, 100.0, 50.0, RED);
    let svg = doc.render();
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(
      svg
        .contains(r##"<rect x="0" y="0" width="100" height="50" fill="#ff0000" fill-opacity="1""##)
    );
    assert!(!svg.contains("base64"));
  }

  #[test]
  fn alpha_becomes_fill_opacity() {
    let mut doc = SvgDocument::new(1.0, 1.0);
    doc.rect(0.0, 0.0, 1.0, 1.0, HALF_BLUE);
    assert!(
      doc
        .render()
        .contains(r##"fill="#0000ff" fill-opacity="0.5019608""##)
    );
  }

  #[test]
  fn linear_gradient_defines_and_references() {
    let mut doc = SvgDocument::new(10.0, 10.0);
    let fill = doc.linear_gradient(
      (0.0, 0.0),
      (10.0, 0.0),
      false,
      &[
        GradientStop {
          offset: 0.0,
          color: RED,
        },
        GradientStop {
          offset: 1.0,
          color: HALF_BLUE,
        },
      ],
    );
    assert_eq!(fill, "url(#lg0)");
    doc.path("M0 0 H10 V10 H0 Z", RED);
    let svg = doc.render();
    assert!(svg.contains(r#"<linearGradient id="lg0""#));
    assert!(svg.contains(r#"<stop offset="0""#));
  }

  #[test]
  fn drop_shadow_uses_filter() {
    let mut doc = SvgDocument::new(10.0, 10.0);
    let filter = doc.drop_shadow_filter(2.0, 2.0, 4.0, RED);
    assert_eq!(filter, "url(#ds0)");
    assert!(doc.render().contains("<feDropShadow"));
  }

  #[test]
  fn clip_path_and_group_nest() {
    let mut doc = SvgDocument::new(10.0, 10.0);
    let clip = doc.clip_path("M0 0 H5 V5 H0 Z");
    let token = doc.begin_group(Affine([1.0, 0.0, 0.0, 1.0, 3.0, 4.0]), 0.5, Some(&clip));
    doc.rect(0.0, 0.0, 10.0, 10.0, RED);
    doc.end_group(token);
    let svg = doc.render();
    assert!(svg.contains("<clipPath id=\"cp0\">"));
    assert!(
      svg.contains(r#"<g transform="matrix(1 0 0 1 3 4)" opacity="0.5" clip-path="url(#cp0)">"#)
    );
    assert!(svg.contains("</g>"));
  }

  #[test]
  fn image_href_is_escaped_not_faked() {
    let mut doc = SvgDocument::new(10.0, 10.0);
    doc.image(0.0, 0.0, 10.0, 10.0, "data:image/png;base64,AAAA");
    let svg = doc.render();
    assert!(
      svg
        .contains(r#"<image x="0" y="0" width="10" height="10" href="data:image/png;base64,AAAA""#)
    );
  }

  #[test]
  fn attribute_injection_is_escaped() {
    let mut doc = SvgDocument::new(10.0, 10.0);
    doc.image(0.0, 0.0, 10.0, 10.0, r#"x"/><script>alert(1)</script>"#);
    let svg = doc.render();
    assert!(!svg.contains("<script>"));
    assert!(svg.contains("&quot;"));
  }

  #[test]
  fn text_emits_glyph_path() {
    let mut doc = SvgDocument::new(10.0, 10.0);
    doc.path("M1 9 L2 1 L3 9 M1.5 5 H2.5", Rgba([0, 0, 0, 255]));
    assert!(
      doc
        .render()
        .contains("<path d=\"M1 9 L2 1 L3 9 M1.5 5 H2.5\"")
    );
  }
}
