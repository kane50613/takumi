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
//! Pagination honors `break-before: page`, `break-after: page`, and
//! `break-inside: avoid`; repeated header/footer bands carve their height out
//! of the content window. Nodes classed `pageNumber` / `totalPages` are
//! filled with the page counters, matching Chromium's print templates.
//!
//! Coverage: backgrounds (color and gradient layers), borders and radius,
//! images (`object-fit`/`object-position`), text with decorations, opacity,
//! blend modes, overflow clipping, affine transforms, pagination. Not yet:
//! box-shadow, filters, `clip-path`, masks, `background-size`/`position`/
//! `repeat`, url() background layers, SVG image sources.

use std::{collections::HashMap, ops::Range, rc::Rc, sync::Arc};

#[cfg(feature = "images")]
use krilla::image::Image as KrillaImage;
use krilla::{
  Data, Document,
  blend::BlendMode as KrillaBlendMode,
  color::rgb,
  error::KrillaError,
  geom::{
    Path as KrillaPath, PathBuilder, Point, Rect as KrillaRect, Size as KrillaSize, Transform,
  },
  num::NormalizedF32,
  page::PageSettings,
  paint::{
    Fill, FillRule, LinearGradient as KrillaLinearGradient, Paint,
    RadialGradient as KrillaRadialGradient, SpreadMethod, Stop, SweepGradient,
  },
  surface::Surface,
  text::{Font, Glyph, GlyphId},
};
use takumi_core::{
  Fonts,
  context::RenderContext,
  error::Error as TakumiError,
  font_style::SizedFontStyle,
  geometry::{
    AvailableSpace, ComputedLayout as Layout, NodeId, PathCommand, Point as CorePoint, Size,
  },
  layout::{
    border::{BorderProperties, BorderSide},
    decoration::ClipBox,
    inline::{
      BuiltInlineLayout, DecorationRect, InlineItem, InlineLayoutMode, InlineLayoutRequest,
      InlineRunLayout, ShapedRun, collect_inline_items, create_inline_layout,
      resolve_inline_max_height, resolve_inline_runs, run_decorations,
    },
    node::{Node, NodeKind, TextData},
    tree::{LayoutResults, LayoutTree, RenderNode},
  },
  paint::{ConicGradientTile, LinearGradientTile, RadialGradientTile, resolve_stops_along_axis},
  resources::{font::FontError, image::ImageSource},
  scene::{NodePaint, PaintItemKind, StackingContextNode, build_stacking_contexts},
  style::{
    Affine, BackgroundImage, BlendMode, BoxDecorationBreak, BreakBetween, BreakInside,
    ComputedStyle, FontFamily, Isolation, Lang, Overflow, ResolvedGradientStop, SizingContext,
    StyleSheet,
  },
  viewport::Viewport,
};
#[cfg(feature = "images")]
use takumi_core::{
  layout::node::ImageData,
  resources::image::RenderedImage,
  style::{Length, ObjectFit, PositionComponent},
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
  /// Single-page output ([`PdfOptions::page`] unset) needs a viewport.
  MissingViewport,
}

impl From<TakumiError> for PdfError {
  fn from(error: TakumiError) -> Self {
    Self::Render(error)
  }
}

/// Inputs for [`render`], built with [`PdfOptions::builder`].
#[derive(TypedBuilder)]
pub struct PdfOptions<'g> {
  /// The viewport to render in. Required for single-page output; ignored when
  /// [`Self::page`] is set (the page geometry defines the layout width).
  #[builder(default, setter(strip_option))]
  pub viewport: Option<Viewport>,
  /// The font context.
  pub fonts: &'g Fonts,
  /// The root node to render.
  pub node: Node,
  /// CSS stylesheets to apply before layout.
  #[builder(default)]
  pub stylesheet: Arc<StyleSheet>,
  /// Resources fetched externally, keyed by URL.
  #[builder(default)]
  pub images: HashMap<Arc<str>, ImageSource>,
  /// Paged output; `None` renders a single page at the viewport size.
  #[builder(default, setter(strip_option))]
  pub page: Option<PageOptions>,
  /// Band repeated at the top of every page. Nodes classed `pageNumber` /
  /// `totalPages` receive the counters, optionally formatted by a
  /// supported `@counter-style` name in the same class list (e.g. `cjk-decimal`). The
  /// band's height is carved out of the content window.
  #[builder(default, setter(strip_option))]
  pub header: Option<Node>,
  /// Band repeated at the bottom of every page; same class hooks as `header`.
  #[builder(default, setter(strip_option))]
  pub footer: Option<Node>,
  /// Per-render font fallback chain (family names in order).
  #[builder(default)]
  pub font_families: Option<FontFamily>,
  /// Default BCP-47 language tag applied to the root.
  #[builder(default)]
  pub lang: Option<Lang>,
}

/// Per-side page margins in px.
#[derive(Clone, Copy)]
pub struct PageMargins {
  /// Top margin.
  pub top: f32,
  /// Right margin.
  pub right: f32,
  /// Bottom margin.
  pub bottom: f32,
  /// Left margin.
  pub left: f32,
}

impl PageMargins {
  /// The same margin on all four sides.
  pub const fn uniform(value: f32) -> Self {
    Self {
      top: value,
      right: value,
      bottom: value,
      left: value,
    }
  }
}

/// Paged output geometry: fixed page size with margins. Content lays out at
/// the width inside the margins and flows across as many pages as it needs.
#[derive(Clone, Copy)]
pub struct PageOptions {
  /// Page width in px (A4 at 96 dpi ≈ 794).
  pub width: f32,
  /// Page height in px (A4 at 96 dpi ≈ 1123).
  pub height: f32,
  /// Page margins in px.
  pub margin: PageMargins,
}

/// Millimeters to CSS px (96 dpi).
const fn mm(value: f32) -> f32 {
  value / 25.4 * 96.0
}

/// Inches to CSS px (96 dpi).
const fn inches(value: f32) -> f32 {
  value * 96.0
}

