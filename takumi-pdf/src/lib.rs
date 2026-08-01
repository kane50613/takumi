#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Vector PDF output for takumi.
//!
//! [`render`] runs takumi-core layout, walks the same backend-agnostic
//! stacking-context scene as `takumi-svg`, and emits a PDF through [`krilla`]:
//! background rects as filled paths and text as real glyph runs with embedded,
//! subsetted fonts — selectable, searchable, copyable.
//!
//! With [`PdfOptions::page`] set, content lays out at the page's content width
//! with unbounded height and is sliced into pages: unsplittable atoms (text
//! lines, images, transformed subtrees) are collected, cut points move up to
//! avoid splitting them, and each page re-walks the scene through a vertical
//! window (clip + translate). Every text line is emitted on exactly one page.
//!
//! Coverage so far: background-color, text runs (fill color and opacity),
//! nested containers, affine transforms, pagination. Not yet: borders, radius,
//! gradients, images, shadows, filters, clipping, text decorations,
//! break-* properties, page headers/footers.

use std::{collections::HashMap, ops::Range, rc::Rc, sync::Arc};

use krilla::{
  Data, Document,
  color::rgb,
  error::KrillaError,
  geom::{PathBuilder, Point, Rect as KrillaRect, Size as KrillaSize, Transform},
  num::NormalizedF32,
  page::PageSettings,
  paint::{Fill, FillRule},
  surface::Surface,
  text::{Font, Glyph, GlyphId},
};
use takumi_core::{
  Fonts,
  context::RenderContext,
  error::Error as TakumiError,
  font_style::SizedFontStyle,
  geometry::{AvailableSpace, ComputedLayout as Layout, NodeId, Size},
  layout::{
    inline::{
      BuiltInlineLayout, InlineItem, InlineLayoutMode, InlineLayoutRequest, InlineRunLayout,
      collect_inline_items, create_inline_layout, resolve_inline_max_height, resolve_inline_runs,
    },
    node::{Node, NodeKind, TextData},
    tree::{LayoutResults, LayoutTree, RenderNode},
  },
  resources::font::FontError,
  scene::{NodePaint, PaintItemKind, StackingContextNode, build_stacking_contexts},
  style::{Affine, ComputedStyle, FontFamily, Lang, SizingContext, StyleSheet},
  viewport::Viewport,
};
use typed_builder::TypedBuilder;

/// Errors from [`render`].
#[derive(Debug)]
pub enum PdfError {
  /// Layout or resource resolution failed in takumi-core.
  Render(TakumiError),
  /// Font data could not be interpreted.
  Font(FontError),
  /// PDF serialization failed.
  Krilla(KrillaError),
  /// The computed page size is empty or non-finite.
  InvalidPageSize,
}

impl From<TakumiError> for PdfError {
  fn from(error: TakumiError) -> Self {
    Self::Render(error)
  }
}

/// Inputs for [`render`], built with [`PdfOptions::builder`].
#[derive(TypedBuilder)]
pub struct PdfOptions<'g> {
  /// The viewport to render in.
  pub(crate) viewport: Viewport,
  /// The font context.
  pub(crate) fonts: &'g Fonts,
  /// The root node to render.
  pub(crate) node: Node,
  /// CSS stylesheets to apply before layout.
  #[builder(default)]
  pub(crate) stylesheet: Arc<StyleSheet>,
  /// Paged output; `None` renders a single page at the viewport size.
  #[builder(default, setter(strip_option))]
  pub(crate) page: Option<PageOptions>,
  /// Per-render font fallback chain (family names in order).
  #[builder(default)]
  pub(crate) font_families: Option<FontFamily>,
  /// Default BCP-47 language tag applied to the root.
  #[builder(default)]
  pub(crate) lang: Option<Lang>,
}

