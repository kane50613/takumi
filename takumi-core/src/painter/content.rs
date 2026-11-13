//! What a node paints inside its box, besides its children.

use std::rc::Rc;

use super::{BoxFrame, GlyphDevice, InlineLines, LineItem};
use crate::{
  context::RenderContext,
  font_style::SizedFontStyle,
  geometry::{ComputedLayout, Point},
  layout::{
    inline::{FragmentItems, InlineRunLayout, ProcessedInlineSpan},
    node::{ImageData, NodeKind},
    tree::RenderNode,
  },
  resources::font::FontError,
  scene::{InlineLayer, NodePaint},
  style::Affine,
};

/// A block's inline content laid out in its content box, and the part of it one box paints: the
/// block's own, or an inline layer's.
pub struct PaintedInline<'c> {
  /// The spans its items process into.
  pub spans: Vec<ProcessedInlineSpan<'c>>,
  /// Its runs, inline boxes and span fragments, resolved for painting.
  pub runs: InlineRunLayout<'c>,
  items: Rc<FragmentItems>,
  /// The block's layout, which the content lays out in.
  layout: ComputedLayout,
  /// The inline layer the box paints, none for the block itself.
  layer: Option<usize>,
  /// Where the block's border box sits in the box that paints the content.
  origin: Point<f32>,
}

impl PaintedInline<'_> {
  /// The text it laid out.
  pub fn text(&self) -> &str {
    &self.items.text
  }

  /// The block the content lays out in, its border box placed for the painting box at `origin`.
  pub fn frame(&self, origin: Point<f32>) -> BoxFrame {
    BoxFrame::new(self.layout, origin + self.origin)
  }

  /// The transform placing the block's border box, given the painting box's `transform`, and the
  /// frame of the block at that origin.
  pub fn block_placement(&self, transform: Affine) -> (Affine, BoxFrame) {
    (
      transform * Affine::translation(self.origin.x, self.origin.y),
      BoxFrame::new(self.layout, Point::ZERO),
    )
  }

  /// The baseline of each line of the block, top first, which numbers its lines.
  pub fn line_baselines(&self) -> Vec<f32> {
    self.items.run_baselines()
  }

  /// The lines whose items `keep` accepts, holding what the box paints.
  pub fn lines(&self, keep: impl Fn(LineItem) -> bool) -> InlineLines<'_> {
    self
      .runs
      .lines(self.layout, keep)
      .in_layer(&self.spans, self.layer)
  }

  /// Paints what the box paints of the content, the box at `origin`, in the block's `style`.
  pub fn paint(&self, style: &SizedFontStyle, origin: Point<f32>, device: &mut dyn GlyphDevice) {
    self
      .lines(|_| true)
      .paint(&self.spans, style, self.frame(origin), device);
  }
}

/// What a node paints inside its box decorations.
#[derive(Clone, Copy)]
pub enum OwnContent<'n> {
  /// Lines of inline content: the node's inline formatting context, or a text node's own text.
  Inline(&'n RenderNode),
  /// The part of the inline content of the block `0` a `display: inline` element painting as a
  /// layer of its own paints.
  InlineLayer(&'n RenderNode, InlineLayer),
  /// A replaced image.
  Image(&'n ImageData),
  /// Nothing, including a box whose text an anonymous child box lays out.
  None,
}

impl<'n> OwnContent<'n> {
  /// What `node` paints inside its box.
  pub fn of(node: &'n RenderNode) -> Self {
    if node.should_create_inline_layout() {
      return Self::Inline(node);
    }

    match node.node.as_ref().map(|input| &input.kind) {
      Some(NodeKind::Text(_) | NodeKind::Image(_)) if node.has_anonymous_text_item_child() => {
        Self::None
      }
      Some(NodeKind::Text(_)) => Self::Inline(node),
      Some(NodeKind::Image(image)) if node.context.style.is_visible() => Self::Image(image),
      _ => Self::None,
    }
  }

  /// What the box `paint` paints inside its box decorations, its node being `node` under `root`.
  pub fn of_paint(root: &'n RenderNode, node: &'n RenderNode, paint: &NodePaint) -> Self {
    let Some(layer) = paint.inline_layer else {
      return Self::of(node);
    };

    root
      .node_at_path(&paint.path[..paint.path.len() - layer.span_depth])
      .map_or(Self::None, |block| Self::InlineLayer(block, layer))
  }

  /// The style its inline content lays out in: the node's, its context being `context`, or the
  /// block's for an inline layer.
  pub fn font_style<'c>(&self, context: &'c RenderContext) -> SizedFontStyle<'c>
  where
    'n: 'c,
  {
    let context = match *self {
      Self::InlineLayer(block, _) => &block.context,
      _ => context,
    };

    SizedFontStyle::from_style(&context.style, context)
  }

  /// Whether the inline content lays anything out in the node's own `font_style`.
  pub(crate) fn has_inline_content(&self, font_style: &SizedFontStyle) -> bool {
    let node = match *self {
      Self::Inline(node) => node,
      Self::InlineLayer(..) => return true,
      Self::Image(_) | Self::None => return false,
    };

    node.should_create_inline_layout()
      || matches!(
        node.node.as_ref().map(|input| &input.kind),
        Some(NodeKind::Text(_)) if font_style.sizing.font_size != 0.0
      )
  }

  /// The fragment items the inline content lays out to in the content box of `layout`, styled
  /// by `font_style`, its node's own, without resolving a glyph; `None` when it has none. An
  /// inline layer's are its block's.
  pub fn fragment_items(
    &self,
    font_style: &SizedFontStyle<'_>,
    layout: ComputedLayout,
  ) -> Option<Rc<FragmentItems>> {
    let (node, layout) = self.block(layout)?;

    if !self.has_inline_content(font_style) {
      return None;
    }

    node.fragment_items_in(layout)
  }

  /// The block whose inline content it paints, and the block's layout given the box's `layout`.
  fn block(&self, layout: ComputedLayout) -> Option<(&'n RenderNode, ComputedLayout)> {
    match *self {
      Self::Inline(node) => Some((node, layout)),
      Self::InlineLayer(block, layer) => Some((block, layer.block)),
      Self::Image(_) | Self::None => None,
    }
  }

  /// The inline content laid out in `layout`'s content box, `None` when it has none.
  pub fn lay_out_inline<'c>(
    &self,
    font_style: &'c SizedFontStyle<'c>,
    layout: ComputedLayout,
  ) -> Option<Result<PaintedInline<'c>, FontError>>
  where
    'n: 'c,
  {
    let (node, block_layout) = self.block(layout)?;
    let items = self.fragment_items(font_style, layout)?;
    let spans = node.inline_spans(&items);
    let (layer, origin) = match self {
      Self::InlineLayer(_, layer) => (
        Some(layer.span),
        Point::new(-layer.layout.location.x, -layer.layout.location.y),
      ),
      _ => (None, Point::ZERO),
    };

    Some(
      items
        .resolve_runs(&spans, &node.context, block_layout, layer)
        .map(|runs| PaintedInline {
          spans: spans.spans,
          runs,
          items,
          layout: block_layout,
          layer,
          origin,
        }),
    )
  }
}