/// Presets are portrait with a half-inch margin; chain
/// [`landscape`](Self::landscape) and [`with_margin`](Self::with_margin) to
/// adjust, or fill the fields directly for any other size.
// ponytail: A4 + LETTER only; add other @page keywords when someone asks.
impl PageOptions {
  const DEFAULT_MARGIN: f32 = 48.0;

  /// ISO A4: 210 × 297 mm.
  pub const A4: Self = Self::preset(mm(210.0), mm(297.0));

  /// US Letter: 8.5 × 11 in.
  pub const LETTER: Self = Self::preset(inches(8.5), inches(11.0));

  const fn preset(width: f32, height: f32) -> Self {
    Self {
      width,
      height,
      margin: PageMargins::uniform(Self::DEFAULT_MARGIN),
    }
  }

  /// Swaps width and height.
  pub const fn landscape(self) -> Self {
    Self {
      width: self.height,
      height: self.width,
      ..self
    }
  }

  /// Replaces the margins with a uniform value.
  pub const fn with_margin(self, margin: f32) -> Self {
    Self {
      margin: PageMargins::uniform(margin),
      ..self
    }
  }

  const fn content_size(&self) -> (f32, f32) {
    (
      self.width - self.margin.left - self.margin.right,
      self.height - self.margin.top - self.margin.bottom,
    )
  }
}

/// Shared inputs for laying out an independent node tree: the main content or
/// a header/footer band.
struct TreeInputs<'g> {
  fonts: &'g Fonts,
  stylesheet: Arc<StyleSheet>,
  images: Rc<HashMap<Arc<str>, ImageSource>>,
  font_families: Option<FontFamily>,
  lang: Option<Lang>,
}

/// A node tree taken through layout and scene building, ready to emit.
struct PreparedTree {
  root: RenderNode,
  results: LayoutResults,
  contexts: Vec<StackingContextNode>,
  width: f32,
  height: f32,
}

impl PreparedTree {
  fn emitter<'a>(&'a self, fonts: &'a mut FontMap) -> Emitter<'a> {
    Emitter {
      root: &self.root,
      contexts: &self.contexts,
      results: &self.results,
      fonts,
      window: None,
      line_window: None,
    }
  }
}

fn prepare_tree(
  inputs: &TreeInputs<'_>,
  node: Node,
  viewport: Viewport,
) -> Result<PreparedTree, PdfError> {
  let context = RenderContext::builder()
    .fonts(
      inputs
        .fonts
        .snapshot_with_fallbacks(inputs.font_families.as_ref()),
    )
    .sizing(SizingContext::builder().viewport(viewport).build())
    .images(inputs.images.clone())
    .stylesheet(inputs.stylesheet.clone())
    .style(Box::new(ComputedStyle {
      lang: inputs.lang,
      font_family: inputs.font_families.clone().unwrap_or_default(),
      ..Default::default()
    }))
    .build();

  let root = RenderNode::from_node(&context, node);
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

  Ok(PreparedTree {
    root,
    results,
    contexts,
    width,
    height,
  })
}

const COUNTER_STYLES: [&str; 7] = [
  "decimal",
  "decimal-leading-zero",
  "lower-roman",
  "upper-roman",
  "cjk-decimal",
  "trad-chinese-informal",
  "cjk-ideographic",
];

/// Formats a page counter in a CSS `@counter-style` named style. Unknown
/// styles fall back to `decimal`.
fn format_counter(value: usize, style: &str) -> String {
  match style {
    "cjk-decimal" => value
      .to_string()
      .bytes()
      .map(|digit| CHINESE_DIGITS[usize::from(digit - b'0')])
      .collect(),
    // Blink defines cjk-ideographic as `extends trad-chinese-informal`.
    "trad-chinese-informal" | "cjk-ideographic" => chinese_informal(value),
    "lower-roman" => roman(value).to_ascii_lowercase(),
    "upper-roman" => roman(value),
    "decimal-leading-zero" => format!("{value:02}"),
    _ => value.to_string(),
  }
}

const CHINESE_DIGITS: [char; 10] = ['零', '一', '二', '三', '四', '五', '六', '七', '八', '九'];

/// Reading-style Chinese numerals (一, 十二, 一百零三) up to 9999; larger
/// values fall back to positional digits.
fn chinese_informal(value: usize) -> String {
  if value >= 10_000 {
    return format_counter(value, "cjk-decimal");
  }
  if value == 0 {
    return CHINESE_DIGITS[0].to_string();
  }
  let mut out = String::new();
  let mut needs_zero = false;

  for (unit, name) in [
    (1000, Some('千')),
    (100, Some('百')),
    (10, Some('十')),
    (1, None),
  ] {
    let digit = value / unit % 10;

    if digit == 0 {
      needs_zero = !out.is_empty();
      continue;
    }
    if needs_zero {
      out.push(CHINESE_DIGITS[0]);
      needs_zero = false;
    }
    // 10-19 reads 十 not 一十.
    if !(unit == 10 && digit == 1 && value < 20) {
      out.push(CHINESE_DIGITS[digit]);
    }
    if let Some(name) = name {
      out.push(name);
    }
  }
  out
}

fn roman(value: usize) -> String {
  const NUMERALS: [(usize, &str); 13] = [
    (1000, "M"),
    (900, "CM"),
    (500, "D"),
    (400, "CD"),
    (100, "C"),
    (90, "XC"),
    (50, "L"),
    (40, "XL"),
    (10, "X"),
    (9, "IX"),
    (5, "V"),
    (4, "IV"),
    (1, "I"),
  ];
  let mut remaining = value;
  let mut out = String::new();

  for (unit, numeral) in NUMERALS {
    while remaining >= unit {
      out.push_str(numeral);
      remaining -= unit;
    }
  }
  out
}

