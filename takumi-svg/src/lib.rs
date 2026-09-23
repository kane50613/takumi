#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Vector SVG output for takumi.
//!
//! [`render`] turns a node tree into real SVG (`<rect>`, `<path>`,
//! `<linearGradient>`/`<radialGradient>`, `<filter>`, `<clipPath>`, glyph-outline
//! `<path>`s, embedded `<image>`) instead of wrapping a rasterized bitmap in a
//! `data:` URL. [`quick_xml`] builds the document, so every attribute and value
//! is escaped.
//!
//! Coverage:
//!
//! - Backgrounds, borders, border-radius (backgrounds and clip).
//! - Linear and radial gradients; conic via a wedge-path approximation.
//! - Box-shadow.
//! - Text: glyph outlines, decorations, text-shadow, `-webkit-text-stroke`.
//! - Bitmap and emoji glyphs, images.
//! - Clip-path, overflow, opacity.
//! - Filter and backdrop-filter (`<filter>` chains; the backdrop is the scene
//!   replayed up to the element).
//! - Affine transforms.

mod box_model;
mod gradient;
mod image;
mod render;
mod scene_emit;
mod text;

use std::{borrow::Cow, collections::HashMap, fmt, fmt::Write as _, io, mem};

use box_model::{quantize_path, rect_path_data};
use quick_xml::{
  Writer,
  events::{BytesEnd, BytesStart, BytesText, Event},
};
pub use render::{SvgOptions, render};
use takumi_core::{
  geometry::Size,
  painter::StrokeStyle,
  shadow::SizedShadow,
  style::{Affine, Color, Filter, FilterReference, LUMA_WEIGHTS, SEPIA_WEIGHTS, SizingContext},
};
use tiny_skia::PremultipliedColorU8;

/// Straight-alpha RGBA color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rgba(pub [u8; 4]);

impl Rgba {
  /// Unpremultiplies a tiny-skia pixel.
  pub(crate) fn demultiplied(color: PremultipliedColorU8) -> Self {
    let color = color.demultiply();

    Self([color.red(), color.green(), color.blue(), color.alpha()])
  }

  /// `#rgb` or `#rrggbb` hex; alpha goes in a separate `*-opacity`.
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

/// An axis-aligned rectangle in absolute SVG user space.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Frame {
  pub x: f32,
  pub y: f32,
  pub w: f32,
  pub h: f32,
}

impl Frame {
  pub(crate) fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
    Self { x, y, w, h }
  }

  /// Path `d` data tracing the rectangle.
  pub(crate) fn path_data(self) -> String {
    rect_path_data(self.x, self.y, self.x + self.w, self.y + self.h)
  }
}

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
/// resolves `url(#id)` references regardless of document order.
pub(crate) struct SvgDocument {
  writer: Writer<Vec<u8>>,
  next_id: u32,
  /// Interned glyph outlines in glyph space, emitted as `<defs>` by [`Self::render`].
  glyph_defs: Vec<String>,
  glyph_ids: HashMap<String, u32>,
}

impl SvgDocument {
  /// Creates a document with the given pixel viewport and writes the root
  /// `<svg>` open tag.
  pub(crate) fn new(width: f32, height: f32) -> io::Result<Self> {
    // Indent so the emitted SVG is one element per line and reviewable in a diff.
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
    let (width, height) = (num(width), num(height));
    let mut svg = BytesStart::new("svg");
    svg.push_attribute(("xmlns", "http://www.w3.org/2000/svg"));
    svg.push_attribute(("width", width.as_str()));
    svg.push_attribute(("height", height.as_str()));
    svg.push_attribute(("viewBox", format!("0 0 {width} {height}").as_str()));
    writer.write_event(Event::Start(svg))?;
    Ok(Self {
      writer,
      next_id: 0,
      glyph_defs: Vec::new(),
      glyph_ids: HashMap::new(),
    })
  }

  /// Allocates a document-unique id and its `url(#id)` reference.
  fn alloc_id(&mut self, prefix: &str) -> (String, String) {
    let id = format!("{prefix}{}", self.next_id);
    let reference = format!("url(#{id})");

    self.next_id += 1;
    (id, reference)
  }

