#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Vector SVG output for takumi.
//!
//! [`render`] turns a takumi node tree into **real** SVG — `<rect>`,
//! `<path>`, `<linearGradient>`/`<radialGradient>`, `<filter>`, `<clipPath>`,
//! glyph outline `<path>`s, and embedded `<image>` — rather than wrapping a
//! rasterized bitmap in a `data:` URL. The document is built with [`quick_xml`]
//! so every attribute and value is correctly escaped.
//!
//! Coverage: backgrounds, borders, border-radius (backgrounds/clip), linear and
//! radial gradients (conic via a wedge-path approximation), box-shadow, text
//! (glyph outlines, decorations, text-shadow, `-webkit-text-stroke`), bitmap/
//! emoji glyphs and images, clip-path/overflow, opacity, and affine transforms.

mod box_model;
mod gradient;
mod image;
mod render;
mod text;
pub use render::{SvgOptions, render};

use std::io;

use quick_xml::Writer;
use quick_xml::events::{BytesEnd, BytesStart, Event};

/// Straight-alpha RGBA color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rgba(pub [u8; 4]);

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

/// The affine transform type, re-exported from `takumi-core`. Serialized as an
/// SVG `matrix(a b c d e f)` via [`matrix_attr`].
pub(crate) use takumi_core::layout::style::Affine;

pub(crate) const IDENTITY: Affine = Affine::IDENTITY;

/// A single stop in a gradient.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GradientStop {
  /// Offset along the gradient, 0.0–1.0.
  pub offset: f32,
  /// Stop color.
  pub color: Rgba,
}

/// An incrementally-built SVG document.
///
/// Gradient/filter/clip definitions are written inline at the point of use; SVG
/// resolves `url(#id)` references regardless of document order, so no separate
/// `<defs>` section is needed. Each write is forwarded to the underlying
/// [`quick_xml`] writer and surfaces its [`io::Result`].
pub(crate) struct SvgDocument {
  writer: Writer<Vec<u8>>,
  next_id: u32,
}

impl SvgDocument {
  /// Creates a document with the given pixel viewport and writes the root
  /// `<svg>` open tag.
  pub(crate) fn new(width: f32, height: f32) -> io::Result<Self> {
    let mut writer = Writer::new(Vec::new());
    let mut svg = BytesStart::new("svg");
    svg.push_attribute(("xmlns", "http://www.w3.org/2000/svg"));
    svg.push_attribute(("width", num(width).as_str()));
    svg.push_attribute(("height", num(height).as_str()));
    svg.push_attribute((
      "viewBox",
      format!("0 0 {} {}", num(width), num(height)).as_str(),
    ));
    writer.write_event(Event::Start(svg))?;
    Ok(Self { writer, next_id: 0 })
  }

  fn alloc_id(&mut self, prefix: &str) -> String {
    let id = format!("{prefix}{}", self.next_id);
    self.next_id += 1;
    id
  }

  fn empty(&mut self, name: &str, attrs: &[(&str, String)]) -> io::Result<()> {
    let mut element = BytesStart::new(name);
    for (key, value) in attrs {
      element.push_attribute((*key, value.as_str()));
    }
    self.writer.write_event(Event::Empty(element))
  }

  fn open(&mut self, name: &str, attrs: &[(&str, String)]) -> io::Result<()> {
    let mut element = BytesStart::new(name);
    for (key, value) in attrs {
      element.push_attribute((*key, value.as_str()));
    }
    self.writer.write_event(Event::Start(element))
  }

  fn close(&mut self, name: &str) -> io::Result<()> {
    self.writer.write_event(Event::End(BytesEnd::new(name)))
  }

