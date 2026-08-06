#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Vector PDF output for takumi.
//!
//! [`render`] runs takumi-core layout, walks the same backend-agnostic
//! stacking-context scene as `takumi-svg`, and emits a PDF through a vendored
//! krilla fork:
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
//! `break-inside: avoid`; repeated header/footer bands draw in the page
//! margin areas like Chromium's print templates. Nodes classed `pageNumber`
//! / `totalPages` are filled with the page counters.
//!
//! Coverage: backgrounds (color and gradient layers), borders and radius,
//! images (`object-fit`/`object-position`), text with decorations, opacity,
//! blend modes, overflow clipping, affine transforms, pagination. Not yet:
//! box-shadow, filters, `clip-path`, masks, `background-size`/`position`/
//! `repeat`, url() background layers.

use std::{cell::RefCell, collections::HashMap, ops::Range, rc::Rc, sync::Arc};

#[cfg(all(feature = "svg", feature = "images"))]
mod svg;
mod tags;

#[allow(
  dead_code,
  missing_docs,
  clippy::all,
  clippy::redundant_closure_for_method_calls,
  clippy::unwrap_used,
  clippy::expect_used,
  clippy::panic
)]
mod krilla;
#[allow(
  dead_code,
  missing_docs,
  clippy::all,
  clippy::redundant_closure_for_method_calls,
  clippy::unwrap_used,
  clippy::expect_used,
  clippy::panic
)]
mod subsetter;

#[cfg(feature = "images")]
use crate::krilla::image::Image as KrillaImage;
use crate::krilla::{
  Data, Document, SerializeSettings,
  action::{Action, LinkAction},
  annotation::{Annotation, LinkAnnotation, Target},
  blend::BlendMode as KrillaBlendMode,
  color::rgb,
  configure::{Accessibility, Archival, ConfigurationBuilder},
  destination::XyzDestination,
  error::KrillaError,
  geom::{
    Path as KrillaPath, PathBuilder, Point, Rect as KrillaRect, Size as KrillaSize, Transform,
  },
  metadata::{DateTime, Metadata},
  num::NormalizedF32,
  outline::{Outline, OutlineNode},
  page::PageSettings,
  paint::{
    Fill, FillRule, LinearGradient as KrillaLinearGradient, Paint,
    RadialGradient as KrillaRadialGradient, SpreadMethod, Stop, SweepGradient,
  },
  surface::Surface,
  tagging::{Artifact, ArtifactType, ContentTag},
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
    ComputedStyle, Display, FlexDirection, FontFamily, Isolation, Lang, Length, Overflow,
    ResolvedGradientStop, SizingContext, Style, StyleDeclaration, StyleSheet,
  },
  viewport::Viewport,
};
#[cfg(feature = "images")]
use takumi_core::{
  layout::node::ImageData,
  resources::image::RenderedImage,
  style::{ObjectFit, PositionComponent},
};
use typed_builder::TypedBuilder;

use crate::tags::{TagCollector, build_tag_tree};

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
  /// The requested archival standard could not be configured.
  InvalidStandard,
}

impl From<TakumiError> for PdfError {
  fn from(error: TakumiError) -> Self {
    Self::Render(error)
  }
}

/// Archival standard the output conforms to.
///
/// Levels that require tagged PDF (the `a` conformance levels, PDF/UA) are not
/// offered. PDF/A-1 is omitted too: it prohibits transparency, which most
/// takumi output uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PdfStandard {
  /// Plain PDF 1.7, no validation.
  #[default]
  None,
  /// PDF/A-2b: archival, basic conformance.
  A2b,
  /// PDF/A-2u: archival with guaranteed Unicode mapping.
  A2u,
  /// PDF/A-3b: PDF/A-2b plus arbitrary file attachments.
  A3b,
  /// PDF/A-3u: PDF/A-2u plus arbitrary file attachments.
  A3u,
  /// PDF/A-4: archival, PDF 2.0.
  A4,
  /// PDF/A-2a: PDF/A-2 with accessibility (tagged) conformance.
  A2a,
  /// PDF/A-3a: PDF/A-3 with accessibility (tagged) conformance.
  A3a,
}

impl PdfStandard {
  fn archival(self) -> Option<Archival> {
    match self {
      PdfStandard::None => None,
      PdfStandard::A2b => Some(Archival::A2_B),
      PdfStandard::A2u => Some(Archival::A2_U),
      PdfStandard::A3b => Some(Archival::A3_B),
      PdfStandard::A3u => Some(Archival::A3_U),
      PdfStandard::A4 => Some(Archival::A4),
      PdfStandard::A2a => Some(Archival::A2_A),
      PdfStandard::A3a => Some(Archival::A3_A),
    }
  }

  fn requires_tagging(self) -> bool {
    matches!(self, PdfStandard::A2a | PdfStandard::A3a)
  }
}

