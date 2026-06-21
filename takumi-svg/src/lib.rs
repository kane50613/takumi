#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Vector SVG output for takumi.
//!
//! [`render`] turns a takumi node tree into real SVG (`<rect>`, `<path>`,
//! `<linearGradient>`/`<radialGradient>`, `<filter>`, `<clipPath>`, glyph outline
//! `<path>`s, embedded `<image>`) rather than wrapping a rasterized bitmap in a
//! `data:` URL. The document is built with [`quick_xml`] so every attribute and
//! value is correctly escaped.
//!
//! Coverage: backgrounds, borders, border-radius (backgrounds/clip), linear and
//! radial gradients (conic via a wedge-path approximation), box-shadow, text
//! (glyph outlines, decorations, text-shadow, `-webkit-text-stroke`), bitmap/
//! emoji glyphs and images, clip-path/overflow, opacity, and affine transforms.

mod box_model;
mod gradient;
mod image;
mod render;
mod scene_emit;
mod text;
pub use render::{SvgOptions, render};

use std::{borrow::Cow, fmt, fmt::Write as _, io};

use quick_xml::{
  Writer,
  events::{BytesEnd, BytesStart, Event},
};

/// Straight-alpha RGBA color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rgba(pub [u8; 4]);

impl Rgba {
  /// `#rgb` or `#rrggbb` hex (alpha is emitted separately as `*-opacity`). The
  /// short form is used when each channel's nibbles match, which SVG expands back
  /// to the same color.
  fn hex(self) -> String {
    let [r, g, b, _] = self.0;
    let collapsible = |c: u8| c >> 4 == c & 0x0f;
    if collapsible(r) && collapsible(g) && collapsible(b) {
      format!("#{:x}{:x}{:x}", r & 0x0f, g & 0x0f, b & 0x0f)
    } else {
      format!("#{r:02x}{g:02x}{b:02x}")
    }
  }

  /// Alpha as a 0.0–1.0 opacity value.
  fn opacity(self) -> f32 {
    self.0[3] as f32 / 255.0
  }
}