  fn empty(&mut self, name: &str, attrs: &[(&str, Cow<'_, str>)]) -> io::Result<()> {
    self.writer.write_event(Event::Empty(element(name, attrs)))
  }

  fn open(&mut self, name: &str, attrs: &[(&str, Cow<'_, str>)]) -> io::Result<()> {
    self.writer.write_event(Event::Start(element(name, attrs)))
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

  /// Appends a filled path; `even_odd` picks the fill rule ring shapes need.
  pub(crate) fn path(&mut self, data: &str, fill: Rgba, even_odd: bool) -> io::Result<()> {
    let mut attrs: Vec<(&str, Cow<'_, str>)> =
      vec![("d", data.into()), ("fill", fill.hex().into())];

    push_opacity(&mut attrs, "fill-opacity", fill.opacity());
    if even_odd {
      attrs.push(("fill-rule", "evenodd".into()));
    }
    self.empty("path", &attrs)
  }

  /// Defines a linear gradient and returns its `url(#id)` reference.
  pub(crate) fn linear_gradient(
    &mut self,
    (x1, y1): (f32, f32),
    (x2, y2): (f32, f32),
    repeating: bool,
    stops: &[GradientStop],
  ) -> io::Result<String> {
    let geometry = vec![
      ("x1", num(x1).into()),
      ("y1", num(y1).into()),
      ("x2", num(x2).into()),
      ("y2", num(y2).into()),
    ];

    self.gradient("linearGradient", "lg", repeating, geometry, stops)
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
    let mut geometry = vec![
      ("cx", num(cx).into()),
      ("cy", num(cy).into()),
      ("r", num(r).into()),
    ];
    let (sx, sy) = scale;

    if (sx - 1.0).abs() > f32::EPSILON || (sy - 1.0).abs() > f32::EPSILON {
      let [e, f] = [cx - sx * cx, cy - sy * cy].map(Num);
      let [sx, sy] = [sx, sy].map(Num);

      geometry.push((
        "gradientTransform",
        format!("matrix({sx} 0 0 {sy} {e} {f})").into(),
      ));
    }

    self.gradient("radialGradient", "rg", repeating, geometry, stops)
  }

  /// Defines a gradient element; `repeating` tiles the stops beyond their range.
  fn gradient(
    &mut self,
    name: &str,
    id_prefix: &str,
    repeating: bool,
    geometry: Vec<(&str, Cow<'_, str>)>,
    stops: &[GradientStop],
  ) -> io::Result<String> {
    let (id, reference) = self.alloc_id(id_prefix);
    let mut attrs = vec![
      ("id", id.into()),
      ("gradientUnits", "userSpaceOnUse".into()),
    ];

    if repeating {
      attrs.push(("spreadMethod", "repeat".into()));
    }
    attrs.extend(geometry);
    self.open(name, &attrs)?;
    self.write_stops(stops)?;
    self.close(name)?;
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
  /// `even_odd` picks the clip rule ring shapes need.
  pub(crate) fn clip_path(
    &mut self,
    data: &str,
    even_odd: bool,
    transform: Option<&str>,
  ) -> io::Result<String> {
    let (id, reference) = self.alloc_id("cp");
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
    let (id, reference) = self.alloc_id("cp");
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

  /// Opens a `<g>` clipped to the path data.
  pub(crate) fn begin_clipped_group(&mut self, data: &str) -> io::Result<GroupToken> {
    let clip = self.clip_path(data, false, None)?;

    self.begin_group(Affine::IDENTITY, 1.0, Some(&clip), None)
  }

  /// Opens an alpha `<mask>` in user space and returns the open token plus its
  /// `url(#id)`; CSS `mask-image` defaults to alpha masking.
  pub(crate) fn begin_mask(&mut self) -> io::Result<(GroupToken, String)> {
    let (id, reference) = self.alloc_id("mk");
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

  /// Opens a user-space `<pattern>` tile and returns the open token plus its
  /// `url(#id)`.
  pub(crate) fn begin_pattern(
    &mut self,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
  ) -> io::Result<(GroupToken, String)> {
    let (id, reference) = self.alloc_id("pat");
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

  /// Appends a raster image referenced by a `data:` URL href.
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

  /// Opens a `<g>` with a transform and optional opacity, clip, and filter.
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

  /// Closes the most recently opened group.
  pub(crate) fn end_group(&mut self, _token: GroupToken) -> io::Result<()> {
    self.close("g")
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

  /// Appends a glyph path with an optional `(color, width, line-join)` stroke.
  pub(crate) fn glyph_path(
    &mut self,
    data: &str,
    fill: Rgba,
    stroke: Option<(Rgba, f32, &str)>,
  ) -> io::Result<()> {
    let mut attrs: Vec<(&str, Cow<'_, str>)> = vec![("d", data.into())];

    attrs.extend(glyph_paint_attrs(fill, stroke));
    self.empty("path", &attrs)
  }

  /// Interns a glyph outline (path data in glyph space, translation excluded)
  /// and returns the id shared by every `<use>` of the same outline.
  pub(crate) fn glyph_ref(&mut self, data: String) -> u32 {
    if let Some(&id) = self.glyph_ids.get(&data) {
      return id;
    }
    let id = self.glyph_defs.len() as u32;

    self.glyph_ids.insert(data.clone(), id);
    self.glyph_defs.push(data);
    id
  }

  /// Emits and clears a run of interned glyphs as `<use>` references. Fill and
  /// stroke go on a shared `<g>`, or directly on a lone `<use>`.
  pub(crate) fn flush_glyph_uses(
    &mut self,
    uses: &mut Vec<(u32, f32, f32)>,
    fill: Rgba,
    stroke: Option<(Rgba, f32, &str)>,
  ) -> io::Result<()> {
    if uses.is_empty() {
      return Ok(());
    }

    let paint = glyph_paint_attrs(fill, stroke);

    if let &[(id, x, y)] = uses.as_slice() {
      let mut attrs = glyph_use_attrs(id, x, y);

      attrs.extend(paint);
      self.empty("use", &attrs)?;
    } else {
      self.open("g", &paint)?;
      for &(id, x, y) in uses.iter() {
        self.empty("use", &glyph_use_attrs(id, x, y))?;
      }
      self.close("g")?;
    }

    uses.clear();
    Ok(())
  }

  /// Defines a gaussian-blur filter (for text-shadow) and returns its `url(#id)`.
  pub(crate) fn blur_filter(&mut self, std_deviation: f32) -> io::Result<String> {
    let (id, reference) = self.alloc_id("bl");
    self.open("filter", &[("id", id.into())])?;
    self.empty(
      "feGaussianBlur",
      &[("stdDeviation", num(std_deviation).into())],
    )?;
    self.close("filter")?;
    Ok(reference)
  }

  /// Defines a CSS `filter` list as SVG `<filter>` elements and returns their
  /// `url(#id)` references, to be applied innermost first (index 0 closest to
  /// the element). Runs of filter functions become one generated chain filter;
  /// each `url()` reference becomes its own `<filter>`, emitted verbatim, since
  /// its own region and `color-interpolation-filters` must keep applying.
  ///
  /// With `restore_opaque_alpha` the last filter snaps alpha back to opaque. A
  /// backdrop blur feathers the replay's alpha at the canvas edge (outside the
  /// input is transparent), letting the sharp original bleed through; browsers
  /// sample the backdrop with edge duplication instead, so no feathering exists
  /// to begin with.
  pub(crate) fn filter(
    &mut self,
    filters: &[Filter],
    sizing: &SizingContext,
    current_color: Color,
    size: Size<f32>,
    restore_opaque_alpha: bool,
  ) -> io::Result<Vec<String>> {
    let mut references = Vec::new();
    let mut pending: Vec<&Filter> = Vec::new();

    for filter in filters {
      match filter {
        Filter::Reference(reference) => {
          if !pending.is_empty() {
            references.push(self.function_chain_filter(
              &pending,
              sizing,
              current_color,
              size,
              false,
            )?);
            pending.clear();
          }
          references.push(self.reference_filter(reference)?);
        }
        other => pending.push(other),
      }
    }

    if !pending.is_empty() {
      references.push(self.function_chain_filter(
        &pending,
        sizing,
        current_color,
        size,
        restore_opaque_alpha,
      )?);
    } else if restore_opaque_alpha && !references.is_empty() {
      references.push(self.function_chain_filter(&[], sizing, current_color, size, true)?);
    }

    Ok(references)
  }

  /// Opens a group per filter reference after the first, outermost last: later
  /// filters in the list apply after earlier ones, so they wrap outside.
  pub(crate) fn begin_filter_wrappers(
    &mut self,
    references: &[String],
  ) -> io::Result<Vec<GroupToken>> {
    references
      .iter()
      .skip(1)
      .rev()
      .map(|reference| self.begin_group(Affine::IDENTITY, 1.0, None, Some(reference)))
      .collect()
  }

  /// Emits a referenced `<filter>` verbatim under a fresh document-unique id.
  fn reference_filter(&mut self, reference: &FilterReference) -> io::Result<String> {
    let (id, filter_reference) = self.alloc_id("fr");
    // The parser strips any author id and injects the canonical one, so this
    // textual rewrite always hits.
    let markup = reference.markup.replacen(
      &format!(r#"id="{}""#, FilterReference::ID),
      &format!(r#"id="{id}""#),
      1,
    );
    self
      .writer
      .write_event(Event::Text(BytesText::from_escaped(markup)))?;
    Ok(filter_reference)
  }

  /// Defines a run of CSS filter functions as one chained `<filter>`.
  /// Primitives are chained with `result="fN"`/`in="f(N-1)"`; the region is
  /// widened so blur/shadow are not clipped. `size` is the element's border-box
  /// size, the resolution basis for `drop-shadow` lengths (mirroring the raster
  /// backend).
  fn function_chain_filter(
    &mut self,
    filters: &[&Filter],
    sizing: &SizingContext,
    current_color: Color,
    size: Size<f32>,
    restore_opaque_alpha: bool,
  ) -> io::Result<String> {
    let (id, reference) = self.alloc_id("ft");
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
    for (index, filter) in filters.iter().copied().enumerate() {
      let result = format!("f{index}");
      self.filter_primitive(filter, &prev, &result, sizing, current_color, size)?;
      prev = result.into();
    }
    if restore_opaque_alpha {
      self.open("feComponentTransfer", &[("in", prev)])?;
      self.empty(
        "feFuncA",
        &[("type", "discrete".into()), ("tableValues", "1".into())],
      )?;
      self.close("feComponentTransfer")?;
    }
    self.close("filter")?;
    Ok(reference)
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
      Filter::Grayscale(amount) => self.color_matrix(
        input,
        result,
        &projection_matrix(amount.0.clamp(0.0, 1.0), [LUMA_WEIGHTS; 3]),
      ),
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
      Filter::Sepia(amount) => self.color_matrix(
        input,
        result,
        &projection_matrix(amount.0.clamp(0.0, 1.0), SEPIA_WEIGHTS),
      ),
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
      _ => Ok(()),
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

  /// Strokes a path, dashed when the stroke carries a dash pattern.
  pub(crate) fn stroke_path(&mut self, data: &str, stroke: &StrokeStyle) -> io::Result<()> {
    let color = Rgba(stroke.color.0);
    let mut attrs: Vec<(&str, Cow<'_, str>)> = vec![
      ("d", data.into()),
      ("fill", "none".into()),
      ("stroke", color.hex().into()),
    ];
    push_opacity(&mut attrs, "stroke-opacity", color.opacity());
    attrs.push(("stroke-width", num(stroke.width).into()));
    if let Some(dash) = stroke.dash {
      let [dash, gap] = dash.map(Num);

      attrs.push(("stroke-dasharray", format!("{dash} {gap}").into()));
    }
    if stroke.round_cap {
      attrs.push(("stroke-linecap", "round".into()));
    }
    self.empty("path", &attrs)
  }

  /// Closes the root `<svg>` and serializes the document to a string. Interned
  /// glyph outlines are flushed as a trailing `<defs>`; `<use>` references
  /// resolve document-wide, so forward references are fine.
  pub(crate) fn render(mut self) -> io::Result<String> {
    if !self.glyph_defs.is_empty() {
      self.open("defs", &[])?;
      for (id, data) in mem::take(&mut self.glyph_defs).iter().enumerate() {
        self.empty(
          "path",
          &[("id", format!("g{id}").into()), ("d", data.as_str().into())],
        )?;
      }
      self.close("defs")?;
    }
    self.close("svg")?;
    Ok(String::from_utf8_lossy(&self.writer.into_inner()).into_owned())
  }
}

/// Opaque proof that a `<g>` is open; consumed by [`SvgDocument::end_group`].
#[must_use]
pub(crate) struct GroupToken(());

fn matrix_attr(transform: Affine) -> String {
  let [a, b, c, d, e, f] = transform.to_cols_array().map(Num);

  format!("matrix({a} {b} {c} {d} {e} {f})")
}

/// CSS `grayscale()`/`sepia()` color matrix (spec form: identity lerped toward
/// the `weights` projection by `amount`), matching the raster backend.
fn projection_matrix(amount: f32, weights: [[f32; 3]; 3]) -> [f32; 20] {
  let mut matrix = [0.0; 20];

  for (row, row_weights) in weights.into_iter().enumerate() {
    for (column, weight) in row_weights.into_iter().enumerate() {
      matrix[row * 5 + column] = if row == column {
        1.0 - amount + amount * weight
      } else {
        amount * weight
      };
    }
  }
  matrix[18] = 1.0;
  matrix
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
    let Ok(text) = str::from_utf8(&buf.bytes[..buf.len]) else {
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

fn element<'a>(name: &'a str, attrs: &[(&str, Cow<'_, str>)]) -> BytesStart<'a> {
  let mut element = BytesStart::new(name);

  for (key, value) in attrs {
    element.push_attribute((*key, value.as_ref()));
  }
  element
}

/// Fill plus optional `-webkit-text-stroke` attributes, shared by glyph
/// `<path>` elements and `<use>` runs.
fn glyph_paint_attrs<'a>(
  fill: Rgba,
  stroke: Option<(Rgba, f32, &'a str)>,
) -> Vec<(&'a str, Cow<'a, str>)> {
  let mut attrs: Vec<(&str, Cow<'_, str>)> = vec![("fill", fill.hex().into())];

  push_opacity(&mut attrs, "fill-opacity", fill.opacity());
  if let Some((color, width, line_join)) = stroke {
    attrs.push(("stroke", color.hex().into()));
    push_opacity(&mut attrs, "stroke-opacity", color.opacity());
    attrs.push(("stroke-width", num(width).into()));
    if line_join != "miter" {
      attrs.push(("stroke-linejoin", line_join.into()));
    }
  }
  attrs
}

/// `<use>` attributes referencing an interned glyph outline. The position is
/// quantized like the path data it replaces.
fn glyph_use_attrs(id: u32, x: f32, y: f32) -> Vec<(&'static str, Cow<'static, str>)> {
  vec![
    ("href", format!("#g{id}").into()),
    ("x", num(quantize_path(x)).into()),
    ("y", num(quantize_path(y)).into()),
  ]
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
    doc.path("M0 0 H10 V10 H0 Z", RED, false).unwrap();
    let svg = doc.render().unwrap();
    assert!(svg.contains(r#"<linearGradient id="lg0""#));
    assert!(svg.contains(r#"<stop offset="0""#));
  }

  #[test]
  fn clip_path_and_group_nest() {
    let mut doc = SvgDocument::new(10.0, 10.0).unwrap();
    let clip = doc.clip_path("M0 0 H5 V5 H0 Z", false, None).unwrap();
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
    let token = doc.begin_group(Affine::IDENTITY, 0.5, None, None).unwrap();
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
      .path("M1 9 L2 1 L3 9 M1.5 5 H2.5", Rgba([0, 0, 0, 255]), false)
      .unwrap();
    assert!(
      doc
        .render()
        .unwrap()
        .contains("<path d=\"M1 9 L2 1 L3 9 M1.5 5 H2.5\"")
    );
  }
}