/// Paged output geometry: fixed page size with a uniform margin. Content lays
/// out at `width - 2 * margin` and flows across as many pages as it needs.
#[derive(Clone, Copy)]
pub struct PageOptions {
  /// Page width in px (A4 at 96 dpi ≈ 794).
  pub width: f32,
  /// Page height in px (A4 at 96 dpi ≈ 1123).
  pub height: f32,
  // ponytail: uniform margin; per-side margins when someone asks.
  /// Margin applied to all four sides, in px.
  pub margin: f32,
}

/// Millimeters to CSS px (96 dpi).
fn mm(value: f32) -> f32 {
  value / 25.4 * 96.0
}

/// Inches to CSS px (96 dpi).
fn inches(value: f32) -> f32 {
  value * 96.0
}

/// Every preset is portrait with a half-inch margin; chain
/// [`landscape`](Self::landscape) and [`with_margin`](Self::with_margin) to
/// adjust. The set mirrors the CSS `@page` size keywords.
impl PageOptions {
  const DEFAULT_MARGIN: f32 = 48.0;

  fn preset(width: f32, height: f32) -> Self {
    Self {
      width,
      height,
      margin: Self::DEFAULT_MARGIN,
    }
  }

  /// ISO A3: 297 × 420 mm.
  pub fn a3() -> Self {
    Self::preset(mm(297.0), mm(420.0))
  }

  /// ISO A4: 210 × 297 mm.
  pub fn a4() -> Self {
    Self::preset(mm(210.0), mm(297.0))
  }

  /// ISO A5: 148 × 210 mm.
  pub fn a5() -> Self {
    Self::preset(mm(148.0), mm(210.0))
  }

  /// ISO B4: 250 × 353 mm.
  pub fn b4() -> Self {
    Self::preset(mm(250.0), mm(353.0))
  }

  /// ISO B5: 176 × 250 mm.
  pub fn b5() -> Self {
    Self::preset(mm(176.0), mm(250.0))
  }

  /// JIS B4: 257 × 364 mm.
  pub fn jis_b4() -> Self {
    Self::preset(mm(257.0), mm(364.0))
  }

  /// JIS B5: 182 × 257 mm.
  pub fn jis_b5() -> Self {
    Self::preset(mm(182.0), mm(257.0))
  }

  /// US Letter: 8.5 × 11 in.
  pub fn letter() -> Self {
    Self::preset(inches(8.5), inches(11.0))
  }

  /// US Legal: 8.5 × 14 in.
  pub fn legal() -> Self {
    Self::preset(inches(8.5), inches(14.0))
  }

  /// US Ledger/Tabloid: 11 × 17 in.
  pub fn ledger() -> Self {
    Self::preset(inches(11.0), inches(17.0))
  }

  /// Swaps width and height.
  pub fn landscape(self) -> Self {
    Self {
      width: self.height,
      height: self.width,
      ..self
    }
  }

  /// Replaces the uniform margin.
  pub fn with_margin(self, margin: f32) -> Self {
    Self { margin, ..self }
  }

  fn content_size(&self) -> (f32, f32) {
    (
      self.width - 2.0 * self.margin,
      self.height - 2.0 * self.margin,
    )
  }
}