/// Whether the output carries a tagged structure tree, and to which standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tagging {
  /// No structure tree.
  Off,
  /// Structure tree from the HTML semantics, unvalidated.
  #[default]
  On,
  /// Structure tree validated against PDF/UA-1. Requires a PDF 1.7 level in
  /// [`PdfOptions::standard`] (PDF/A-4 is PDF 2.0; the ranges do not
  /// overlap).
  Ua1,
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
  /// band lays out at full page width and draws in the top margin area, like
  /// Chromium's print templates; it does not shrink the content window.
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
  /// Document metadata written to the PDF's info dictionary.
  #[builder(default, setter(strip_option))]
  pub metadata: Option<PdfMetadata>,
  /// Generates a PDF outline (bookmarks) from `h1`–`h6` headings.
  #[builder(default)]
  pub outline: bool,
  /// Archival standard the output conforms to. Validation failures fail the
  /// render.
  #[builder(default)]
  pub standard: PdfStandard,
  /// Structure-tree emission: on by default like Chromium's print-to-PDF,
  /// optionally validated against PDF/UA-1. The tagged standards (`A2a`,
  /// `A3a`) force it on.
  #[builder(default)]
  pub tagged: Tagging,
}

/// Document metadata for the PDF's info dictionary. [`PdfOptions::lang`]
/// doubles as the metadata language.
#[derive(Default, Clone)]
pub struct PdfMetadata {
  /// The document title.
  pub title: Option<String>,
  /// The document description (the info dictionary's subject).
  pub description: Option<String>,
  /// The document authors.
  pub authors: Vec<String>,
  /// The document keywords.
  pub keywords: Vec<String>,
  /// The tool that created the source document.
  pub creator: Option<String>,
  /// The document creation date, interpreted as UTC. Tagged archival
  /// standards require one; supplying it keeps output deterministic.
  pub creation_date: Option<PdfDate>,
}

/// A UTC timestamp for [`PdfMetadata::creation_date`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PdfDate {
  /// Full year, e.g. 2026.
  pub year: u16,
  /// Month `1..=12`.
  pub month: u8,
  /// Day of month `1..=31`.
  pub day: u8,
  /// Hour `0..=23`.
  pub hour: u8,
  /// Minute `0..=59`.
  pub minute: u8,
  /// Second `0..=59`.
  pub second: u8,
}

fn build_metadata(metadata: &PdfMetadata, lang: Option<Lang>) -> Metadata {
  let mut result = Metadata::new();

  if let Some(title) = &metadata.title {
    result = result.title(title.clone());
  }
  if let Some(description) = &metadata.description {
    result = result.description(description.clone());
  }
  if !metadata.authors.is_empty() {
    result = result.authors(metadata.authors.clone());
  }
  if !metadata.keywords.is_empty() {
    result = result.keywords(metadata.keywords.clone());
  }
  if let Some(creator) = &metadata.creator {
    result = result.creator(creator.clone());
  }
  if let Some(lang) = lang {
    result = result.language(lang.as_str().to_string());
  }
  if let Some(date) = metadata.creation_date {
    result = result.creation_date(
      DateTime::new(date.year)
        .month(date.month)
        .day(date.day)
        .hour(date.hour)
        .minute(date.minute)
        .second(date.second)
        .utc_offset_hour(0),
    );
  }
  result
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

/// CSS px (96 dpi) to PDF pt (72 dpi). Layout runs in px; page geometry,
/// annotations, and destinations are written in pt so pages print at their
/// physical size.
const PT_PER_PX: f32 = 72.0 / 96.0;

/// Chromium's print template page insets bands 15pt from the paper edge
/// (`#header { padding-top: 15pt }`, `#footer { padding-bottom: 15pt }` in
/// components/printing/resources/print_header_footer_template_page.html).
const BAND_EDGE_PADDING: f32 = 20.0;

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
  fn emitter<'a>(
    &'a self,
    fonts: &'a mut FontMap,
    inline: Option<&'a InlineMap<'a>>,
    tags: Option<&'a RefCell<TagCollector>>,
  ) -> Emitter<'a> {
    Emitter {
      root: &self.root,
      contexts: &self.contexts,
      results: &self.results,
      fonts,
      inline,
      window: None,
      line_window: None,
      tags,
    }
  }
}

// The root fills the content box like a browser body: a fit-content root
// resolves child percentages against a tentative width first and the final
// width later, and taffy does not reconcile heights across those passes.
fn fill_root(node: Node, viewport: Viewport) -> Node {
  let mut style = Style::default()
    .with(StyleDeclaration::display(Display::Flex))
    .with(StyleDeclaration::flex_direction(FlexDirection::Column))
    .with(StyleDeclaration::width(Length::Percentage(100.0)));

  if viewport.size.height.is_some() {
    style = style.with(StyleDeclaration::height(Length::Percentage(100.0)));
  }

  Node::container([node]).with_style(style)
}

