//! Inline layout prepared once per render and shared by atom collection and page emission.

use std::collections::HashMap;

use takumi_core::{
  context::RenderContext,
  font_style::SizedFontStyle,
  geometry::ComputedLayout as Layout,
  layout::{
    inline::{
      BuiltInlineLayout, InlineItem, InlineLayoutMode, InlineLayoutRequest, InlineRunLayout,
      collect_inline_items, create_inline_layout, resolve_inline_runs,
    },
    node::{NodeKind, TextData},
    tree::RenderNode,
  },
  scene::{NodePaint, PaintItemKind},
};

use crate::{options::PdfError, pagination::Atom, tree::PreparedTree};

/// A text box's inline layout, built once per render and reused by atom
/// collection and every page's emission.
pub(crate) struct PreparedInline<'c> {
  pub(crate) built: BuiltInlineLayout<'c>,
  pub(crate) runs: InlineRunLayout,
}

/// Inline layouts keyed by the source [`RenderNode`]'s address.
pub(crate) type InlineMap<'c> = HashMap<usize, PreparedInline<'c>>;

pub(crate) fn inline_key(node: &RenderNode) -> usize {
  std::ptr::from_ref(node) as usize
}

/// The text-bearing boxes of a prepared tree, with the resolved font style
/// each inline layout borrows.
pub(crate) fn collect_text_boxes<'t>(tree: &'t PreparedTree) -> Vec<TextBox<'t>> {
  let mut boxes = Vec::new();

  collect_text_boxes_context(tree, 0, &mut boxes);
  boxes
}

pub(crate) struct TextBox<'t> {
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
pub(crate) fn build_inline_map<'c>(boxes: &'c [TextBox<'c>]) -> Result<InlineMap<'c>, PdfError> {
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

/// The inline items an emitted box lays out: the flattened subtree for an
/// inline formatting context, the lone run for a text node, nothing otherwise.
pub(crate) fn node_inline_items(node: &RenderNode) -> Option<Vec<InlineItem<'_>>> {
  if node.should_create_inline_layout() {
    return Some(collect_inline_items(node));
  }
  if let Some(NodeKind::Text(text)) = node.node.as_ref().map(|n| &n.kind) {
    return Some(single_text_items(text, &node.context));
  }
  None
}

/// One atom per text line: each run's ascent-to-descent band.
pub(crate) fn text_line_atoms(
  runs: &InlineRunLayout,
  layout: Layout,
  y: f32,
  atoms: &mut Vec<Atom>,
) {
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

/// Atomic vertical bands occupied by inline boxes: in-flow ones and floats
/// alike, so a page cut slices through neither.
pub(crate) fn inline_box_atoms(
  runs: &InlineRunLayout,
  layout: Layout,
  y: f32,
  atoms: &mut Vec<Atom>,
) {
  let content_y = layout.content_box_offset().y;

  for inline_box in &runs.inline_boxes {
    let top = y + content_y + inline_box.y;

    atoms.push((top, top + inline_box.height));
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
pub(crate) fn build_inline_runs<'c>(
  items: Vec<InlineItem<'c>>,
  font_style: &'c SizedFontStyle<'c>,
  context: &'c RenderContext,
  layout: Layout,
) -> Result<Option<(BuiltInlineLayout<'c>, InlineRunLayout)>, PdfError> {
  let content = layout.content_box_size();
  if font_style.sizing.font_size == 0.0 || content.width <= 0.0 || content.height <= 0.0 {
    return Ok(None);
  }

  let built = create_inline_layout(InlineLayoutRequest::in_content_box(
    items,
    content,
    font_style,
    context,
    InlineLayoutMode::Draw,
  ));
  let runs = resolve_inline_runs(&built, context, layout).map_err(PdfError::Font)?;

  Ok(Some((built, runs)))
}