/// Renders a node tree to a PDF: single-page at the viewport size, or paged
/// when [`PdfOptions::page`] is set.
pub fn render(options: PdfOptions<'_>) -> Result<Vec<u8>, PdfError> {
  let viewport = match options.page {
    Some(page) => {
      let (content_width, _) = page.content_size();
      if content_width <= 0.0 {
        return Err(PdfError::InvalidPageSize);
      }
      Viewport::new((content_width as u32, None))
    }
    None => options.viewport,
  };

  let context = RenderContext::builder()
    .fonts(
      options
        .fonts
        .snapshot_with_fallbacks(options.font_families.as_ref()),
    )
    .sizing(SizingContext::builder().viewport(viewport).build())
    .images(Rc::new(HashMap::new()))
    .stylesheet(options.stylesheet)
    .style(Box::new(ComputedStyle {
      lang: options.lang,
      font_family: options.font_families.unwrap_or_default(),
      ..Default::default()
    }))
    .build();

  let root = RenderNode::from_node(&context, options.node);
  let mut tree = LayoutTree::from_render_node(&root);

  tree.compute_layout(viewport.into());

  let results = tree.into_results();
  let root_layout = results.layout(NodeId::ROOT)?;
  let width = viewport
    .size
    .width
    .map_or(root_layout.size.width, |w| w as f32);
  let height = viewport
    .size
    .height
    .map_or(root_layout.size.height, |h| h as f32);

  let contexts = build_stacking_contexts(
    &root,
    &results,
    NodeId::ROOT,
    Affine::IDENTITY,
    (Some(width), Some(height)),
  )?;

  let mut document = Document::new();
  let mut emitter = Emitter {
    root: &root,
    contexts: &contexts,
    results: &results,
    fonts: HashMap::new(),
    window: None,
    line_window: None,
  };

  match options.page {
    Some(page) => {
      let page_size =
        KrillaSize::from_wh(page.width, page.height).ok_or(PdfError::InvalidPageSize)?;
      let (content_width, content_height) = page.content_size();
      if content_height <= 0.0 {
        return Err(PdfError::InvalidPageSize);
      }

      let mut atoms = Vec::new();

      emitter.collect_atoms(0, Affine::IDENTITY, &mut atoms)?;
      let starts = page_starts(&mut atoms, height, content_height);

      for (index, &y0) in starts.iter().enumerate() {
        let line_start = if index == 0 { f32::NEG_INFINITY } else { y0 };
        let line_end = starts.get(index + 1).copied().unwrap_or(f32::INFINITY);

        emitter.window = Some((y0, y0 + content_height));
        emitter.line_window = Some((line_start, line_end));
        let mut pdf_page = document.start_page_with(PageSettings::new(page_size));
        let mut surface = pdf_page.surface();

        if let Some(window) =
          KrillaRect::from_xywh(page.margin, page.margin, content_width, content_height)
        {
          let mut builder = PathBuilder::new();

          builder.push_rect(window);
          if let Some(path) = builder.finish() {
            surface.push_clip_path(&path, &FillRule::NonZero);
            surface.push_transform(&Transform::from_translate(page.margin, page.margin - y0));
            emitter.emit_context(0, Affine::IDENTITY, &mut surface)?;
            surface.pop();
            surface.pop();
          }
        }

        surface.finish();
        pdf_page.finish();
      }
    }
    None => {
      let page_size = KrillaSize::from_wh(width, height).ok_or(PdfError::InvalidPageSize)?;
      let mut page = document.start_page_with(PageSettings::new(page_size));
      let mut surface = page.surface();

      emitter.emit_context(0, Affine::IDENTITY, &mut surface)?;
      surface.finish();
      page.finish();
    }
  }

  document.finish().map_err(PdfError::Krilla)
}

/// Unsplittable vertical extents in content coordinates: text lines, images,
/// and transformed subtrees (which cannot be windowed without distortion).
type Atom = (f32, f32);

/// Page start offsets for slicing `total` height into windows of `window`
/// height. Each cut moves up to the top of any atom straddling the ideal cut
/// line, so atoms land whole on the next page; an atom taller than the window
/// is cut anyway.
fn page_starts(atoms: &mut [Atom], total: f32, window: f32) -> Vec<f32> {
  atoms.sort_by(|a, b| a.0.total_cmp(&b.0));

  let mut starts = vec![0.0_f32];
  let mut y0 = 0.0_f32;

  while y0 + window < total {
    let ideal = y0 + window;
    let pushed_up = atoms
      .iter()
      .filter(|(top, bottom)| *top < ideal && *bottom > ideal)
      .map(|(top, _)| *top)
      .fold(ideal, f32::min);
    let cut = if pushed_up > y0 + 1.0 {
      pushed_up
    } else {
      ideal
    };

    starts.push(cut);
    y0 = cut;
  }
  starts
}