fn prepare_tree(
  inputs: &TreeInputs<'_>,
  node: Node,
  viewport: Viewport,
) -> Result<PreparedTree, PdfError> {
  let node = fill_root(node, viewport);
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

/// A band measured before pagination: the laid-out tree plus whether it must
/// re-prepare per page (it contains counter hooks).
struct MeasuredBand {
  measured: PreparedTree,
  dynamic: bool,
}

/// Measures a band with three-digit counters and records whether it must
/// re-prepare per page.
fn measure_band(
  inputs: &TreeInputs<'_>,
  template: &Node,
  viewport: Viewport,
) -> Result<MeasuredBand, PdfError> {
  Ok(MeasuredBand {
    measured: prepare_band(inputs, template, 999, 999, viewport)?,
    dynamic: band_has_counters(template),
  })
}

/// Whether a band template contains any page-counter hook. A band without one
/// lays out identically on every page, so it is prepared once and reused.
fn band_has_counters(node: &Node) -> bool {
  if counter_text(node, 1, 1).is_some() {
    return true;
  }
  match &node.kind {
    NodeKind::Container { children } => children.iter().any(band_has_counters),
    _ => false,
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
  bounds: (f32, f32, f32, f32),
  artifact: bool,
  surface: &mut Surface,
) -> Result<(), PdfError> {
  let (x, y, width, height) = bounds;
  let Some(path) = KrillaRect::from_xywh(x, y, width, height).and_then(rect_path) else {
    return Ok(());
  };

  if artifact {
    // `Other` stays valid below PDF 2.0, where the header/footer artifact
    // subtypes do not exist yet.
    surface.start_tagged(ContentTag::Artifact(Artifact::new(
      ArtifactType::Other,
      None,
    )));
  }
  surface.push_clip_path(&path, &FillRule::NonZero);
  surface.push_transform(&Transform::from_translate(x, y));
  let mut emitter = band.emitter(fonts, None, None);

  emitter.emit_context(0, Affine::IDENTITY, surface)?;
  surface.pop();
  surface.pop();
  if artifact {
    surface.end_tagged();
  }
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
  let mut document = {
    let mut builder = ConfigurationBuilder::new();
    let mut validated = false;

    if let Some(archival) = options.standard.archival() {
      builder = builder.with_archival_validator(archival);
      validated = true;
    }
    if options.tagged == Tagging::Ua1 {
      builder = builder.with_accessibility_validator(Accessibility::UA1);
      validated = true;
    }
    if validated {
      let configuration = builder.finish().map_err(|_| PdfError::InvalidStandard)?;

      Document::new_with(SerializeSettings {
        configuration,
        ..SerializeSettings::default()
      })
    } else {
      Document::new()
    }
  };
  let tag_collector = (options.tagged != Tagging::Off || options.standard.requires_tagging())
    .then(|| RefCell::new(TagCollector::default()));

  if let Some(metadata) = &options.metadata {
    document.set_metadata(build_metadata(metadata, inputs.lang));
  } else if tag_collector.is_some() && inputs.lang.is_some() {
    // Tagged standards check the document language even without metadata.
    document.set_metadata(build_metadata(&PdfMetadata::default(), inputs.lang));
  }
  match options.page {
    Some(page) => {
      let page_size = KrillaSize::from_wh(page.width * PT_PER_PX, page.height * PT_PER_PX)
        .ok_or(PdfError::InvalidPageSize)?;
      let (content_width, content_height) = page.content_size();
      if !(content_width.is_finite()
        && content_height.is_finite()
        && content_width > 0.0
        && content_height > 0.0)
      {
        return Err(PdfError::InvalidPageSize);
      }
      // Bands lay out at full page width and draw inside the margin areas,
      // like Chromium's print header and footer templates. The content window
      // is always the full margin box; a band taller than its margin overlaps
      // content, exactly as in Chromium.
      let band_viewport = Viewport::new((page.width as u32, None));
      let content_viewport = Viewport::new((content_width as u32, None));

      // Band heights are measured once with three-digit counters; per-page
      // emission clips to the measured band, so a wrap caused by a wider real
      // counter cannot move the band box between pages. A band without counter
      // hooks reuses the measured layout on every page.
      let header_band = options
        .header
        .as_ref()
        .map(|template| measure_band(&inputs, template, band_viewport))
        .transpose()?;
      let footer_band = options
        .footer
        .as_ref()
        .map(|template| measure_band(&inputs, template, band_viewport))
        .transpose()?;
      let header_height = header_band
        .as_ref()
        .map_or(0.0, |band| band.measured.height);
      let footer_height = footer_band
        .as_ref()
        .map_or(0.0, |band| band.measured.height);
      let window_height = content_height;

      let content = prepare_tree(&inputs, options.node, content_viewport)?;
      let text_boxes = collect_text_boxes(&content);
      let inline_map = build_inline_map(&text_boxes)?;
      let mut atoms = Vec::new();
      let mut forced = Vec::new();

      content
        .emitter(&mut fonts, Some(&inline_map), None)
        .collect_atoms(0, Affine::IDENTITY, &mut atoms, &mut forced)?;
      let starts = page_starts(&mut atoms, &mut forced, content.height, window_height);
      let pages = starts.len();
      let (links, headings) = collect_interactive(&content);

      for (index, &y0) in starts.iter().enumerate() {
        let mut pdf_page = document.start_page_with(PageSettings::new(page_size));
        let mut surface = pdf_page.surface();

        surface.push_transform(&Transform::from_scale(PT_PER_PX, PT_PER_PX));
        if let (Some(band), Some(template)) = (&header_band, &options.header) {
          let prepared;
          let tree = if band.dynamic {
            prepared = prepare_band(&inputs, template, index + 1, pages, band_viewport)?;
            &prepared
          } else {
            &band.measured
          };

          emit_band(
            tree,
            &mut fonts,
            (0.0, BAND_EDGE_PADDING, page.width, header_height),
            tag_collector.is_some(),
            &mut surface,
          )?;
        }

        let content_top = page.margin.top;
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
          let mut emitter = content.emitter(&mut fonts, Some(&inline_map), tag_collector.as_ref());

          emitter.window = Some((y0, y0 + paint_height));
          emitter.line_window = Some((if index == 0 { f32::NEG_INFINITY } else { y0 }, next_start));
          emitter.emit_context(0, Affine::IDENTITY, &mut surface)?;
          surface.pop();
          surface.pop();
        }

        if let (Some(band), Some(template)) = (&footer_band, &options.footer) {
          let prepared;
          let tree = if band.dynamic {
            prepared = prepare_band(&inputs, template, index + 1, pages, band_viewport)?;
            &prepared
          } else {
            &band.measured
          };

          emit_band(
            tree,
            &mut fonts,
            (
              0.0,
              page.height - BAND_EDGE_PADDING - footer_height,
              page.width,
              footer_height,
            ),
            tag_collector.is_some(),
            &mut surface,
          )?;
        }

        surface.pop();
        surface.finish();
        add_link_annotations(
          &mut pdf_page,
          &links,
          (y0, y0 + paint_height),
          (page.margin.left, content_top),
          tag_collector.as_ref(),
        );
        pdf_page.finish();
      }

      if (options.outline || options.tagged == Tagging::Ua1) && !headings.is_empty() {
        document.set_outline(build_outline(&headings, |heading| {
          let index = starts
            .partition_point(|start| *start <= heading.top)
            .saturating_sub(1);
          let y = page.margin.top + (heading.top - starts[index]).max(0.0);

          XyzDestination::new(
            index,
            Point::from_xy(page.margin.left * PT_PER_PX, y * PT_PER_PX),
          )
        }));
      }
      if let Some(collector) = &tag_collector {
        document.set_tag_tree(build_tag_tree(
          &content.root,
          inputs.lang.as_ref().map(Lang::as_str),
          &mut collector.borrow_mut(),
        ));
      }
    }
    None => {
      let viewport = options.viewport.ok_or(PdfError::MissingViewport)?;
      let content = prepare_tree(&inputs, options.node, viewport)?;
      let page_size = KrillaSize::from_wh(content.width * PT_PER_PX, content.height * PT_PER_PX)
        .ok_or(PdfError::InvalidPageSize)?;
      let text_boxes = collect_text_boxes(&content);
      let inline_map = build_inline_map(&text_boxes)?;
      let mut page = document.start_page_with(PageSettings::new(page_size));
      let mut surface = page.surface();

      surface.push_transform(&Transform::from_scale(PT_PER_PX, PT_PER_PX));
      let mut emitter = content.emitter(&mut fonts, Some(&inline_map), tag_collector.as_ref());

      emitter.emit_context(0, Affine::IDENTITY, &mut surface)?;
      surface.pop();
      surface.finish();
      let (links, headings) = collect_interactive(&content);

      add_link_annotations(
        &mut page,
        &links,
        (0.0, content.height),
        (0.0, 0.0),
        tag_collector.as_ref(),
      );
      page.finish();
      if (options.outline || options.tagged == Tagging::Ua1) && !headings.is_empty() {
        document.set_outline(build_outline(&headings, |heading| {
          XyzDestination::new(0, Point::from_xy(0.0, heading.top.max(0.0) * PT_PER_PX))
        }));
      }
      if let Some(collector) = &tag_collector {
        document.set_tag_tree(build_tag_tree(
          &content.root,
          inputs.lang.as_ref().map(Lang::as_str),
          &mut collector.borrow_mut(),
        ));
      }
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

/// A hyperlink box in content coordinates.
struct LinkTarget {
  uri: String,
  rect: KrillaRect,
  /// Source-node path, so the annotation can join that node's `Link` element.
  path: Vec<usize>,
}

/// A heading in content coordinates, for the outline.
struct HeadingTarget {
  level: u8,
  text: String,
  top: f32,
}

/// The axis-aligned bounding box of a node-local rect under the node's
/// absolute transform, in content coordinates.
fn transformed_rect(transform: Affine, origin: (f32, f32), size: Size<f32>) -> Option<KrillaRect> {
  let cols = transform.to_cols_array();
  let corners = [
    (origin.0, origin.1),
    (origin.0 + size.width, origin.1),
    (origin.0, origin.1 + size.height),
    (origin.0 + size.width, origin.1 + size.height),
  ];
  let mut left = f32::INFINITY;
  let mut top = f32::INFINITY;
  let mut right = f32::NEG_INFINITY;
  let mut bottom = f32::NEG_INFINITY;

  for (x, y) in corners {
    let px = cols[0] * x + cols[2] * y + cols[4];
    let py = cols[1] * x + cols[3] * y + cols[5];

    left = left.min(px);
    top = top.min(py);
    right = right.max(px);
    bottom = bottom.max(py);
  }
  KrillaRect::from_ltrb(left, top, right, bottom)
}

fn heading_level(tag: &str) -> Option<u8> {
  let mut bytes = tag.bytes();

  if !bytes.next()?.eq_ignore_ascii_case(&b'h') {
    return None;
  }
  let level = bytes.next()?;

  if bytes.next().is_none() && (b'1'..=b'6').contains(&level) {
    Some(level - b'0')
  } else {
    None
  }
}

fn node_text(node: &Node, out: &mut String) {
  match &node.kind {
    NodeKind::Text(text) => out.push_str(&text.text),
    NodeKind::Container { children } => {
      for child in children {
        node_text(child, out);
      }
    }
    _ => {}
  }
}

/// Collects hyperlinks and headings from the prepared scene, in paint order.
fn collect_interactive(tree: &PreparedTree) -> (Vec<LinkTarget>, Vec<HeadingTarget>) {
  let mut links = Vec::new();
  let mut headings = Vec::new();

  collect_interactive_context(tree, 0, &mut links, &mut headings);
  headings.sort_by(|a, b| a.top.total_cmp(&b.top));
  (links, headings)
}

fn collect_interactive_context(
  tree: &PreparedTree,
  id: usize,
  links: &mut Vec<LinkTarget>,
  headings: &mut Vec<HeadingTarget>,
) {
  let Some(context) = tree.contexts.get(id) else {
    return;
  };

  if let Some(paint) = context.root() {
    collect_interactive_paint(tree, paint, links, headings);
  }
  for bucket in context.in_paint_order() {
    for item in bucket {
      match &item.kind {
        PaintItemKind::Node(paint) => collect_interactive_paint(tree, paint, links, headings),
        PaintItemKind::Context(child) => {
          collect_interactive_context(tree, *child, links, headings);
        }
      }
    }
  }
}

fn collect_interactive_paint(
  tree: &PreparedTree,
  paint: &NodePaint,
  links: &mut Vec<LinkTarget>,
  headings: &mut Vec<HeadingTarget>,
) {
  let Some(node) = tree.root.node_at_path(&paint.path) else {
    return;
  };
  let Ok(layout) = tree.results.layout(paint.node_id) else {
    return;
  };
  let Some(source) = node.node.as_ref() else {
    return;
  };
  let Some(rect) = transformed_rect(paint.transform, (0.0, 0.0), layout.size) else {
    return;
  };

  match source.href().filter(|uri| allowed_link_uri(uri)) {
    // The whole box is one link; per-run collection would double-annotate it.
    Some(uri) => links.push(LinkTarget {
      uri: uri.to_string(),
      rect,
      path: paint.path.clone(),
    }),
    None if node.should_create_inline_layout() => {
      collect_inline_links(node, layout, paint.transform, &paint.path, links);
    }
    None => {}
  }
  if let Some(level) = source.tag_name().and_then(heading_level) {
    let mut text = String::new();

    node_text(source, &mut text);
    let text = text.trim();

    if !text.is_empty() {
      headings.push(HeadingTarget {
        level,
        text: text.to_string(),
        top: rect.top(),
      });
    }
  }
}

/// Whether an `href` is written to the PDF: `http`, `https`, `mailto`, or
/// `tel`. Other schemes (and scheme-less values, which have no meaning inside
/// a standalone document) are dropped.
fn allowed_link_uri(uri: &str) -> bool {
  let Some((scheme, _)) = uri.split_once(':') else {
    return false;
  };

  ["http", "https", "mailto", "tel"]
    .iter()
    .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
}

/// Measures a box's inline layout and records one link box per glyph run that
/// sits inside an anchor.
fn collect_inline_links(
  node: &RenderNode,
  layout: Layout,
  transform: Affine,
  path: &[usize],
  links: &mut Vec<LinkTarget>,
) {
  let context = &node.context;
  let font_style = SizedFontStyle::from_style(&context.style, context);
  let content = layout.content_box_size();

  if font_style.sizing.font_size == 0.0 || content.width <= 0.0 || content.height <= 0.0 {
    return;
  }
  let items = collect_inline_items(node);

  if !items
    .iter()
    .any(|item| matches!(item, InlineItem::Text { link: Some(_), .. }))
  {
    return;
  }
  let built = create_inline_layout(InlineLayoutRequest {
    items,
    available_space: Size {
      width: AvailableSpace::Definite(content.width),
      height: AvailableSpace::Definite(content.height),
    },
    max_width: content.width,
    max_height: resolve_inline_max_height(&font_style, content.height),
    style: &font_style,
    context,
    mode: InlineLayoutMode::Measure,
    shape_cacheable: true,
  });
  let (runs, _) = built.measure_runs(layout);

  for run in runs {
    let Some(uri) = run.link.filter(|uri| allowed_link_uri(uri)) else {
      continue;
    };
    let Some(rect) = transformed_rect(
      transform,
      (run.x, run.y),
      Size {
        width: run.width,
        height: run.height,
      },
    ) else {
      continue;
    };

    links.push(LinkTarget {
      uri: uri.to_string(),
      rect,
      path: path.to_vec(),
    });
  }
}

/// Adds this page's slice of every link as annotations. `window` is the page's
/// content window in content coordinates; `offset` maps content to page
/// coordinates.
fn add_link_annotations(
  page: &mut crate::krilla::page::Page,
  links: &[LinkTarget],
  window: (f32, f32),
  offset: (f32, f32),
  tags: Option<&RefCell<TagCollector>>,
) {
  for link in links {
    let top = link.rect.top().max(window.0);
    let bottom = link.rect.bottom().min(window.1);

    if bottom <= top {
      continue;
    }
    let Some(rect) = KrillaRect::from_ltrb(
      (link.rect.left() + offset.0) * PT_PER_PX,
      (top - window.0 + offset.1) * PT_PER_PX,
      (link.rect.right() + offset.0) * PT_PER_PX,
      (bottom - window.0 + offset.1) * PT_PER_PX,
    ) else {
      continue;
    };

    let annotation = Annotation::new_link(
      LinkAnnotation::new(
        rect,
        Target::Action(Action::Link(LinkAction::new(link.uri.clone()))),
      ),
      // Tagged output requires alt text on link annotations; the target URI
      // is the honest description available.
      tags.is_some().then(|| link.uri.clone()),
    );

    match tags {
      Some(tags) => {
        let identifier = page.add_tagged_annotation(annotation);

        tags.borrow_mut().record_annotation(&link.path, identifier);
      }
      None => page.add_annotation(annotation),
    }
  }
}

/// Nests flat headings into an outline tree: a heading adopts the following
/// deeper headings as children, like an HTML document outline.
fn build_outline(
  headings: &[HeadingTarget],
  destination: impl Fn(&HeadingTarget) -> XyzDestination,
) -> Outline {
  fn take(
    headings: &[HeadingTarget],
    index: &mut usize,
    level: u8,
    destination: &impl Fn(&HeadingTarget) -> XyzDestination,
  ) -> Vec<OutlineNode> {
    let mut nodes = Vec::new();

    while let Some(heading) = headings.get(*index) {
      if heading.level < level {
        break;
      }
      *index += 1;
      let mut node = OutlineNode::new(heading.text.clone(), destination(heading));

      for child in take(headings, index, heading.level + 1, destination) {
        node.push_child(child);
      }
      nodes.push(node);
    }
    nodes
  }

  let mut outline = Outline::new();
  let mut index = 0;

  for node in take(headings, &mut index, 1, &destination) {
    outline.push_child(node);
  }
  outline
}

/// A text box's inline layout, built once per render and reused by atom
/// collection and every page's emission.
struct PreparedInline<'c> {
  built: BuiltInlineLayout<'c>,
  runs: InlineRunLayout,
}

/// Inline layouts keyed by the source [`RenderNode`]'s address.
type InlineMap<'c> = HashMap<usize, PreparedInline<'c>>;

fn inline_key(node: &RenderNode) -> usize {
  std::ptr::from_ref(node) as usize
}

/// The text-bearing boxes of a prepared tree, with the resolved font style
/// each inline layout borrows.
fn collect_text_boxes<'t>(tree: &'t PreparedTree) -> Vec<TextBox<'t>> {
  let mut boxes = Vec::new();

  collect_text_boxes_context(tree, 0, &mut boxes);
  boxes
}

struct TextBox<'t> {
  node: &'t RenderNode,
  layout: Layout,
  font_style: SizedFontStyle<'t>,
}