/// The counter value a node's class hooks request, if any: `pageNumber` or
/// `totalPages`, optionally paired with a `@counter-style` name — the same
/// contract as Chromium's print header/footer templates.
fn counter_text(node: &Node, page: usize, pages: usize) -> Option<String> {
  let classes = node.class_name()?;
  let value = if classes
    .split_whitespace()
    .any(|class| class == "pageNumber")
  {
    page
  } else if classes
    .split_whitespace()
    .any(|class| class == "totalPages")
  {
    pages
  } else {
    return None;
  };
  let style = classes
    .split_whitespace()
    .find(|class| COUNTER_STYLES.contains(class))
    .unwrap_or("decimal");

  Some(format_counter(value, style))
}

/// Fills `pageNumber` / `totalPages` class hooks with the formatted counter,
/// like Chromium assigning `textContent` in its header/footer template.
fn substitute_page_counters(node: &mut Node, page: usize, pages: usize) {
  if let Some(text) = counter_text(node, page, pages) {
    match &mut node.kind {
      NodeKind::Text(data) => data.text = text,
      NodeKind::Container { children } => *children = vec![Node::text(text)],
      _ => {}
    }
    return;
  }
  if let NodeKind::Container { children } = &mut node.kind {
    for child in children {
      substitute_page_counters(child, page, pages);
    }
  }
}

/// Lays out a band template with the given counter values.
fn prepare_band(
  inputs: &TreeInputs<'_>,
  template: &Node,
  page: usize,
  pages: usize,
  viewport: Viewport,
) -> Result<PreparedTree, PdfError> {
  let mut node = template.clone();

  substitute_page_counters(&mut node, page, pages);
  prepare_tree(inputs, node, viewport)
}

/// Emits a band clipped to `(x, y, width, height)` on the page.
fn emit_band(
  band: &PreparedTree,
  fonts: &mut FontMap,
  x: f32,
  y: f32,
  width: f32,
  height: f32,
  surface: &mut Surface,
) -> Result<(), PdfError> {
  let Some(path) = KrillaRect::from_xywh(x, y, width, height).and_then(rect_path) else {
    return Ok(());
  };

  surface.push_clip_path(&path, &FillRule::NonZero);
  surface.push_transform(&Transform::from_translate(x, y));
  let mut emitter = band.emitter(fonts);

  emitter.emit_context(0, Affine::IDENTITY, surface)?;
  surface.pop();
  surface.pop();
  Ok(())
}