use taffy::Size;
use takumi_core::{
  layout::style::{Affine, Color, Filter, LUMA_WEIGHTS, SEPIA_WEIGHTS, SizingContext},
  shadow::SizedShadow,
};

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
    // Indent so the emitted SVG is one element per line and reviewable in a diff.
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
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

  fn empty(&mut self, name: &str, attrs: &[(&str, Cow<'_, str>)]) -> io::Result<()> {
    let mut element = BytesStart::new(name);
    for (key, value) in attrs {
      element.push_attribute((*key, value.as_ref()));
    }
    self.writer.write_event(Event::Empty(element))
  }

  fn open(&mut self, name: &str, attrs: &[(&str, Cow<'_, str>)]) -> io::Result<()> {
    let mut element = BytesStart::new(name);
    for (key, value) in attrs {
      element.push_attribute((*key, value.as_ref()));
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
    let mut attrs: Vec<(&str, Cow<'_, str>)> = vec![
      ("x", num(x).into()),
      ("y", num(y).into()),
      ("width", num(width).into()),
      ("height", num(height).into()),
      ("fill", fill.hex().into()),
    ];
    push_opacity(&mut attrs, "fill-opacity", fill.opacity());
    self.empty("rect", &attrs)
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
        ("x", num(x).into()),
        ("y", num(y).into()),
        ("width", num(width).into()),
        ("height", num(height).into()),
        ("fill", paint.into()),
      ],
    )
  }

  /// Appends a solid-fill path from SVG path data (`d`).
  pub(crate) fn path(&mut self, data: &str, fill: Rgba) -> io::Result<()> {
    let mut attrs: Vec<(&str, Cow<'_, str>)> =
      vec![("d", data.into()), ("fill", fill.hex().into())];
    push_opacity(&mut attrs, "fill-opacity", fill.opacity());
    self.empty("path", &attrs)
  }

  /// Appends a filled path using the even-odd fill rule (for ring shapes such as
  /// rounded borders: an outer subpath with an inner subpath punched out).
  pub(crate) fn path_evenodd(&mut self, data: &str, fill: Rgba) -> io::Result<()> {
    let mut attrs: Vec<(&str, Cow<'_, str>)> =
      vec![("d", data.into()), ("fill", fill.hex().into())];
    push_opacity(&mut attrs, "fill-opacity", fill.opacity());
    attrs.push(("fill-rule", "evenodd".into()));
    self.empty("path", &attrs)
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
    let reference = format!("url(#{id})");
    let mut attrs = vec![
      ("id", id.into()),
      ("gradientUnits", "userSpaceOnUse".into()),
    ];
    if repeating {
      attrs.push(("spreadMethod", "repeat".into()));
    }
    attrs.extend([
      ("x1", num(x1).into()),
      ("y1", num(y1).into()),
      ("x2", num(x2).into()),
      ("y2", num(y2).into()),
    ]);
    self.open("linearGradient", &attrs)?;
    self.write_stops(stops)?;
    self.close("linearGradient")?;
    Ok(reference)
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
    let reference = format!("url(#{id})");
    let mut attrs = vec![
      ("id", id.into()),
      ("gradientUnits", "userSpaceOnUse".into()),
    ];
    if repeating {
      attrs.push(("spreadMethod", "repeat".into()));
    }
    attrs.extend([
      ("cx", num(cx).into()),
      ("cy", num(cy).into()),
      ("r", num(r).into()),
    ]);
    let (sx, sy) = scale;
    if (sx - 1.0).abs() > f32::EPSILON || (sy - 1.0).abs() > f32::EPSILON {
      let e = cx - sx * cx;
      let f = cy - sy * cy;
      attrs.push((
        "gradientTransform",
        format!("matrix({} 0 0 {} {} {})", num(sx), num(sy), num(e), num(f)).into(),
      ));
    }
    self.open("radialGradient", &attrs)?;
    self.write_stops(stops)?;
    self.close("radialGradient")?;
    Ok(reference)
  }

  fn write_stops(&mut self, stops: &[GradientStop]) -> io::Result<()> {
    for stop in stops {
      let mut attrs: Vec<(&str, Cow<'_, str>)> = vec![
        ("offset", num(stop.offset).into()),
        ("stop-color", stop.color.hex().into()),
      ];
      push_opacity(&mut attrs, "stop-opacity", stop.color.opacity());
      self.empty("stop", &attrs)?;
    }
    Ok(())
  }

  /// Defines a clip path from SVG path data and returns its `url(#id)`.
  pub(crate) fn clip_path(&mut self, data: &str) -> io::Result<String> {
    self.clip_path_impl(data, false)
  }

  /// Like [`SvgDocument::clip_path`] but with the even-odd clip rule, for ring
  /// shapes (an outer subpath with an inner subpath punched out).
  pub(crate) fn clip_path_evenodd(&mut self, data: &str) -> io::Result<String> {
    self.clip_path_impl(data, true)
  }

  fn clip_path_impl(&mut self, data: &str, even_odd: bool) -> io::Result<String> {
    let id = self.alloc_id("cp");
    let reference = format!("url(#{id})");
    self.open("clipPath", &[("id", id.into())])?;
    let mut attrs: Vec<(&str, Cow<'_, str>)> = vec![("d", data.into())];
    if even_odd {
      attrs.push(("clip-rule", "evenodd".into()));
    }
    self.empty("path", &attrs)?;
    self.close("clipPath")?;
    Ok(reference)
  }

  /// Opens a `<mask>` in user space (`maskUnits="userSpaceOnUse"`) and returns the
  /// open token plus its `url(#id)` reference. Content emitted before
  /// [`SvgDocument::end_mask`] is the mask source; CSS `mask-image` defaults to
  /// alpha masking, so `mask-type="alpha"` is set (the mask's alpha attenuates the
  /// masked element rather than its luminance).
  pub(crate) fn begin_mask(&mut self) -> io::Result<(GroupToken, String)> {
    let id = self.alloc_id("mk");
    let reference = format!("url(#{id})");
    self.open(
      "mask",
      &[
        ("id", id.into()),
        ("maskUnits", "userSpaceOnUse".into()),
        ("style", "mask-type:alpha".into()),
      ],
    )?;
    Ok((GroupToken(()), reference))
  }

  /// Closes the most recently opened mask.
  pub(crate) fn end_mask(&mut self, _token: GroupToken) -> io::Result<()> {
    self.close("mask")
  }

  /// Opens a `<g mask="url(#id)">` and returns its token.
  pub(crate) fn begin_masked_group(&mut self, mask: &str) -> io::Result<GroupToken> {
    self.open("g", &[("mask", mask.into())])?;
    Ok(GroupToken(()))
  }

  /// Opens a `<pattern>` tile in user space at `(x, y)` with the given tile size
  /// and returns the open token plus its `url(#id)` reference. Content emitted
  /// before [`SvgDocument::end_pattern`] becomes one tile; fill a rect with the
  /// returned reference to tile it across the box.
  pub(crate) fn begin_pattern(
    &mut self,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
  ) -> io::Result<(GroupToken, String)> {
    let id = self.alloc_id("pat");
    let reference = format!("url(#{id})");
    self.open(
      "pattern",
      &[
        ("id", id.into()),
        ("patternUnits", "userSpaceOnUse".into()),
        ("x", num(x).into()),
        ("y", num(y).into()),
        ("width", num(width).into()),
        ("height", num(height).into()),
      ],
    )?;
    Ok((GroupToken(()), reference))
  }

  /// Closes the most recently opened pattern.
  pub(crate) fn end_pattern(&mut self, _token: GroupToken) -> io::Result<()> {
    self.close("pattern")
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
      ("x", num(x).into()),
      ("y", num(y).into()),
      ("width", num(width).into()),
      ("height", num(height).into()),
      ("href", href.into()),
    ];
    if let Some(par) = preserve_aspect_ratio {
      attrs.push(("preserveAspectRatio", par.into()));
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
    let mut attrs: Vec<(&str, Cow<'_, str>)> = Vec::with_capacity(4);
    if !transform.is_identity() {
      attrs.push(("transform", matrix_attr(transform).into()));
    }
    if opacity < 1.0 {
      attrs.push(("opacity", num(opacity).into()));
    }
    if let Some(clip) = clip {
      attrs.push(("clip-path", clip.into()));
    }
    if let Some(filter) = filter {
      attrs.push(("filter", filter.into()));
    }
    self.open("g", &attrs)?;
    Ok(GroupToken(()))
  }

  /// Opens a `<g>` carrying a `mix-blend-mode` so the wrapped subtree composites
  /// against its backdrop. Returns a token for [`SvgDocument::end_group`].
  pub(crate) fn begin_blend_group(&mut self, mix_blend_mode: &str) -> io::Result<GroupToken> {
    self.open(
      "g",
      &[("style", format!("mix-blend-mode:{mix_blend_mode}").into())],
    )?;
    Ok(GroupToken(()))
  }

  /// Opens a `<g style="isolation:isolate">` establishing an isolated group, so a
  /// descendant's `mix-blend-mode` composites within the subtree rather than
  /// against the page backdrop. Returns a token for [`SvgDocument::end_group`].
  pub(crate) fn begin_isolate_group(&mut self) -> io::Result<GroupToken> {
    self.open("g", &[("style", "isolation:isolate".into())])?;
    Ok(GroupToken(()))
  }

  /// Appends a filled path with an optional stroke (for `-webkit-text-stroke`).
  /// The stroke is `(color, width, line-join)`; raster joins miter/round/bevel,
  /// so emitting `stroke-linejoin` avoids miter spikes on glyph corners.
  pub(crate) fn glyph_path(
    &mut self,
    data: &str,
    fill: Rgba,
    stroke: Option<(Rgba, f32, &str)>,
  ) -> io::Result<()> {
    let mut attrs: Vec<(&str, Cow<'_, str>)> =
      vec![("d", data.into()), ("fill", fill.hex().into())];
    push_opacity(&mut attrs, "fill-opacity", fill.opacity());
    if let Some((color, width, line_join)) = stroke {
      attrs.push(("stroke", color.hex().into()));
      push_opacity(&mut attrs, "stroke-opacity", color.opacity());
      attrs.push(("stroke-width", num(width).into()));
      if line_join != "miter" {
        attrs.push(("stroke-linejoin", line_join.into()));
      }
    }
    self.empty("path", &attrs)
  }

  /// Defines a gaussian-blur filter (for text-shadow) and returns its `url(#id)`.
  pub(crate) fn blur_filter(&mut self, std_deviation: f32) -> io::Result<String> {
    let id = self.alloc_id("bl");
    let reference = format!("url(#{id})");
    self.open("filter", &[("id", id.into())])?;
    self.empty(
      "feGaussianBlur",
      &[("stdDeviation", num(std_deviation).into())],
    )?;
    self.close("filter")?;
    Ok(reference)
  }

  /// Defines a CSS `filter` chain as an SVG `<filter>` and returns its
  /// `url(#id)` (or `None` if the list is empty). Primitives are chained with
  /// `result="fN"`/`in="f(N-1)"`; the region is widened so blur/shadow are not
  /// clipped. `size` is the element's border-box size, the resolution basis for
  /// `drop-shadow` lengths (mirroring the raster backend).
  pub(crate) fn filter(
    &mut self,
    filters: &[Filter],
    sizing: &SizingContext,
    current_color: Color,
    size: Size<f32>,
  ) -> io::Result<Option<String>> {
    if filters.is_empty() {
      return Ok(None);
    }
    let id = self.alloc_id("ft");
    let reference = format!("url(#{id})");
    self.open(
      "filter",
      &[
        ("id", id.into()),
        ("x", "-50%".into()),
        ("y", "-50%".into()),
        ("width", "200%".into()),
        ("height", "200%".into()),
        ("color-interpolation-filters", "sRGB".into()),
      ],
    )?;

    let mut prev: Cow<'_, str> = "SourceGraphic".into();
    for (index, filter) in filters.iter().enumerate() {
      let result = format!("f{index}");
      self.filter_primitive(filter, &prev, &result, sizing, current_color, size)?;
      prev = result.into();
    }
    self.close("filter")?;
    Ok(Some(reference))
  }

  fn filter_primitive(
    &mut self,
    filter: &Filter,
    input: &str,
    result: &str,
    sizing: &SizingContext,
    current_color: Color,
    size: Size<f32>,
  ) -> io::Result<()> {
    match filter {
      Filter::Blur(length) => self.empty(
        "feGaussianBlur",
        &[
          ("in", input.into()),
          ("stdDeviation", num(length.to_px(sizing, 1.0)).into()),
          ("result", result.into()),
        ],
      ),
      Filter::Brightness(v) => self.component_transfer_rgb(
        input,
        result,
        &[("type", "linear".into()), ("slope", num(v.0).into())],
      ),
      Filter::Contrast(v) => self.component_transfer_rgb(
        input,
        result,
        &[
          ("type", "linear".into()),
          ("slope", num(v.0).into()),
          ("intercept", num(0.5 * (1.0 - v.0)).into()),
        ],
      ),
      Filter::Grayscale(amount) => {
        let a = amount.0.clamp(0.0, 1.0);
        let m = grayscale_matrix(a);
        self.color_matrix(input, result, &m)
      }
      Filter::Saturate(v) => self.empty(
        "feColorMatrix",
        &[
          ("in", input.into()),
          ("type", "saturate".into()),
          ("values", num(v.0).into()),
          ("result", result.into()),
        ],
      ),
      Filter::HueRotate(angle) => self.empty(
        "feColorMatrix",
        &[
          ("in", input.into()),
          ("type", "hueRotate".into()),
          ("values", num((**angle as i32) as f32).into()),
          ("result", result.into()),
        ],
      ),
      Filter::Invert(amount) => {
        let a = amount.0.clamp(0.0, 1.0);
        self.component_transfer_rgb(
          input,
          result,
          &[
            ("type", "table".into()),
            ("tableValues", format!("{} {}", num(a), num(1.0 - a)).into()),
          ],
        )
      }
      Filter::Sepia(amount) => {
        let a = amount.0.clamp(0.0, 1.0);
        let m = sepia_matrix(a);
        self.color_matrix(input, result, &m)
      }
      Filter::Opacity(v) => {
        self.open(
          "feComponentTransfer",
          &[("in", input.into()), ("result", result.into())],
        )?;
        self.empty(
          "feFuncA",
          &[("type", "linear".into()), ("slope", num(v.0).into())],
        )?;
        self.close("feComponentTransfer")
      }
      Filter::DropShadow(shadow) => {
        let resolved = SizedShadow::from_text_shadow(*shadow, sizing, current_color, size);
        let color = Rgba(resolved.color.0);
        self.empty(
          "feGaussianBlur",
          &[
            ("in", "SourceAlpha".into()),
            ("stdDeviation", num(resolved.blur_radius).into()),
            ("result", "dsb".into()),
          ],
        )?;
        self.empty(
          "feOffset",
          &[
            ("in", "dsb".into()),
            ("dx", num(resolved.offset_x).into()),
            ("dy", num(resolved.offset_y).into()),
            ("result", "dso".into()),
          ],
        )?;
        self.empty(
          "feFlood",
          &[
            ("flood-color", color.hex().into()),
            ("flood-opacity", num(color.opacity()).into()),
            ("result", "dsc".into()),
          ],
        )?;
        self.empty(
          "feComposite",
          &[
            ("in", "dsc".into()),
            ("in2", "dso".into()),
            ("operator", "in".into()),
            ("result", "dss".into()),
          ],
        )?;
        self.open("feMerge", &[("result", result.into())])?;
        self.empty("feMergeNode", &[("in", "dss".into())])?;
        self.empty("feMergeNode", &[("in", input.into())])?;
        self.close("feMerge")
      }
    }
  }

  fn component_transfer_rgb(
    &mut self,
    input: &str,
    result: &str,
    func_attrs: &[(&str, Cow<'_, str>)],
  ) -> io::Result<()> {
    self.open(
      "feComponentTransfer",
      &[("in", input.into()), ("result", result.into())],
    )?;
    for func in ["feFuncR", "feFuncG", "feFuncB"] {
      self.empty(func, func_attrs)?;
    }
    self.close("feComponentTransfer")
  }

  fn color_matrix(&mut self, input: &str, result: &str, matrix: &[f32; 20]) -> io::Result<()> {
    let mut values = String::with_capacity(matrix.len() * APPROX_CHARS_PER_NUMBER);
    for (i, value) in matrix.iter().enumerate() {
      if i > 0 {
        values.push(' ');
      }
      let _ = write!(values, "{}", Num(*value));
    }
    self.empty(
      "feColorMatrix",
      &[
        ("in", input.into()),
        ("type", "matrix".into()),
        ("values", values.into()),
        ("result", result.into()),
      ],
    )
  }

  /// Defines a `<clipPath>` from raw SVG path data with an optional transform,
  /// and returns its `url(#id)`. Used for `clip-path: path(...)` whose data is in
  /// box-local coordinates.
  pub(crate) fn clip_path_transformed(
    &mut self,
    data: &str,
    even_odd: bool,
    transform: Option<&str>,
  ) -> io::Result<String> {
    let id = self.alloc_id("cp");
    let reference = format!("url(#{id})");
    self.open("clipPath", &[("id", id.into())])?;
    let mut attrs: Vec<(&str, Cow<'_, str>)> = vec![("d", data.into())];
    if even_odd {
      attrs.push(("clip-rule", "evenodd".into()));
    }
    if let Some(transform) = transform {
      attrs.push(("transform", transform.into()));
    }
    self.empty("path", &attrs)?;
    self.close("clipPath")?;
    Ok(reference)
  }

  /// Defines an elliptical `<clipPath>` and returns its `url(#id)`.
  pub(crate) fn clip_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32) -> io::Result<String> {
    let id = self.alloc_id("cp");
    let reference = format!("url(#{id})");
    self.open("clipPath", &[("id", id.into())])?;
    self.empty(
      "ellipse",
      &[
        ("cx", num(cx).into()),
        ("cy", num(cy).into()),
        ("rx", num(rx).into()),
        ("ry", num(ry).into()),
      ],
    )?;
    self.close("clipPath")?;
    Ok(reference)
  }

  /// Strokes an open/closed path (for dashed/dotted borders). `dasharray` and
  /// `linecap` are optional.
  #[allow(clippy::too_many_arguments)]
  pub(crate) fn stroke_path(
    &mut self,
    data: &str,
    stroke: Rgba,
    width: f32,
    dasharray: Option<&str>,
    linecap: Option<&str>,
  ) -> io::Result<()> {
    let mut attrs: Vec<(&str, Cow<'_, str>)> = vec![
      ("d", data.into()),
      ("fill", "none".into()),
      ("stroke", stroke.hex().into()),
    ];
    push_opacity(&mut attrs, "stroke-opacity", stroke.opacity());
    attrs.push(("stroke-width", num(width).into()));
    if let Some(dasharray) = dasharray {
      attrs.push(("stroke-dasharray", dasharray.into()));
    }
    if let Some(linecap) = linecap {
      attrs.push(("stroke-linecap", linecap.into()));
    }
    self.empty("path", &attrs)
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

pub(crate) fn matrix_attr(transform: Affine) -> String {
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

/// CSS `grayscale(a)` color matrix (spec form: identity lerped toward the luma
/// projection by `a`). Matches the raster backend's luma-lerp.
fn grayscale_matrix(a: f32) -> [f32; 20] {
  let [lr, lg, lb] = LUMA_WEIGHTS;
  let r0 = 1.0 - a + a * lr;
  let g_to_r = a * lg;
  let b_to_r = a * lb;
  let r_to_g = a * lr;
  let g0 = 1.0 - a + a * lg;
  let b_to_g = a * lb;
  let r_to_b = a * lr;
  let g_to_b = a * lg;
  let b0 = 1.0 - a + a * lb;
  [
    r0, g_to_r, b_to_r, 0.0, 0.0, //
    r_to_g, g0, b_to_g, 0.0, 0.0, //
    r_to_b, g_to_b, b0, 0.0, 0.0, //
    0.0, 0.0, 0.0, 1.0, 0.0,
  ]
}

/// CSS `sepia(a)` color matrix (spec form: identity lerped toward the sepia
/// projection by `a`). Matches the raster backend's per-channel sepia lerp.
fn sepia_matrix(a: f32) -> [f32; 20] {
  let lerp = |to: f32, idx_diag: bool| {
    if idx_diag { 1.0 - a + a * to } else { a * to }
  };
  let [[rr, rg, rb], [gr, gg, gb], [br, bg, bb]] = SEPIA_WEIGHTS;
  [
    lerp(rr, true),
    lerp(rg, false),
    lerp(rb, false),
    0.0,
    0.0, //
    lerp(gr, false),
    lerp(gg, true),
    lerp(gb, false),
    0.0,
    0.0, //
    lerp(br, false),
    lerp(bg, false),
    lerp(bb, true),
    0.0,
    0.0, //
    0.0,
    0.0,
    0.0,
    1.0,
    0.0,
  ]
}

/// Quantization grid for coordinates, dimensions, and opacities: three decimals.
/// SVG rendering is insensitive below this at the raster sizes takumi targets, so
/// dropping the float tail keeps documents compact without visible drift.
const COORD_FACTOR: f32 = 1000.0;

/// Rough characters one quantized number serializes to, used to presize buffers.
pub(crate) const APPROX_CHARS_PER_NUMBER: usize = 8;

/// Finite-guarded, quantized float formatter shared by every SVG numeric
/// emission site. Non-finite values serialize as `0`; finite values are rounded
/// to [`COORD_FACTOR`]'s grid and printed with the shortest representation, so
/// trailing zeros are dropped.
pub(crate) struct Num(pub f32);

/// Stack buffer for one formatted number, sidestepping the per-coordinate heap
/// allocation of `f32::to_string`. 32 bytes holds any `f32` Display.
struct NumBuf {
  bytes: [u8; 32],
  len: usize,
}

impl fmt::Write for NumBuf {
  fn write_str(&mut self, s: &str) -> fmt::Result {
    let end = self.len + s.len();
    if end > self.bytes.len() {
      return Err(fmt::Error);
    }
    self.bytes[self.len..end].copy_from_slice(s.as_bytes());
    self.len = end;
    Ok(())
  }
}

impl fmt::Display for Num {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if !self.0.is_finite() {
      return f.write_str("0");
    }
    let value = (self.0 * COORD_FACTOR).round() / COORD_FACTOR;
    if value == 0.0 {
      return f.write_str("0");
    }
    let mut buf = NumBuf {
      bytes: [0; 32],
      len: 0,
    };
    if write!(buf, "{value}").is_err() {
      return write!(f, "{value}");
    }
    let Ok(text) = std::str::from_utf8(&buf.bytes[..buf.len]) else {
      return write!(f, "{value}");
    };
    // Drop the redundant integer-part zero: `0.5` -> `.5`, `-0.5` -> `-.5`.
    if let Some(rest) = text.strip_prefix("0.") {
      f.write_str(".")?;
      f.write_str(rest)
    } else if let Some(rest) = text.strip_prefix("-0.") {
      f.write_str("-.")?;
      f.write_str(rest)
    } else {
      f.write_str(text)
    }
  }
}

fn num(value: f32) -> String {
  Num(value).to_string()
}

/// Pushes an `*-opacity` attribute only when it differs from the SVG default of
/// `1` (fully opaque), so opaque fills stay attribute-free.
fn push_opacity<'a>(attrs: &mut Vec<(&'a str, Cow<'a, str>)>, name: &'a str, opacity: f32) {
  if opacity < 1.0 {
    attrs.push((name, num(opacity).into()));
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
    assert!(svg.contains(r##"<rect x="0" y="0" width="100" height="50" fill="#f00""##));
    assert!(!svg.contains("fill-opacity"));
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
        .contains(r##"fill="#00f" fill-opacity=".502""##)
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
      svg.contains(r#"<g transform="matrix(1 0 0 1 3 4)" opacity=".5" clip-path="url(#cp0)">"#)
    );
    assert!(svg.contains("</g>"));
  }

  #[test]
  fn identity_transform_is_omitted() {
    let mut doc = SvgDocument::new(10.0, 10.0).unwrap();
    let token = doc.begin_group(IDENTITY, 0.5, None, None).unwrap();
    doc.end_group(token).unwrap();
    assert!(doc.render().unwrap().contains("<g opacity=\".5\">"));
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