/// Scene walker state: the render tree, the stacking-context scene, and a cache
/// of krilla fonts keyed by the backing blob identity.
struct Emitter<'a> {
  root: &'a RenderNode,
  contexts: &'a [StackingContextNode],
  results: &'a LayoutResults,
  fonts: HashMap<(usize, usize, u32), Font>,
  /// Vertical content window `[top, bottom)` of the page being emitted;
  /// paint wholly outside it is skipped so clipped-away content never reaches
  /// the content stream (or text extraction).
  window: Option<(f32, f32)>,
  /// Text-line ownership window: `[this page's cut, next page's cut)`. Wider
  /// than `window` at the edges (first page reaches up to −∞, last to +∞) and
  /// narrower at the bottom when a cut lands above the page's full height, so
  /// every line is emitted on exactly one page.
  line_window: Option<(f32, f32)>,
}

impl Emitter<'_> {
  fn window_excludes(&self, top: f32, bottom: f32) -> bool {
    self
      .window
      .is_some_and(|(y0, y1)| bottom <= y0 || top >= y1)
  }

  /// Whether a text line whose band starts at `top` belongs to another page.
  /// Lines use a half-open ownership rule (the page containing their top) so
  /// overlapping ascent/descent bands of adjacent lines never emit twice.
  fn window_disowns_line(&self, top: f32) -> bool {
    self
      .line_window
      .is_some_and(|(y0, y1)| top < y0 || top >= y1)
  }

  fn window_excludes_bounds(&self, bounds: Option<takumi_core::scene::SceneBounds>) -> bool {
    bounds.is_some_and(|b| self.window_excludes(b.top as f32, b.bottom as f32))
  }
}