/// Renders a node tree to a PDF: single-page at the viewport size, or paged
/// when [`PdfOptions::page`] is set.
pub fn render(options: PdfOptions<'_>) -> Result<Vec<u8>, PdfError> {
  let inputs = TreeInputs {
    fonts: options.fonts,
    stylesheet: options.stylesheet,
    images: Rc::new(options.images),
    font_families: options.font_families,
    lang: options.lang,
  };
  let mut fonts = FontMap::new();
  let mut document = Document::new();

  match options.page {
    Some(page) => {
      let page_size =
        KrillaSize::from_wh(page.width, page.height).ok_or(PdfError::InvalidPageSize)?;
      let (content_width, content_height) = page.content_size();
      if !(content_width.is_finite()
        && content_height.is_finite()
        && content_width > 0.0
        && content_height > 0.0)
      {
        return Err(PdfError::InvalidPageSize);
      }
      let band_viewport = Viewport::new((content_width as u32, None));

      // Band heights are measured once with three-digit counters; per-page
      // emission clips to the measured band, so a wrap caused by a wider real
      // counter cannot push the content window around between pages.
      let header_height = match &options.header {
        Some(template) => prepare_band(&inputs, template, 999, 999, band_viewport)?.height,
        None => 0.0,
      };
      let footer_height = match &options.footer {
        Some(template) => prepare_band(&inputs, template, 999, 999, band_viewport)?.height,
        None => 0.0,
      };
      let window_height = content_height - header_height - footer_height;
      if window_height <= 0.0 {
        return Err(PdfError::InvalidPageSize);
      }

      let content = prepare_tree(&inputs, options.node, band_viewport)?;
      let mut atoms = Vec::new();
      let mut forced = Vec::new();

      content
        .emitter(&mut fonts)
        .collect_atoms(0, Affine::IDENTITY, &mut atoms, &mut forced)?;
      let starts = page_starts(&mut atoms, &mut forced, content.height, window_height);
      let pages = starts.len();

      for (index, &y0) in starts.iter().enumerate() {
        let mut pdf_page = document.start_page_with(PageSettings::new(page_size));
        let mut surface = pdf_page.surface();

        if let Some(template) = &options.header {
          let band = prepare_band(&inputs, template, index + 1, pages, band_viewport)?;

          emit_band(
            &band,
            &mut fonts,
            page.margin.left,
            page.margin.top,
            content_width,
            header_height,
            &mut surface,
          )?;
        }

        let content_top = page.margin.top + header_height;
        // Paint stops at the next cut: the region between a raised cut and the
        // page's full height belongs to the next page and stays blank, exactly
        // like browser print fragmentation.
        let next_start = starts.get(index + 1).copied().unwrap_or(f32::INFINITY);
        let paint_height = (next_start - y0).min(window_height);

        if let Some(path) =
          KrillaRect::from_xywh(page.margin.left, content_top, content_width, paint_height)
            .and_then(rect_path)
        {
          surface.push_clip_path(&path, &FillRule::NonZero);
          surface.push_transform(&Transform::from_translate(
            page.margin.left,
            content_top - y0,
          ));
          let mut emitter = content.emitter(&mut fonts);

          emitter.window = Some((y0, y0 + paint_height));
          emitter.line_window = Some((if index == 0 { f32::NEG_INFINITY } else { y0 }, next_start));
          emitter.emit_context(0, Affine::IDENTITY, &mut surface)?;
          surface.pop();
          surface.pop();
        }

        if let Some(template) = &options.footer {
          let band = prepare_band(&inputs, template, index + 1, pages, band_viewport)?;

          emit_band(
            &band,
            &mut fonts,
            page.margin.left,
            page.height - page.margin.bottom - footer_height,
            content_width,
            footer_height,
            &mut surface,
          )?;
        }

        surface.finish();
        pdf_page.finish();
      }
    }
    None => {
      let viewport = options.viewport.ok_or(PdfError::MissingViewport)?;
      let content = prepare_tree(&inputs, options.node, viewport)?;
      let page_size =
        KrillaSize::from_wh(content.width, content.height).ok_or(PdfError::InvalidPageSize)?;
      let mut page = document.start_page_with(PageSettings::new(page_size));
      let mut surface = page.surface();
      let mut emitter = content.emitter(&mut fonts);

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
/// height. Each cut moves up to the top of any atom straddling it, repeated
/// until no atom straddles (a raised cut can land inside another atom). An
/// atom taller than the window can never fit a page, so it does not push cuts
/// at all — matching browsers, where `break-inside: avoid` is dropped for
/// boxes taller than the fragmentainer.
fn page_starts(atoms: &mut [Atom], forced: &mut Vec<f32>, total: f32, window: f32) -> Vec<f32> {
  atoms.sort_by(|a, b| a.0.total_cmp(&b.0));
  forced.retain(|cut| *cut > 1.0 && *cut < total - 1.0);
  forced.sort_by(f32::total_cmp);

  let mut starts = vec![0.0_f32];
  let mut y0 = 0.0_f32;

  loop {
    let limit = y0 + window;

    if let Some(cut) = forced.iter().copied().find(|cut| *cut > y0 + 1.0)
      && cut <= limit
    {
      starts.push(cut);
      y0 = cut;
      continue;
    }
    if limit >= total {
      break;
    }
    let mut cut = limit;

    loop {
      let pushed_up = atoms
        .iter()
        .filter(|(top, bottom)| *top < cut && *bottom > cut && bottom - top <= window)
        .map(|(top, _)| *top)
        .fold(cut, f32::min);

      if pushed_up >= cut {
        break;
      }
      if pushed_up <= y0 + 1.0 {
        cut = limit;
        break;
      }
      cut = pushed_up;
    }

    starts.push(cut);
    y0 = cut;
  }
  starts
}

/// Scene walker state: the render tree, the stacking-context scene, and a cache
/// of krilla fonts keyed by the backing blob identity.
type FontMap = HashMap<(u64, u32), Font>;

struct Emitter<'a> {
  root: &'a RenderNode,
  contexts: &'a [StackingContextNode],
  results: &'a LayoutResults,
  fonts: &'a mut FontMap,
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

  /// Whether a text line at `baseline` belongs to another page. Ownership is
  /// keyed on the baseline (always inside the line's own box, unlike the font
  /// ascent band, which can poke above the container a forced break cut at) and
  /// half-open, so each line is emitted exactly once.
  fn window_disowns_line(&self, baseline: f32) -> bool {
    self
      .line_window
      .is_some_and(|(y0, y1)| baseline < y0 || baseline >= y1)
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

    let (child_frame, root_pushed) = match context.root() {
      Some(paint) => self.emit_box(paint, parent, surface)?,
      None => (parent, 0),
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
    pop_transforms(surface, root_pushed);
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

    let style = &node.context.style;
    let mut pushed = 0;

    if style.mix_blend_mode != BlendMode::Normal {
      surface.push_blend_mode(krilla_blend(style.mix_blend_mode));
      pushed += 1;
    }
    if style.isolation == Isolation::Isolate {
      surface.push_isolated();
      pushed += 1;
    }
    let opacity = style.opacity.0;

    if opacity < 1.0 {
      surface
        .push_opacity(NormalizedF32::new(opacity.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE));
      pushed += 1;
    }

    let relative = parent.invert().unwrap_or(Affine::IDENTITY) * paint.transform;
    let (x, y) = if relative.only_translation() {
      (relative.x, relative.y)
    } else {
      let cols = relative.to_cols_array();

      surface.push_transform(&Transform::from_row(
        cols[0], cols[1], cols[2], cols[3], cols[4], cols[5],
      ));
      pushed += 1;
      (0.0, 0.0)
    };
    let frame = if relative.only_translation() {
      parent
    } else {
      parent * relative
    };
    // `box-decoration-break: clone`: the fragment of the box on this page
    // paints its own complete decorations (paint-only; cloned padding does not
    // reserve layout space). `slice` needs nothing — the page window slices
    // the full-box decorations, which is exactly the sliced rendering.
    let (deco_y, deco_size) = if style.box_decoration_break == BoxDecorationBreak::Clone
      && let Some((window_top, window_bottom)) = self.window
    {
      let top = y.max(window_top);
      let bottom = (y + layout.size.height).min(window_bottom);

      (
        top,
        Size {
          width: layout.size.width,
          height: (bottom - top).max(0.0),
        },
      )
    } else {
      (y, layout.size)
    };
    let border = BorderProperties::from_context(&node.context, deco_size, layout.border);

    self.emit_background(node, &border, deco_size, x, deco_y, surface);
    self.emit_background_layers(node, &border, deco_size, x, deco_y, surface);
    self.emit_borders(&border, x, deco_y, deco_size, surface);

    // Children and own content clip to the (rounded) padding box when overflow
    // is hidden; without radius a per-axis overflow leaves the visible axis
    // unbounded.
    if style.clips_overflow() {
      let clip_border = BorderProperties::from_context(&node.context, layout.size, layout.border);
      let path = if clip_border.is_zero() {
        overflow_clip_rect(style, layout, x, y)
      } else {
        let clip = ClipBox::padding_box(clip_border, layout);
        let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);

        clip
          .border
          .append_mask_commands(&mut commands, clip.size, clip.offset);
        krilla_path(&commands, x, y)
      };
      if let Some(path) = path {
        surface.push_clip_path(&path, &FillRule::NonZero);
        pushed += 1;
      }
    }

    self.emit_own_content(node, layout, x, y, surface)?;
    Ok((frame, pushed))
  }

  fn emit_background(
    &self,
    node: &RenderNode,
    border: &BorderProperties,
    size: Size<f32>,
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
    let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);

    border.append_mask_commands(&mut commands, size, CorePoint::ZERO);
    let Some(path) = krilla_path(&commands, x, y) else {
      return;
    };

    surface.set_fill(Some(fill_from_rgba(color.0, 1.0)));
    surface.draw_path(&path);
  }

  /// Paints `background-image` gradient layers, bottom layer first, clipped to
  /// the rounded border box. Each layer fills the whole positioning area.
  // ponytail: background-size/position/repeat and url() layers are not
  // resolved yet; port takumi-svg's placement logic when needed.
  fn emit_background_layers(
    &self,
    node: &RenderNode,
    border: &BorderProperties,
    size: Size<f32>,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) {
    let style = &node.context.style;
    let Some(images) = style.background_image.as_deref() else {
      return;
    };
    if images
      .iter()
      .all(|image| matches!(image, BackgroundImage::None))
    {
      return;
    }
    let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);

    border.append_mask_commands(&mut commands, size, CorePoint::ZERO);
    let Some(clip) = krilla_path(&commands, x, y) else {
      return;
    };

    surface.push_clip_path(&clip, &FillRule::NonZero);
    for image in images.iter().rev() {
      self.background_layer(image, node, size, x, y, surface);
    }
    surface.pop();
  }

  fn background_layer(
    &self,
    image: &BackgroundImage,
    node: &RenderNode,
    size: Size<f32>,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) {
    let (w, h) = (size.width, size.height);
    let sizing = &node.context.sizing;
    let current_color = node.context.current_color;

    let paint: Paint = match image {
      BackgroundImage::Linear(gradient) => {
        let tile = LinearGradientTile::new(gradient, w as u32, h as u32, sizing, current_color);
        let resolved = resolve_stops_along_axis(
          &gradient.stops,
          tile.axis_length.max(1e-6),
          sizing,
          current_color,
        );
        if resolved.is_empty() {
          return;
        }
        let max_extent = tile.axis_length / 2.0;
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        let point_at = |t: f32| {
          (
            cx + (t - max_extent) * tile.dir_x,
            cy + (t - max_extent) * tile.dir_y,
          )
        };
        let (t0, t1, base, span) = if gradient.repeating {
          let first = resolved.first().map_or(0.0, |s| s.position);
          let last = resolved.last().map_or(tile.axis_length, |s| s.position);
          (first, last, first, (last - first).max(1e-6))
        } else {
          (0.0, tile.axis_length, 0.0, tile.axis_length.max(1e-6))
        };
        let (x1, y1) = point_at(t0);
        let (x2, y2) = point_at(t1);

        KrillaLinearGradient {
          x1,
          y1,
          x2,
          y2,
          transform: Transform::identity(),
          spread_method: spread(gradient.repeating),
          stops: krilla_stops(&resolved, base, span),
          anti_alias: false,
        }
        .into()
      }
      BackgroundImage::Radial(gradient) => {
        let tile = RadialGradientTile::new(gradient, w as u32, h as u32, sizing, current_color);
        let resolved = resolve_stops_along_axis(
          &gradient.stops,
          tile.radius_scale.max(1e-6),
          sizing,
          current_color,
        );
        if resolved.is_empty() {
          return;
        }
        let radius_x = tile.inv_radius_x.max(1e-6).recip();
        let radius_y = tile.inv_radius_y.max(1e-6).recip();
        let extent = tile.radius_scale.max(1e-6);
        // PDF radial shadings cannot repeat, so a repeating gradient expands
        // its period across the full radius instead of relying on the spread.
        let stops = if gradient.repeating {
          expanded_radial_stops(&resolved, extent)
        } else {
          krilla_stops(&resolved, 0.0, extent)
        };
        let scale_x = (radius_x / extent).max(1e-6);
        let scale_y = (radius_y / extent).max(1e-6);

        KrillaRadialGradient {
          fx: 0.0,
          fy: 0.0,
          fr: 0.0,
          cx: 0.0,
          cy: 0.0,
          cr: extent,
          transform: Transform::from_row(scale_x, 0.0, 0.0, scale_y, x + tile.cx, y + tile.cy),
          spread_method: SpreadMethod::Pad,
          stops,
          anti_alias: false,
        }
        .into()
      }
      BackgroundImage::Conic(gradient) => {
        let tile = ConicGradientTile::new(gradient, w as u32, h as u32, sizing, current_color);
        let lut_len = tile.color_lut.len();
        if lut_len == 0 {
          return;
        }
        const SWEEP_STOPS: usize = 64;
        let stops = (0..=SWEEP_STOPS)
          .map(|i| {
            let t = i as f32 / SWEEP_STOPS as f32;
            let index =
              tile.lut_index_for_adjusted_angle_with_len(t * core::f32::consts::TAU, lut_len);
            let color = tile.color_lut[index].demultiply();

            krilla_stop(t, [color.red(), color.green(), color.blue(), color.alpha()])
          })
          .collect();
        let (ccx, ccy) = (x + tile.cx, y + tile.cy);

        SweepGradient {
          cx: ccx,
          cy: ccy,
          start_angle: 0.0,
          end_angle: 360.0,
          transform: Transform::from_rotate_at(tile.start_rad.to_degrees() - 90.0, ccx, ccy),
          spread_method: SpreadMethod::Pad,
          stops,
          anti_alias: false,
        }
        .into()
      }
      BackgroundImage::Url(_) | BackgroundImage::None => return,
    };

    let Some(path) = KrillaRect::from_xywh(x, y, w, h).and_then(rect_path) else {
      return;
    };

    surface.set_fill(Some(Fill {
      paint,
      opacity: NormalizedF32::ONE,
      rule: FillRule::NonZero,
    }));
    surface.draw_path(&path);
  }

  /// Fills the border ring: one even-odd fill for a uniform color, per-side
  /// trapezoids clipped to the ring otherwise.
  // ponytail: dashed/dotted/double render as solid; port the stroke-based
  // patterns from takumi-svg when someone needs them.
  fn emit_borders(
    &self,
    border: &BorderProperties,
    x: f32,
    y: f32,
    size: Size<f32>,
    surface: &mut Surface,
  ) {
    if !border.has_visible_sides() {
      return;
    }
    let mut ring = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);

    border.append_border_ring_commands(&mut ring, size);
    let Some(ring_path) = krilla_path(&ring, x, y) else {
      return;
    };

    if let Some(color) = border.has_uniform_visible_color() {
      if color.0[3] != 0 {
        surface.set_fill(Some(Fill {
          rule: FillRule::EvenOdd,
          ..fill_from_rgba(color.0, 1.0)
        }));
        surface.draw_path(&ring_path);
      }
      return;
    }

    surface.push_clip_path(&ring_path, &FillRule::EvenOdd);
    for (side, width, color, style) in [
      (
        BorderSide::Top,
        border.width.top,
        border.color.top,
        border.style.top,
      ),
      (
        BorderSide::Right,
        border.width.right,
        border.color.right,
        border.style.right,
      ),
      (
        BorderSide::Bottom,
        border.width.bottom,
        border.color.bottom,
        border.style.bottom,
      ),
      (
        BorderSide::Left,
        border.width.left,
        border.color.left,
        border.style.left,
      ),
    ] {
      if width <= 0.0 || color.0[3] == 0 || !style.is_rendered() {
        continue;
      }
      let mut polygon = Vec::new();

      border.append_side_polygon_commands_at(side, &mut polygon, size, CorePoint::ZERO);
      if let Some(path) = krilla_path(&polygon, x, y) {
        surface.set_fill(Some(fill_from_rgba(color.0, 1.0)));
        surface.draw_path(&path);
      }
    }
    surface.pop();
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
      #[cfg(feature = "images")]
      Some(NodeKind::Image(image)) => {
        self.emit_image(image, &node.context, layout, x, y, surface);
        Ok(())
      }
      _ => Ok(()),
    }
  }

  #[cfg(feature = "images")]
  /// Draws an image node into its content box, honoring `object-fit` and
  /// `object-position`. The source rasterizes at its intrinsic size and embeds
  /// once per distinct pixel data (krilla dedups by content hash).
  // ponytail: pixels upload as un-premultiplied RGBA8, so JPEG bytes re-encode
  // as flate; add DCT passthrough when PDF size from photos matters.
  fn emit_image(
    &self,
    image: &ImageData,
    context: &RenderContext,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) {
    let content = layout.content_box_size();
    let offset = layout.content_box_offset();
    let (bx, by, w, h) = (x + offset.x, y + offset.y, content.width, content.height);
    if w <= 0.0 || h <= 0.0 {
      return;
    }
    let Ok(source) = image.src.resolve(context) else {
      return;
    };

    let (iw, ih) = {
      let (width, height) = source.size(&context.sizing);
      if width <= 0.0 || height <= 0.0 {
        return;
      }
      (width, height)
    };
    let scale = match context.style.object_fit {
      ObjectFit::Contain => (w / iw).min(h / ih),
      ObjectFit::Cover => (w / iw).max(h / ih),
      ObjectFit::ScaleDown => (w / iw).min(h / ih).min(1.0),
      ObjectFit::None => 1.0,
      _ => 0.0,
    };
    let (dw, dh) = if scale == 0.0 {
      (w, h)
    } else {
      (iw * scale, ih * scale)
    };
    let Some(krilla_image) = rasterized_image(&source, context, (dw, dh)) else {
      return;
    };
    let position = context.style.object_position.0;
    let ix = bx + position_axis(position.x, context, w - dw);
    let iy = by + position_axis(position.y, context, h - dh);

    let Some(size) = KrillaSize::from_wh(dw, dh) else {
      return;
    };
    let overflows = dw > w + 0.5 || dh > h + 0.5;

    if overflows {
      let Some(path) = KrillaRect::from_xywh(bx, by, w, h).and_then(rect_path) else {
        return;
      };

      surface.push_clip_path(&path, &FillRule::NonZero);
    }
    surface.push_transform(&Transform::from_translate(ix, iy));
    surface.draw_image(krilla_image, size);
    surface.pop();
    if overflows {
      surface.pop();
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
    let font_style = SizedFontStyle::from_style(&context.style, context);
    let Some((built, runs)) = build_inline_runs(
      single_text_items(text, context),
      &font_style,
      context,
      layout,
    )?
    else {
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
      let Some(font) = self.cached_font(shaped) else {
        continue;
      };
      let offset = run.glyph_offset(layout);
      if let Some(glyph) = shaped.glyphs.first() {
        let baseline = y + offset.y + glyph.y;
        if self.window_disowns_line(baseline) {
          continue;
        }
      }
      let decorations = run_decorations(
        shaped,
        layout,
        run.baseline_shift,
        run.transform(Affine::IDENTITY),
      );

      for decoration in decorations.iter().filter(|d| !d.over) {
        draw_decoration(surface, decoration, x, y);
      }
      let run_text = built
        .text
        .get(shaped.text_range.clone())
        .unwrap_or_default();
      let spans = glyph_text_spans(shaped, run_text);

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
      for decoration in decorations.iter().filter(|d| d.over) {
        draw_decoration(surface, decoration, x, y);
      }
    }
    Ok(())
  }

  /// A krilla font for a run's backing blob, cached by the blob's stable id.
  /// Copies the blob into the cache once per distinct font.
  fn cached_font(&mut self, shaped: &ShapedRun) -> Option<Font> {
    let key = (shaped.font_id(), shaped.font_index);

    if let Some(font) = self.fonts.get(&key) {
      return Some(font.clone());
    }
    let font = Font::new(Data::from(shaped.font_data().to_vec()), shaped.font_index)?;

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
    forced: &mut Vec<f32>,
  ) -> Result<(), PdfError> {
    let Some(context) = self.contexts.get(id) else {
      return Ok(());
    };

    let child_frame = match context.root() {
      Some(paint) => self.collect_box_atoms(paint, parent, atoms, forced)?,
      None => parent,
    };

    for bucket in context.in_paint_order() {
      for item in bucket {
        match &item.kind {
          PaintItemKind::Node(paint) => {
            self.collect_box_atoms(paint, child_frame, atoms, forced)?;
          }
          PaintItemKind::Context(child) => {
            self.collect_atoms(*child, child_frame, atoms, forced)?;
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
    forced: &mut Vec<f32>,
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
    let style = &node.context.style;

    if style.break_before == BreakBetween::Page {
      forced.push(y);
    }
    if style.break_after == BreakBetween::Page {
      forced.push(y + layout.size.height);
    }
    if style.break_inside == BreakInside::Avoid {
      atoms.push((y, y + layout.size.height));
    }

    if node.should_create_inline_layout() {
      self.collect_text_atoms(collect_inline_items(node), &node.context, layout, y, atoms)?;
    } else if !node.has_anonymous_text_item_child() {
      match node.node.as_ref().map(|n| &n.kind) {
        Some(NodeKind::Text(text)) => {
          self.collect_text_atoms(
            single_text_items(text, &node.context),
            &node.context,
            layout,
            y,
            atoms,
          )?;
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

/// The inline item list for a lone text node.
fn single_text_items<'c>(text: &'c TextData, context: &'c RenderContext) -> Vec<InlineItem<'c>> {
  vec![InlineItem::Text {
    text: text.text.as_str().into(),
    context,
  }]
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

/// A single-rectangle krilla path.
fn rect_path(rect: KrillaRect) -> Option<KrillaPath> {
  let mut builder = PathBuilder::new();

  builder.push_rect(rect);
  builder.finish()
}

/// Converts takumi-core path commands to a krilla path translated by `(x, y)`.
fn krilla_path(commands: &[PathCommand], x: f32, y: f32) -> Option<KrillaPath> {
  let mut builder = PathBuilder::new();

  for command in commands {
    match command {
      PathCommand::MoveTo(p) => builder.move_to(p.x + x, p.y + y),
      PathCommand::LineTo(p) => builder.line_to(p.x + x, p.y + y),
      PathCommand::QuadTo(c, p) => builder.quad_to(c.x + x, c.y + y, p.x + x, p.y + y),
      PathCommand::CubicTo(c1, c2, p) => {
        builder.cubic_to(c1.x + x, c1.y + y, c2.x + x, c2.y + y, p.x + x, p.y + y);
      }
      PathCommand::Close => builder.close(),
    }
  }
  builder.finish()
}

/// The rectangular overflow clip: each hidden axis bounds to the padding box,
/// a visible axis is left effectively unbounded.
fn overflow_clip_rect(style: &ComputedStyle, layout: Layout, x: f32, y: f32) -> Option<KrillaPath> {
  const UNBOUNDED: f32 = 1.0e6;
  let clip_x = style.overflow_x != Overflow::Visible;
  let clip_y = style.overflow_y != Overflow::Visible;

  let (left, right) = if clip_x {
    let padding_left = x + layout.border.left;
    let padding_right = (x + layout.size.width - layout.border.right).max(padding_left);
    (padding_left, padding_right)
  } else {
    (x - UNBOUNDED, x + layout.size.width + UNBOUNDED)
  };
  let (top, bottom) = if clip_y {
    let padding_top = y + layout.border.top;
    let padding_bottom = (y + layout.size.height - layout.border.bottom).max(padding_top);
    (padding_top, padding_bottom)
  } else {
    (y - UNBOUNDED, y + layout.size.height + UNBOUNDED)
  };
  KrillaRect::from_ltrb(left, top, right, bottom).and_then(rect_path)
}

/// Fills one decoration rect under its border-box transform offset by `(x, y)`.
fn draw_decoration(surface: &mut Surface, decoration: &DecorationRect, x: f32, y: f32) {
  if decoration.color.0[3] == 0 || decoration.width <= 0.0 || decoration.height <= 0.0 {
    return;
  }
  let Some(path) =
    KrillaRect::from_xywh(0.0, 0.0, decoration.width, decoration.height).and_then(rect_path)
  else {
    return;
  };
  let [a, b, c, d, e, f] = decoration.transform;

  surface.push_transform(&Transform::from_row(a, b, c, d, e + x, f + y));
  surface.set_fill(Some(fill_from_rgba(decoration.color.0, 1.0)));
  surface.draw_path(&path);
  surface.pop();
}

/// Resolves one `object-position` axis to an offset within `available` space.
#[cfg(feature = "images")]
fn position_axis(component: PositionComponent, context: &RenderContext, available: f32) -> f32 {
  match Length::from(component) {
    Length::Auto => available * 0.5,
    length => length.to_px(&context.sizing, available),
  }
}

/// Rasterizes an image source into a krilla image. Bitmap sources rasterize at
/// their intrinsic size; SVG sources at twice the target size for print density.
#[cfg(feature = "images")]
fn rasterized_image(
  source: &ImageSource,
  context: &RenderContext,
  target: (f32, f32),
) -> Option<KrillaImage> {
  let (width, height) = match source {
    ImageSource::Bitmap(bitmap) => (bitmap.width(), bitmap.height()),
    ImageSource::Gif(gif) => gif.dimensions(),
    ImageSource::Encoded(encoded) => encoded.dimensions(),
    #[cfg(feature = "svg")]
    ImageSource::Svg(_) => (
      (target.0 * 2.0).ceil() as u32,
      (target.1 * 2.0).ceil() as u32,
    ),
    _ => return None,
  };
  if width == 0 || height == 0 {
    return None;
  }
  let rendered = source
    .render_for_layout(width, height, context.style.image_rendering, 0)
    .ok()?;
  let buffer = match &rendered {
    RenderedImage::Rasterized(buffer) => buffer.as_ref(),
    RenderedImage::Sampled { source, .. } => source.as_ref(),
  };
  let mut data = buffer.data().to_vec();

  for pixel in data.chunks_exact_mut(4) {
    let alpha = pixel[3];
    if alpha != 0 && alpha != 255 {
      let alpha16 = u16::from(alpha);
      pixel[0] = ((u16::from(pixel[0]) * 255 + alpha16 / 2) / alpha16).min(255) as u8;
      pixel[1] = ((u16::from(pixel[1]) * 255 + alpha16 / 2) / alpha16).min(255) as u8;
      pixel[2] = ((u16::from(pixel[2]) * 255 + alpha16 / 2) / alpha16).min(255) as u8;
    }
  }
  Some(KrillaImage::from_rgba8(
    data,
    buffer.width(),
    buffer.height(),
  ))
}

const fn spread(repeating: bool) -> SpreadMethod {
  if repeating {
    SpreadMethod::Repeat
  } else {
    SpreadMethod::Pad
  }
}

fn krilla_stop(offset: f32, rgba: [u8; 4]) -> Stop {
  Stop {
    offset: NormalizedF32::new(offset.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ZERO),
    color: rgb::Color::new(rgba[0], rgba[1], rgba[2]).into(),
    opacity: NormalizedF32::new(f32::from(rgba[3]) / 255.0).unwrap_or(NormalizedF32::ONE),
  }
}

fn krilla_stops(resolved: &[ResolvedGradientStop], base: f32, span: f32) -> Vec<Stop> {
  resolved
    .iter()
    .map(|stop| krilla_stop((stop.position - base) / span, stop.color.0))
    .collect()
}

/// Tiles one period of repeating stops across `extent` (the full gradient
/// radius), for shadings that cannot express a repeat natively.
fn expanded_radial_stops(resolved: &[ResolvedGradientStop], extent: f32) -> Vec<Stop> {
  let first = resolved.first().map_or(0.0, |s| s.position);
  let last = resolved.last().map_or(extent, |s| s.position);
  let period = (last - first).max(1e-6);
  let cycles = (((extent - first) / period).ceil().max(1.0)) as usize;
  let mut stops = Vec::with_capacity(cycles * resolved.len());

  for cycle in 0..cycles {
    let offset = first + cycle as f32 * period;

    for stop in resolved {
      stops.push(krilla_stop(
        (offset + stop.position - first) / extent,
        stop.color.0,
      ));
    }
  }
  stops
}

const fn krilla_blend(mode: BlendMode) -> KrillaBlendMode {
  match mode {
    BlendMode::Multiply => KrillaBlendMode::Multiply,
    BlendMode::Screen => KrillaBlendMode::Screen,
    BlendMode::Overlay => KrillaBlendMode::Overlay,
    BlendMode::Darken => KrillaBlendMode::Darken,
    BlendMode::Lighten => KrillaBlendMode::Lighten,
    BlendMode::ColorDodge => KrillaBlendMode::ColorDodge,
    BlendMode::ColorBurn => KrillaBlendMode::ColorBurn,
    BlendMode::HardLight => KrillaBlendMode::HardLight,
    BlendMode::SoftLight => KrillaBlendMode::SoftLight,
    BlendMode::Difference => KrillaBlendMode::Difference,
    BlendMode::Exclusion => KrillaBlendMode::Exclusion,
    BlendMode::Hue => KrillaBlendMode::Hue,
    BlendMode::Saturation => KrillaBlendMode::Saturation,
    BlendMode::Color => KrillaBlendMode::Color,
    BlendMode::Luminosity => KrillaBlendMode::Luminosity,
    _ => KrillaBlendMode::Normal,
  }
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

/// Per-glyph byte ranges into `run_text` for ToUnicode, from the shaper's
/// cluster segmentation (correct for ligatures and complex scripts).
fn glyph_text_spans(shaped: &ShapedRun, run_text: &str) -> Vec<Range<usize>> {
  let base = shaped.text_range.start;

  if shaped.cluster_ranges.len() == shaped.glyphs.len() {
    return shaped
      .cluster_ranges
      .iter()
      .map(|range| {
        let start = range.start.saturating_sub(base).min(run_text.len());
        let end = range.end.saturating_sub(base).min(run_text.len());

        if start <= end { start..end } else { 0..0 }
      })
      .collect();
  }

  // Alignment unknown: map every glyph to the whole run.
  vec![0..run_text.len(); shaped.glyphs.len()]
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
    assert_eq!(
      page_starts(&mut [], &mut Vec::new(), 250.0, 100.0),
      vec![0.0, 100.0, 200.0]
    );
  }

  #[test]
  fn page_starts_pushes_straddling_atom_to_next_page() {
    let mut atoms = [(90.0, 110.0)];

    assert_eq!(
      page_starts(&mut atoms, &mut Vec::new(), 250.0, 100.0),
      vec![0.0, 90.0, 190.0]
    );
  }

  #[test]
  fn page_starts_hard_cuts_atom_taller_than_window() {
    let mut atoms = [(0.0, 300.0)];

    assert_eq!(
      page_starts(&mut atoms, &mut Vec::new(), 300.0, 100.0),
      vec![0.0, 100.0, 200.0]
    );
  }

  #[test]
  fn page_starts_honors_forced_cuts() {
    let mut forced = vec![40.0, 150.0];

    assert_eq!(
      page_starts(&mut [], &mut forced, 250.0, 100.0),
      vec![0.0, 40.0, 140.0, 150.0]
    );
  }

  #[test]
  fn presets_match_css_page_keywords() {
    let a4 = PageOptions::A4;

    assert!((a4.width - 793.7).abs() < 0.1);
    assert!((a4.height - 1122.5).abs() < 0.1);

    let letter = PageOptions::LETTER;

    assert_eq!((letter.width, letter.height), (816.0, 1056.0));

    let landscape = PageOptions::A4.landscape();

    assert_eq!(landscape.width, a4.height);
    assert_eq!(landscape.height, a4.width);
    assert_eq!(PageOptions::A4.with_margin(0.0).margin.top, 0.0);
  }
}