  /// Appends a solid-fill rectangle.
  pub(crate) fn rect(
    &mut self,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    fill: Rgba,
  ) -> io::Result<()> {
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
    )
  }

  /// Appends a rectangle filled with a paint reference (e.g. a gradient `url(#id)`).
  pub(crate) fn rect_paint(
    &mut self,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    paint: &str,
  ) -> io::Result<()> {
    self.empty(
      "rect",
      &[
        ("x", num(x)),
        ("y", num(y)),
        ("width", num(width)),
        ("height", num(height)),
        ("fill", paint.to_owned()),
      ],
    )
  }

  /// Appends a solid-fill path from SVG path data (`d`).
  pub(crate) fn path(&mut self, data: &str, fill: Rgba) -> io::Result<()> {
    self.empty(
      "path",
      &[
        ("d", data.to_owned()),
        ("fill", fill.hex()),
        ("fill-opacity", num(fill.opacity())),
      ],
    )
  }

  /// Defines a linear gradient and returns its `url(#id)` reference. When
  /// `repeating` is set the stops tile beyond their range (`spreadMethod`).
  pub(crate) fn linear_gradient(
    &mut self,
    (x1, y1): (f32, f32),
    (x2, y2): (f32, f32),
    repeating: bool,
    stops: &[GradientStop],
  ) -> io::Result<String> {
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
    self.open("linearGradient", &attrs)?;
    self.write_stops(stops)?;
    self.close("linearGradient")?;
    Ok(format!("url(#{id})"))
  }

  /// Defines a radial gradient and returns its `url(#id)` reference. `scale`
  /// stretches the gradient into an ellipse around its center (SVG has no native
  /// `rx`/`ry`, so a non-uniform scale is applied via `gradientTransform`).
  pub(crate) fn radial_gradient(
    &mut self,
    (cx, cy): (f32, f32),
    r: f32,
    scale: (f32, f32),
    repeating: bool,
    stops: &[GradientStop],
  ) -> io::Result<String> {
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
    self.open("radialGradient", &attrs)?;
    self.write_stops(stops)?;
    self.close("radialGradient")?;
    Ok(format!("url(#{id})"))
  }

  fn write_stops(&mut self, stops: &[GradientStop]) -> io::Result<()> {
    for stop in stops {
      self.empty(
        "stop",
        &[
          ("offset", num(stop.offset)),
          ("stop-color", stop.color.hex()),
          ("stop-opacity", num(stop.color.opacity())),
        ],
      )?;
    }
    Ok(())
  }

  /// Defines a clip path from SVG path data and returns its `url(#id)`.
  pub(crate) fn clip_path(&mut self, data: &str) -> io::Result<String> {
    let id = self.alloc_id("cp");
    self.open("clipPath", &[("id", id.clone())])?;
    self.empty("path", &[("d", data.to_owned())])?;
    self.close("clipPath")?;
    Ok(format!("url(#{id})"))
  }

  /// Appends a raster image referenced by a `data:` URL href. This is legitimate
  /// SVG (a genuine photo has no vector form), not the "fake SVG" of wrapping the
  /// whole render in one bitmap.
  pub(crate) fn image(
    &mut self,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    href: &str,
    preserve_aspect_ratio: Option<&str>,
  ) -> io::Result<()> {
    let mut attrs = vec![
      ("x", num(x)),
      ("y", num(y)),
      ("width", num(width)),
      ("height", num(height)),
      ("href", href.to_owned()),
    ];
    if let Some(par) = preserve_aspect_ratio {
      attrs.push(("preserveAspectRatio", par.to_owned()));
    }
    self.empty("image", &attrs)
  }

  /// Opens a `<g>` with a transform and optional opacity/clip; returns a token
  /// that must be passed to [`SvgDocument::end_group`]. An identity transform is
  /// omitted from the output.
  pub(crate) fn begin_group(
    &mut self,
    transform: Affine,
    opacity: f32,
    clip: Option<&str>,
    filter: Option<&str>,
  ) -> io::Result<GroupToken> {
    let mut attrs = Vec::new();
    if !transform.is_identity() {
      attrs.push(("transform", matrix_attr(transform)));
    }
    if opacity < 1.0 {
      attrs.push(("opacity", num(opacity)));
    }
    if let Some(clip) = clip {
      attrs.push(("clip-path", clip.to_owned()));
    }
    if let Some(filter) = filter {
      attrs.push(("filter", filter.to_owned()));
    }
    self.open("g", &attrs)?;
    Ok(GroupToken(()))
  }

  /// Appends a filled path with an optional stroke (for `-webkit-text-stroke`).
  pub(crate) fn glyph_path(
    &mut self,
    data: &str,
    fill: Rgba,
    stroke: Option<(Rgba, f32)>,
  ) -> io::Result<()> {
    let mut attrs = vec![
      ("d", data.to_owned()),
      ("fill", fill.hex()),
      ("fill-opacity", num(fill.opacity())),
    ];
    if let Some((color, width)) = stroke {
      attrs.push(("stroke", color.hex()));
      attrs.push(("stroke-opacity", num(color.opacity())));
      attrs.push(("stroke-width", num(width)));
    }
    self.empty("path", &attrs)
  }

  /// Defines a gaussian-blur filter (for text-shadow) and returns its `url(#id)`.
  pub(crate) fn blur_filter(&mut self, std_deviation: f32) -> io::Result<String> {
    let id = self.alloc_id("bl");
    self.open("filter", &[("id", id.clone())])?;
    self.empty("feGaussianBlur", &[("stdDeviation", num(std_deviation))])?;
    self.close("filter")?;
    Ok(format!("url(#{id})"))
  }

  /// Closes the most recently opened group.
  pub(crate) fn end_group(&mut self, _token: GroupToken) -> io::Result<()> {
    self.close("g")
  }

  /// Closes the root `<svg>` and serializes the document to a string.
  pub(crate) fn render(mut self) -> io::Result<String> {
    self.close("svg")?;
    Ok(String::from_utf8_lossy(&self.writer.into_inner()).into_owned())
  }
}

