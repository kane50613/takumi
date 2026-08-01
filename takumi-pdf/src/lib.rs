#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! Vector PDF output for takumi — proof of concept.
//!
//! [`render`] runs takumi-core layout, walks the same backend-agnostic
//! stacking-context scene as `takumi-svg`, and emits a single-page PDF through
//! [`krilla`]: background rects as filled paths and text as real glyph runs with
//! embedded, subsetted fonts — selectable, searchable, copyable.
//!
//! POC coverage: background-color, text runs (fill color and opacity), nested
//! containers, affine transforms. Everything else (borders, radius, gradients,
//! shadows, images, filters, clipping, text decorations) is out of scope for now.

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
  /// Per-render font fallback chain (family names in order).
  #[builder(default)]
  pub(crate) font_families: Option<FontFamily>,
  /// Default BCP-47 language tag applied to the root.
  #[builder(default)]
  pub(crate) lang: Option<Lang>,
}

/// Renders a node tree to a single-page PDF.
pub fn render(options: PdfOptions<'_>) -> Result<Vec<u8>, PdfError> {
  let viewport = options.viewport;

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
  let page_size = KrillaSize::from_wh(width, height).ok_or(PdfError::InvalidPageSize)?;

  let contexts = build_stacking_contexts(
    &root,
    &results,
    NodeId::ROOT,
    Affine::IDENTITY,
    (Some(width), Some(height)),
  )?;

  let mut document = Document::new();
  let mut page = document.start_page_with(PageSettings::new(page_size));
  let mut surface = page.surface();
  let mut emitter = Emitter {
    root: &root,
    contexts: &contexts,
    results: &results,
    fonts: HashMap::new(),
  };

  emitter.emit_context(0, Affine::IDENTITY, &mut surface)?;

  surface.finish();
  page.finish();
  document.finish().map_err(PdfError::Krilla)
}

/// Scene walker state: the render tree, the stacking-context scene, and a cache
/// of krilla fonts keyed by the backing blob identity.
struct Emitter<'a> {
  root: &'a RenderNode,
  contexts: &'a [StackingContextNode],
  results: &'a LayoutResults,
  fonts: HashMap<(usize, usize, u32), Font>,
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
            self.emit_context(*child, child_frame, surface)?;
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

    self.layout_and_draw_runs(items, context, layout, x, y, surface)
  }

  fn emit_inline_content(
    &mut self,
    node: &RenderNode,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    self.layout_and_draw_runs(
      collect_inline_items(node),
      &node.context,
      layout,
      x,
      y,
      surface,
    )
  }

  fn layout_and_draw_runs(
    &mut self,
    items: Vec<InlineItem<'_>>,
    context: &RenderContext,
    layout: Layout,
    x: f32,
    y: f32,
    surface: &mut Surface,
  ) -> Result<(), PdfError> {
    let font_style = SizedFontStyle::from_style(&context.style, context);
    let content = layout.content_box_size();
    if font_style.sizing.font_size == 0.0 || content.width <= 0.0 || content.height <= 0.0 {
      return Ok(());
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
      mode: InlineLayoutMode::Draw,
    });

    let runs = resolve_inline_runs(&built, context, layout).map_err(PdfError::Font)?;

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