fn collect_text_boxes_context<'t>(tree: &'t PreparedTree, id: usize, boxes: &mut Vec<TextBox<'t>>) {
  let Some(context) = tree.contexts.get(id) else {
    return;
  };

  if let Some(paint) = context.root() {
    collect_text_boxes_paint(tree, paint, boxes);
  }
  for bucket in context.in_paint_order() {
    for item in bucket {
      match &item.kind {
        PaintItemKind::Node(paint) => collect_text_boxes_paint(tree, paint, boxes),
        PaintItemKind::Context(child) => collect_text_boxes_context(tree, *child, boxes),
      }
    }
  }
}

fn collect_text_boxes_paint<'t>(
  tree: &'t PreparedTree,
  paint: &NodePaint,
  boxes: &mut Vec<TextBox<'t>>,
) {
  let Some(node) = tree.root.node_at_path(&paint.path) else {
    return;
  };
  let Ok(layout) = tree.results.layout(paint.node_id) else {
    return;
  };
  let is_text = node.should_create_inline_layout()
    || (!node.has_anonymous_text_item_child()
      && matches!(node.node.as_ref().map(|n| &n.kind), Some(NodeKind::Text(_))));

  if is_text {
    boxes.push(TextBox {
      node,
      layout,
      font_style: SizedFontStyle::from_style(&node.context.style, &node.context),
    });
  }
}

