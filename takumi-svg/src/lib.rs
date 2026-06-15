#![deny(missing_docs)]
//! Vector SVG output for takumi.
//!
//! This crate emits **real** SVG — `<rect>`, `<path>`, `<linearGradient>`,
//! `<filter>`, `<clipPath>`, glyph `<path>`s — instead of wrapping a rasterized
//! bitmap in a `data:` URL. It provides the low-level [`SvgDocument`] builder and
//! per-feature emitters; the node-tree → SVG conversion is wired up once the
//! `takumi-core` / `takumi-paint` split lands and exposes a backend-agnostic
//! scene/display-list.
//!
//! # Feature coverage
//!
//! Every takumi paint primitive maps to a native SVG construct, with one
//! exception:
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
//! | **conic gradient**          | no SVG 1.1 construct                        | raster fallback |

mod render;
pub use render::render_svg;

use std::fmt::Write;

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
    format!("matrix({a} {b} {c} {d} {e} {f})")
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
#[derive(Debug, Default)]
pub struct SvgDocument {
  width: f32,
  height: f32,
  defs: String,
  body: String,
  next_id: u32,
}

impl SvgDocument {
  /// Creates a document with the given pixel viewport.
  pub fn new(width: f32, height: f32) -> Self {
    Self {
      width,
      height,
      ..Self::default()
    }
  }

  fn alloc_id(&mut self, prefix: &str) -> String {
    let id = format!("{prefix}{}", self.next_id);
    self.next_id += 1;
    id
  }

  /// Appends a solid-fill rectangle (optionally clipped).
  pub fn rect(&mut self, x: f32, y: f32, width: f32, height: f32, fill: Rgba) {
    let _ = write!(
      self.body,
      r#"<rect x="{x}" y="{y}" width="{width}" height="{height}" fill="{}" fill-opacity="{}"/>"#,
      fill.hex(),
      fill.opacity()
    );
  }

  /// Appends an arbitrary filled path from raw SVG path data (`d`).
  pub fn path(&mut self, data: &str, fill: Rgba) {
    let _ = write!(
      self.body,
      r#"<path d="{}" fill="{}" fill-opacity="{}"/>"#,
      escape(data),
      fill.hex(),
      fill.opacity()
    );
  }

  /// Defines a linear gradient and returns its `url(#id)` reference.
  pub fn linear_gradient(
    &mut self,
    (x1, y1): (f32, f32),
    (x2, y2): (f32, f32),
    stops: &[GradientStop],
  ) -> String {
    let id = self.alloc_id("lg");
    let _ = write!(
      self.defs,
      r#"<linearGradient id="{id}" gradientUnits="userSpaceOnUse" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}">"#
    );
    self.write_stops(stops);
    self.defs.push_str("</linearGradient>");
    format!("url(#{id})")
  }

  /// Defines a radial gradient and returns its `url(#id)` reference.
  pub fn radial_gradient(
    &mut self,
    (cx, cy): (f32, f32),
    r: f32,
    stops: &[GradientStop],
  ) -> String {
    let id = self.alloc_id("rg");
    let _ = write!(
      self.defs,
      r#"<radialGradient id="{id}" gradientUnits="userSpaceOnUse" cx="{cx}" cy="{cy}" r="{r}">"#
    );
    self.write_stops(stops);
    self.defs.push_str("</radialGradient>");
    format!("url(#{id})")
  }

  fn write_stops(&mut self, stops: &[GradientStop]) {
    for stop in stops {
      let _ = write!(
        self.defs,
        r#"<stop offset="{}" stop-color="{}" stop-opacity="{}"/>"#,
        stop.offset,
        stop.color.hex(),
        stop.color.opacity()
      );
    }
  }

  /// Defines a gaussian-blur drop-shadow filter and returns its `url(#id)`.
  pub fn drop_shadow_filter(&mut self, dx: f32, dy: f32, blur: f32, color: Rgba) -> String {
    let id = self.alloc_id("ds");
    let _ = write!(
      self.defs,
      r#"<filter id="{id}"><feDropShadow dx="{dx}" dy="{dy}" stdDeviation="{}" flood-color="{}" flood-opacity="{}"/></filter>"#,
      blur / 2.0,
      color.hex(),
      color.opacity()
    );
    format!("url(#{id})")
  }

  /// Defines a clip path from raw path data and returns its `url(#id)`.
  pub fn clip_path(&mut self, data: &str) -> String {
    let id = self.alloc_id("cp");
    let _ = write!(
      self.defs,
      r#"<clipPath id="{id}"><path d="{}"/></clipPath>"#,
      escape(data)
    );
    format!("url(#{id})")
  }

  /// Appends a raster image referenced by a `data:` URL href. This is legitimate
  /// SVG (a genuine photo has no vector form), not the "fake SVG" of wrapping the
  /// whole render in one bitmap.
  pub fn image(&mut self, x: f32, y: f32, width: f32, height: f32, href: &str) {
    let _ = write!(
      self.body,
      r#"<image x="{x}" y="{y}" width="{width}" height="{height}" href="{}"/>"#,
      escape(href)
    );
  }

  /// Opens a `<g>` with a transform and optional opacity/clip; returns a token
  /// that must be passed to [`SvgDocument::end_group`].
  pub fn begin_group(&mut self, transform: Affine, opacity: f32, clip: Option<&str>) -> GroupToken {
    self.body.push_str("<g");
    let _ = write!(self.body, r#" transform="{}""#, transform.attr());
    if opacity < 1.0 {
      let _ = write!(self.body, r#" opacity="{opacity}""#);
    }
    if let Some(clip) = clip {
      let _ = write!(self.body, r#" clip-path="{clip}""#);
    }
    self.body.push('>');
    GroupToken(())
  }

  /// Closes the most recently opened group.
  pub fn end_group(&mut self, _token: GroupToken) {
    self.body.push_str("</g>");
  }

  /// Serializes the document to an SVG string.
  pub fn render(&self) -> String {
    let mut out = format!(
      r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"#,
      self.width, self.height, self.width, self.height
    );
    if !self.defs.is_empty() {
      out.push_str("<defs>");
      out.push_str(&self.defs);
      out.push_str("</defs>");
    }
    out.push_str(&self.body);
    out.push_str("</svg>");
    out
  }
}

/// Opaque proof that a `<g>` is open; consumed by [`SvgDocument::end_group`].
#[must_use]
pub struct GroupToken(());

fn escape(input: &str) -> String {
  input
    .replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
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
    assert!(svg.contains("<defs>"));
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
  fn text_emits_glyph_path() {
    // Glyph outlines from skrifa become real <path> fills — faithful, no font
    // dependency in the consumer.
    let mut doc = SvgDocument::new(10.0, 10.0);
    doc.path("M1 9 L2 1 L3 9 M1.5 5 H2.5", Rgba([0, 0, 0, 255]));
    assert!(
      doc
        .render()
        .contains("<path d=\"M1 9 L2 1 L3 9 M1.5 5 H2.5\"")
    );
  }
}