impl Emitter<'_> {
  fn emit_context(
    &mut self,
    id: usize,
    parent: Affine,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    let Some(context) = self.contexts.get(id) else {
      return Ok(());
    };

    let child_frame = match context.root() {
      Some(paint) => {
        let (frame, pushed) = self.emit_box(paint, parent, surface)?;
        pop_transforms(surface, pushed);
        frame
      }
      None => parent,
    };

    for bucket in context.in_paint_order() {
      for item in bucket {
        match &item.kind {
          PaintItemKind::Node(paint) => {
            let (_, pushed) = self.emit_box(paint, child_frame, surface)?;
            pop_transforms(surface, pushed);
          }
          PaintItemKind::Context(child) => {
            let excluded = self
              .contexts
              .get(*child)
              .is_some_and(|ctx| self.window_excludes_bounds(ctx.paint_bounds()));
            if !excluded {
              self.emit_context(*child, child_frame, surface)?;
            }
          }
        }
      }
    }
    Ok(())
  }

  /// Emits one node's background and own content. Returns the frame the node's
  /// children sit in and how many transforms were pushed onto the surface.
  fn emit_box(
    &mut self,
    paint: &NodePaint,
    parent: Affine,
    surface: &mut Surface,
  ) -> Result<(Affine, usize), PdfError> {
    let Some(node) = self.root.node_at_path(&paint.path) else {
      return Ok((parent, 0));
    };
    let Ok(layout) = self.results.layout(paint.node_id) else {
      return Ok((parent, 0));
    };
    if self.window_excludes_bounds(paint.paint_bounds) {
      return Ok((parent, 0));
    }

    let relative = parent.invert().unwrap_or(Affine::IDENTITY) * paint.transform;
    let (x, y, pushed) = if relative.only_translation() {
      (relative.x, relative.y, 0)
    } else {
      let cols = relative.to_cols_array();
      surface.push_transform(&Transform::from_row(
        cols[0], cols[1], cols[2], cols[3], cols[4], cols[5],
      ));
      (0.0, 0.0, 1)
    };
    let frame = if pushed == 0 {
      parent
    } else {
      parent * relative
    };

    self.emit_background(node, layout, x, y, surface);
    self.emit_own_content(node, layout, x, y, surface)?;
    Ok((frame, pushed))
  }

  fn emit_background(
    &self,
    node: &RenderNode,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) {
    let color = node
      .context
      .style
      .background_color
      .resolve(node.context.current_color);
    if color.0[3] == 0 {
      return;
    }
    let Some(rect) = KrillaRect::from_xywh(x, y, layout.size.width, layout.size.height) else {
      return;
    };
    let mut builder = PathBuilder::new();

    builder.push_rect(rect);
    let Some(path) = builder.finish() else {
      return;
    };

    surface.set_fill(Some(fill_from_rgba(color.0, 1.0)));
    surface.draw_path(&path);
  }

  fn emit_own_content(
    &mut self,
    node: &RenderNode,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    if node.should_create_inline_layout() {
      return self.emit_inline_content(node, layout, x, y, surface);
    }
    if node.has_anonymous_text_item_child() {
      return Ok(());
    }
    match node.node.as_ref().map(|n| &n.kind) {
      Some(NodeKind::Text(text)) => self.emit_text(text, &node.context, layout, x, y, surface),
      _ => Ok(()),
    }
  }

  fn emit_text(
    &mut self,
    text: &TextData,
    context: &RenderContext,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    let items = vec![InlineItem::Text {
      text: text.text.as_str().into(),
      context,
    }];
    let font_style = SizedFontStyle::from_style(&context.style, context);
    let Some((built, runs)) = build_inline_runs(items, &font_style, context, layout)? else {
      return Ok(());
    };

    self.draw_runs(&runs, &built, layout, x, y, surface)
  }

  fn emit_inline_content(
    &mut self,
    node: &RenderNode,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    let context = &node.context;
    let font_style = SizedFontStyle::from_style(&context.style, context);
    let Some((built, runs)) =
      build_inline_runs(collect_inline_items(node), &font_style, context, layout)?
    else {
      return Ok(());
    };

    self.draw_runs(&runs, &built, layout, x, y, surface)
  }

  fn draw_runs(
    &mut self,
    runs: &InlineRunLayout,
    built: &BuiltInlineLayout<'_>,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    for run in &runs.runs {
      let shaped = &run.glyph_run;
      if shaped.glyphs.is_empty() {
        continue;
      }
      let Some(font) = self.cached_font(shaped.font_data(), shaped.font_index) else {
        continue;
      };
      let offset = run.glyph_offset(layout);
      if let Some(glyph) = shaped.glyphs.first() {
        let baseline = y + offset.y + glyph.y;
        if self.window_disowns_line(baseline - shaped.metrics.ascent) {
          continue;
        }
      }
      let run_text = built
        .text
        .get(shaped.text_range.clone())
        .unwrap_or_default();
      let spans = glyph_text_spans(run_text, shaped.glyphs.len());

      let glyphs: Vec<PdfGlyph> = shaped
        .glyphs
        .iter()
        .zip(spans)
        .map(|(glyph, range)| PdfGlyph {
          id: GlyphId::new(glyph.id),
          x_offset: glyph.x / shaped.font_size,
          y_offset: -glyph.y / shaped.font_size,
          range,
        })
        .collect();

      let color = shaped.brush.color;

      surface.set_fill(Some(fill_from_rgba(color.0, shaped.brush.opacity)));
      surface.draw_glyphs(
        Point::from_xy(x + offset.x, y + offset.y),
        &glyphs,
        font,
        run_text,
        shaped.font_size,
        false,
      );
    }
    Ok(())
  }

  /// A krilla font for a run's backing blob, cached by blob identity. Copies the
  /// blob into the cache once per distinct font.
  fn cached_font(&mut self, data: &[u8], index: u32) -> Option<Font> {
    let key = (data.as_ptr() as usize, data.len(), index);

    if let Some(font) = self.fonts.get(&key) {
      return Some(font.clone());
    }
    let font = Font::new(Data::from(data.to_vec()), index)?;

    self.fonts.insert(key, font.clone());
    Some(font)
  }
}