/// Builds every text box's inline layout once. Entries borrow the boxes'
/// font styles, so the map lives no longer than `boxes`.
fn build_inline_map<'c>(boxes: &'c [TextBox<'c>]) -> Result<InlineMap<'c>, PdfError> {
  let mut map = InlineMap::new();

  for text_box in boxes {
    let items = if text_box.node.should_create_inline_layout() {
      collect_inline_items(text_box.node)
    } else if let Some(NodeKind::Text(text)) = text_box.node.node.as_ref().map(|n| &n.kind) {
      single_text_items(text, &text_box.node.context)
    } else {
      continue;
    };

    if let Some((built, runs)) = build_inline_runs(
      items,
      &text_box.font_style,
      &text_box.node.context,
      text_box.layout,
    )? {
      map.insert(inline_key(text_box.node), PreparedInline { built, runs });
    }
  }
  Ok(map)
}

/// Scene walker state: the render tree, the stacking-context scene, and a cache
/// of krilla fonts keyed by the backing blob identity.
type FontMap = HashMap<(u64, u32), Font>;

struct Emitter<'a> {
  root: &'a RenderNode,
  contexts: &'a [StackingContextNode],
  results: &'a LayoutResults,
  fonts: &'a mut FontMap,
  /// Pre-built inline layouts for the content tree; band trees build on the
  /// fly.
  inline: Option<&'a InlineMap<'a>>,
  /// Vertical content window `[top, bottom)` of the page being emitted;
  /// paint wholly outside it is skipped so clipped-away content never reaches
  /// the content stream (or text extraction).
  window: Option<(f32, f32)>,
  /// Text-line ownership window: `[this page's cut, next page's cut)`. Wider
  /// than `window` at the edges (first page reaches up to −∞, last to +∞) and
  /// narrower at the bottom when a cut lands above the page's full height, so
  /// every line is emitted on exactly one page.
  line_window: Option<(f32, f32)>,
  /// Records a marked-content identifier per source node while drawing, for
  /// the structure tree built after emission.
  tags: Option<&'a RefCell<TagCollector>>,
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

    let tagged = self.tags.is_some() && has_own_content(node);

    if tagged {
      if decorative_image(node) {
        surface.start_tagged(ContentTag::Artifact(Artifact::new(
          ArtifactType::Other,
          None,
        )));
      } else {
        let identifier = surface.start_tagged(ContentTag::Other);

        if let Some(tags) = self.tags {
          tags.borrow_mut().record(&paint.path, identifier);
        }
      }
    }
    self.emit_own_content(node, layout, x, y, surface)?;
    if tagged {
      surface.end_tagged();
    }
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

      border.append_side_clip_polygon_commands_at(side, &mut polygon, size, CorePoint::ZERO);
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
      return self.emit_node_text(node, layout, x, y, surface);
    }
    if node.has_anonymous_text_item_child() {
      return Ok(());
    }
    match node.node.as_ref().map(|n| &n.kind) {
      Some(NodeKind::Text(_)) => self.emit_node_text(node, layout, x, y, surface),
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
  /// `object-position`. SVG sources draw as vector ops; everything else
  /// rasterizes at its intrinsic size and embeds once per distinct pixel data
  /// (krilla dedups by content hash).
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
    // SVG sources embed as vector ops; everything else rasterizes.
    #[cfg(feature = "svg")]
    let vector = if let ImageSource::Svg(svg) = &source {
      let (svg_width, svg_height) = svg.dimensions();
      if svg_width <= 0.0 || svg_height <= 0.0 {
        return;
      }
      // Fallback rasters (filters, embedded bitmaps) keep the old 2x density.
      let raster_scale = 2.0 * (dw / svg_width).max(dh / svg_height);

      Some((svg.vector_ops(raster_scale), svg_width, svg_height))
    } else {
      None
    };
    #[cfg(not(feature = "svg"))]
    let vector: Option<((), f32, f32)> = None;

    let krilla_image = if vector.is_none() {
      match rasterized_image(&source, context, (dw, dh)) {
        Some(image) => Some(image),
        None => return,
      }
    } else {
      None
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
    #[cfg(feature = "svg")]
    if let Some((ops, svg_width, svg_height)) = vector {
      let canvas = KrillaRect::from_xywh(0.0, 0.0, svg_width, svg_height).and_then(rect_path);

      surface.push_transform(&Transform::from_row(
        dw / svg_width,
        0.0,
        0.0,
        dh / svg_height,
        ix,
        iy,
      ));
      if let Some(canvas) = &canvas {
        surface.push_clip_path(canvas, &FillRule::NonZero);
      }
      svg::draw_svg_ops(surface, ops);
      if canvas.is_some() {
        surface.pop();
      }
      surface.pop();
    }
    if let Some(krilla_image) = krilla_image {
      surface.push_transform(&Transform::from_translate(ix, iy));
      surface.draw_image(krilla_image, size);
      surface.pop();
    }
    if overflows {
      surface.pop();
    }
  }

  /// Draws a text-bearing box's runs, from the pre-built inline map when the
  /// node is in it (content tree) or built on the fly (band trees).
  fn emit_node_text(
    &mut self,
    node: &RenderNode,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    if let Some(prepared) = self.inline.and_then(|map| map.get(&inline_key(node))) {
      return self.draw_runs(&prepared.runs, &prepared.built, layout, x, y, surface);
    }
    let context = &node.context;
    let Some(items) = node_inline_items(node) else {
      return Ok(());
    };
    let font_style = SizedFontStyle::from_style(&context.style, context);
    let Some((built, runs)) = build_inline_runs(items, &font_style, context, layout)? else {
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
      self.collect_text_atoms(node, layout, y, atoms)?;
    } else if !node.has_anonymous_text_item_child() {
      match node.node.as_ref().map(|n| &n.kind) {
        Some(NodeKind::Text(_)) => {
          self.collect_text_atoms(node, layout, y, atoms)?;
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
    node: &RenderNode,
    layout: Layout,
    y: f32,
    atoms: &mut Vec<Atom>,
  ) -> Result<(), PdfError> {
    if let Some(prepared) = self.inline.and_then(|map| map.get(&inline_key(node))) {
      text_line_atoms(&prepared.runs, layout, y, atoms);
      return Ok(());
    }
    let context = &node.context;
    let Some(items) = node_inline_items(node) else {
      return Ok(());
    };
    let font_style = SizedFontStyle::from_style(&context.style, context);
    let Some((_, runs)) = build_inline_runs(items, &font_style, context, layout)? else {
      return Ok(());
    };

    text_line_atoms(&runs, layout, y, atoms);
    Ok(())
  }
}

/// The inline items an emitted box lays out: the flattened subtree for an
/// inline formatting context, the lone run for a text node, nothing otherwise.
fn node_inline_items(node: &RenderNode) -> Option<Vec<InlineItem<'_>>> {
  if node.should_create_inline_layout() {
    return Some(collect_inline_items(node));
  }
  if let Some(NodeKind::Text(text)) = node.node.as_ref().map(|n| &n.kind) {
    return Some(single_text_items(text, &node.context));
  }
  None
}

/// One atom per text line: each run's ascent-to-descent band.
fn text_line_atoms(runs: &InlineRunLayout, layout: Layout, y: f32, atoms: &mut Vec<Atom>) {
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
}

/// The inline item list for a lone text node.
fn single_text_items<'c>(text: &'c TextData, context: &'c RenderContext) -> Vec<InlineItem<'c>> {
  vec![InlineItem::Text {
    text: text.text.as_str().into(),
    context,
    link: None,
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
    shape_cacheable: true,
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

/// Whether the node draws own content (text or an image), i.e. whether a
/// tagged content sequence around it would be non-empty.
fn has_own_content(node: &RenderNode) -> bool {
  if node.should_create_inline_layout() {
    return true;
  }
  if node.has_anonymous_text_item_child() {
    return false;
  }
  match node.node.as_ref().map(|n| &n.kind) {
    Some(NodeKind::Text(_)) => true,
    #[cfg(feature = "images")]
    Some(NodeKind::Image(_)) => true,
    _ => false,
  }
}

/// Whether the node is an image explicitly marked decorative (`alt=""`), so
/// its content is emitted as an artifact instead of a `Figure` element.
fn decorative_image(node: &RenderNode) -> bool {
  node.node.as_ref().is_some_and(|source| {
    source.tag_name().is_some_and(|name| name == "img") && source.alt() == Some("")
  })
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

  fn location(&self) -> Option<crate::krilla::surface::Location> {
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