/// Opaque proof that a `<g>` is open; consumed by [`SvgDocument::end_group`].
#[must_use]
pub(crate) struct GroupToken(());

fn matrix_attr(transform: Affine) -> String {
  let [a, b, c, d, e, f] = transform.to_cols_array();
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
    let mut doc = SvgDocument::new(100.0, 50.0).unwrap();
    doc.rect(0.0, 0.0, 100.0, 50.0, RED).unwrap();
    let svg = doc.render().unwrap();
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(
      svg
        .contains(r##"<rect x="0" y="0" width="100" height="50" fill="#ff0000" fill-opacity="1""##)
    );
    assert!(!svg.contains("base64"));
  }

  #[test]
  fn alpha_becomes_fill_opacity() {
    let mut doc = SvgDocument::new(1.0, 1.0).unwrap();
    doc.rect(0.0, 0.0, 1.0, 1.0, HALF_BLUE).unwrap();
    assert!(
      doc
        .render()
        .unwrap()
        .contains(r##"fill="#0000ff" fill-opacity="0.5019608""##)
    );
  }

  #[test]
  fn linear_gradient_defines_and_references() {
    let mut doc = SvgDocument::new(10.0, 10.0).unwrap();
    let fill = doc
      .linear_gradient(
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
      )
      .unwrap();
    assert_eq!(fill, "url(#lg0)");
    doc.path("M0 0 H10 V10 H0 Z", RED).unwrap();
    let svg = doc.render().unwrap();
    assert!(svg.contains(r#"<linearGradient id="lg0""#));
    assert!(svg.contains(r#"<stop offset="0""#));
  }

  #[test]
  fn clip_path_and_group_nest() {
    let mut doc = SvgDocument::new(10.0, 10.0).unwrap();
    let clip = doc.clip_path("M0 0 H5 V5 H0 Z").unwrap();
    let token = doc
      .begin_group(Affine::translation(3.0, 4.0), 0.5, Some(&clip), None)
      .unwrap();
    doc.rect(0.0, 0.0, 10.0, 10.0, RED).unwrap();
    doc.end_group(token).unwrap();
    let svg = doc.render().unwrap();
    assert!(svg.contains("<clipPath id=\"cp0\">"));
    assert!(
      svg.contains(r#"<g transform="matrix(1 0 0 1 3 4)" opacity="0.5" clip-path="url(#cp0)">"#)
    );
    assert!(svg.contains("</g>"));
  }

  #[test]
  fn identity_transform_is_omitted() {
    let mut doc = SvgDocument::new(10.0, 10.0).unwrap();
    let token = doc.begin_group(IDENTITY, 0.5, None, None).unwrap();
    doc.end_group(token).unwrap();
    assert!(doc.render().unwrap().contains("<g opacity=\"0.5\">"));
  }

  #[test]
  fn image_href_is_escaped_not_faked() {
    let mut doc = SvgDocument::new(10.0, 10.0).unwrap();
    doc
      .image(0.0, 0.0, 10.0, 10.0, "data:image/png;base64,AAAA", None)
      .unwrap();
    let svg = doc.render().unwrap();
    assert!(
      svg
        .contains(r#"<image x="0" y="0" width="10" height="10" href="data:image/png;base64,AAAA""#)
    );
  }

  #[test]
  fn attribute_injection_is_escaped() {
    let mut doc = SvgDocument::new(10.0, 10.0).unwrap();
    doc
      .image(
        0.0,
        0.0,
        10.0,
        10.0,
        r#"x"/><script>alert(1)</script>"#,
        None,
      )
      .unwrap();
    let svg = doc.render().unwrap();
    assert!(!svg.contains("<script>"));
    assert!(svg.contains("&quot;"));
  }

  #[test]
  fn text_emits_glyph_path() {
    let mut doc = SvgDocument::new(10.0, 10.0).unwrap();
    doc
      .path("M1 9 L2 1 L3 9 M1.5 5 H2.5", Rgba([0, 0, 0, 255]))
      .unwrap();
    assert!(
      doc
        .render()
        .unwrap()
        .contains("<path d=\"M1 9 L2 1 L3 9 M1.5 5 H2.5\"")
    );
  }
}