impl Emitter<'_> {
  /// Mirrors [`Self::emit_context`] but records unsplittable vertical extents
  /// instead of painting.
  fn collect_atoms(
    &mut self,
    id: usize,
    parent: Affine,
    atoms: &mut Vec<Atom>,
  ) -> Result<(), PdfError> {
    let Some(context) = self.contexts.get(id) else {
      return Ok(());
    };

    let child_frame = match context.root() {
      Some(paint) => self.collect_box_atoms(paint, parent, atoms)?,
      None => parent,
    };

    for bucket in context.in_paint_order() {
      for item in bucket {
        match &item.kind {
          PaintItemKind::Node(paint) => {
            self.collect_box_atoms(paint, child_frame, atoms)?;
          }
          PaintItemKind::Context(child) => {
            self.collect_atoms(*child, child_frame, atoms)?;
          }
        }
      }
    }
    Ok(())
  }

  /// Records one node's atoms and returns the frame its children sit in. A
  /// node painted under a non-translation transform becomes a single atom
  /// spanning its device bounds — windowing through a rotation would distort.
  fn collect_box_atoms(
    &mut self,
    paint: &NodePaint,
    parent: Affine,
    atoms: &mut Vec<Atom>,
  ) -> Result<Affine, PdfError> {
    let Some(node) = self.root.node_at_path(&paint.path) else {
      return Ok(parent);
    };
    let Ok(layout) = self.results.layout(paint.node_id) else {
      return Ok(parent);
    };

    let relative = parent.invert().unwrap_or(Affine::IDENTITY) * paint.transform;
    if !relative.only_translation() {
      if let Some(bounds) = paint.paint_bounds {
        atoms.push((bounds.top as f32, bounds.bottom as f32));
      }
      return Ok(parent * relative);
    }
    let y = relative.y;

    if node.should_create_inline_layout() {
      self.collect_text_atoms(collect_inline_items(node), &node.context, layout, y, atoms)?;
    } else if !node.has_anonymous_text_item_child() {
      match node.node.as_ref().map(|n| &n.kind) {
        Some(NodeKind::Text(text)) => {
          let items = vec![InlineItem::Text {
            text: text.text.as_str().into(),
            context: &node.context,
          }];

          self.collect_text_atoms(items, &node.context, layout, y, atoms)?;
        }
        Some(NodeKind::Image(_)) => {
          atoms.push((y, y + layout.size.height));
        }
        _ => {}
      }
    }
    Ok(parent)
  }

  /// One atom per text line: the union of each run's ascent-to-descent band.
  fn collect_text_atoms(
    &mut self,
    items: Vec<InlineItem<'_>>,
    context: &RenderContext,
    layout: Layout,
    y: f32,
    atoms: &mut Vec<Atom>,
  ) -> Result<(), PdfError> {
    let font_style = SizedFontStyle::from_style(&context.style, context);
    let Some((_, runs)) = build_inline_runs(items, &font_style, context, layout)? else {
      return Ok(());
    };

    for run in &runs.runs {
      let shaped = &run.glyph_run;
      let Some(glyph) = shaped.glyphs.first() else {
        continue;
      };
      let offset = run.glyph_offset(layout);
      let baseline = y + offset.y + glyph.y;

      atoms.push((
        baseline - shaped.metrics.ascent,
        baseline + shaped.metrics.descent,
      ));
    }
    Ok(())
  }
}

/// Runs inline layout and resolves the paintable run set. `None` when the font
/// size or content box is degenerate.
fn build_inline_runs<'c>(
  items: Vec<InlineItem<'c>>,
  font_style: &'c SizedFontStyle<'c>,
  context: &'c RenderContext,
  layout: Layout,
) -> Result<Option<(BuiltInlineLayout<'c>, InlineRunLayout)>, PdfError> {
  let content = layout.content_box_size();
  if font_style.sizing.font_size == 0.0 || content.width <= 0.0 || content.height <= 0.0 {
    return Ok(None);
  }

  let built = create_inline_layout(InlineLayoutRequest {
    items,
    available_space: Size {
      width: AvailableSpace::Definite(content.width),
      height: AvailableSpace::Definite(content.height),
    },
    max_width: content.width,
    max_height: resolve_inline_max_height(font_style, content.height),
    style: font_style,
    context,
    mode: InlineLayoutMode::Draw,
  });
  let runs = resolve_inline_runs(&built, context, layout).map_err(PdfError::Font)?;

  Ok(Some((built, runs)))
}

fn pop_transforms(surface: &mut Surface, pushed: usize) {
  for _ in 0..pushed {
    surface.pop();
  }
}

fn fill_from_rgba(rgba: [u8; 4], opacity: f32) -> Fill {
  let alpha = (f32::from(rgba[3]) / 255.0) * opacity;

  Fill {
    paint: rgb::Color::new(rgba[0], rgba[1], rgba[2]).into(),
    opacity: NormalizedF32::new(alpha.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE),
    rule: FillRule::NonZero,
  }
}

/// Per-glyph byte ranges into `run_text` for ToUnicode. One char per glyph when
/// the counts line up (true for 1:1 shaping, which covers Latin without
/// ligatures and CJK); otherwise every glyph maps to the whole run.
// ponytail: cluster-accurate ranges need per-glyph cluster data threaded through
// takumi-core's ShapedRun; add when ligature-heavy scripts matter.
fn glyph_text_spans(run_text: &str, glyph_count: usize) -> Vec<Range<usize>> {
  let char_count = run_text.chars().count();

  if char_count == glyph_count {
    let mut spans = Vec::with_capacity(glyph_count);
    let mut indices = run_text.char_indices().peekable();

    while let Some((start, _)) = indices.next() {
      let end = indices.peek().map_or(run_text.len(), |(next, _)| *next);
      spans.push(start..end);
    }
    spans
  } else {
    vec![0..run_text.len(); glyph_count]
  }
}

/// A positioned glyph adapter. Offsets are stored em-normalized (position ÷ font
/// size): krilla calls the accessors with `size = 1.0` for text-space math and
/// with the real font size for cursor movement, so returning `stored × size`
/// satisfies both. Advances stay zero — glyphs carry absolute offsets instead.
struct PdfGlyph {
  id: GlyphId,
  x_offset: f32,
  y_offset: f32,
  range: Range<usize>,
}

impl Glyph for PdfGlyph {
  fn glyph_id(&self) -> GlyphId {
    self.id
  }

  fn text_range(&self) -> Range<usize> {
    self.range.clone()
  }

  fn x_advance(&self, _size: f32) -> f32 {
    0.0
  }

  fn x_offset(&self, size: f32) -> f32 {
    self.x_offset * size
  }

  fn y_offset(&self, size: f32) -> f32 {
    self.y_offset * size
  }

  fn y_advance(&self, _size: f32) -> f32 {
    0.0
  }

  fn location(&self) -> Option<krilla::surface::Location> {
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn page_starts_without_atoms_cuts_at_window() {
    assert_eq!(page_starts(&mut [], 250.0, 100.0), vec![0.0, 100.0, 200.0]);
  }

  #[test]
  fn page_starts_pushes_straddling_atom_to_next_page() {
    let mut atoms = [(90.0, 110.0)];

    assert_eq!(
      page_starts(&mut atoms, 250.0, 100.0),
      vec![0.0, 90.0, 190.0]
    );
  }

  #[test]
  fn page_starts_hard_cuts_atom_taller_than_window() {
    let mut atoms = [(0.0, 300.0)];

    assert_eq!(
      page_starts(&mut atoms, 300.0, 100.0),
      vec![0.0, 100.0, 200.0]
    );
  }

  #[test]
  fn presets_match_css_page_keywords() {
    let a4 = PageOptions::a4();

    assert!((a4.width - 793.7).abs() < 0.1);
    assert!((a4.height - 1122.5).abs() < 0.1);

    let letter = PageOptions::letter();

    assert_eq!((letter.width, letter.height), (816.0, 1056.0));

    let landscape = PageOptions::a4().landscape();

    assert_eq!(landscape.width, a4.height);
    assert_eq!(landscape.height, a4.width);
    assert_eq!(PageOptions::a4().with_margin(0.0).margin, 0.0);
  }
}
