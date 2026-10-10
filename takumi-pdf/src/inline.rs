//! Text boxes' inline layout, read from the fragment items layout left on each
//! box: atom collection reads their line boxes, and page emission keeps a box's
//! resolved runs while consecutive pages show it.

use std::{cell::RefCell, collections::HashMap, mem::replace, rc::Rc};

use takumi_core::{
  font_style::SizedFontStyle,
  geometry::{ComputedLayout as Layout, NodeId},
  layout::{
    inline::{FragmentItems, VisualInlineBox},
    tree::RenderNode,
  },
  painter::{OwnContent, PaintedInline},
  scene::NodePaint,
};

use crate::{options::PdfError, pagination::Atom, tree::PreparedTree};

/// A text box's resolved runs, kept for the pages that show it.
struct PreparedInline<'c> {
  painted: PaintedInline<'c>,
  font_style: &'c SizedFontStyle<'c>,
}

/// The text boxes' resolved runs, resolved on first use and dropped once a page
/// goes by without using them.
pub(crate) struct InlineCache<'c> {
  boxes: HashMap<NodeId, &'c TextBox<'c>>,
  /// Each built layout, and whether the page being emitted used it.
  built: RefCell<HashMap<NodeId, (Rc<PreparedInline<'c>>, bool)>>,
}

/// A text-bearing box of a prepared tree, with the resolved font style its
/// inline layout borrows.
pub(crate) struct TextBox<'t> {
  node: &'t RenderNode,
  node_id: NodeId,
  layout: Layout,
  font_style: SizedFontStyle<'t>,
}

impl<'t> TextBox<'t> {
  pub(crate) fn collect(tree: &'t PreparedTree) -> Vec<Self> {
    let mut boxes = Vec::new();

    tree.for_each_paint(|paint| Self::collect_paint(tree, paint, &mut boxes));
    boxes
  }

  fn collect_paint(tree: &'t PreparedTree, paint: &NodePaint, boxes: &mut Vec<Self>) {
    let Some(node) = tree.scene.root.node_at_path(&paint.path) else {
      return;
    };
    let Ok(layout) = tree.scene.results.layout(paint.node_id) else {
      return;
    };
    if matches!(OwnContent::of(node), OwnContent::Inline(_)) {
      boxes.push(Self {
        node,
        node_id: paint.node_id,
        layout,
        font_style: SizedFontStyle::from_style(&node.context.style, &node.context),
      });
    }
  }
}

impl<'c> InlineCache<'c> {
  pub(crate) fn new(boxes: &'c [TextBox<'c>]) -> Self {
    Self {
      boxes: boxes
        .iter()
        .map(|text_box| (text_box.node_id, text_box))
        .collect(),
      built: RefCell::default(),
    }
  }

  /// The layout of the text box at `node_id`, built now if no page kept it.
  fn get(&self, node_id: NodeId) -> Result<Option<Rc<PreparedInline<'c>>>, PdfError> {
    if let Some((prepared, used)) = self.built.borrow_mut().get_mut(&node_id) {
      *used = true;
      return Ok(Some(Rc::clone(prepared)));
    }
    let Some(text_box) = self.boxes.get(&node_id) else {
      return Ok(None);
    };
    let Some(painted) =
      OwnContent::of(text_box.node).lay_out_inline(&text_box.font_style, text_box.layout)
    else {
      return Ok(None);
    };
    let prepared = Rc::new(PreparedInline {
      painted: painted.map_err(PdfError::Font)?,
      font_style: &text_box.font_style,
    });

    self
      .built
      .borrow_mut()
      .insert(node_id, (Rc::clone(&prepared), true));
    Ok(Some(prepared))
  }

  /// Drops the layouts the page just emitted did not use.
  pub(crate) fn end_page(&self) {
    self
      .built
      .borrow_mut()
      .retain(|_, (_, used)| replace(used, false));
  }
}

/// Visits a text box's inline layout: the one `cache` holds, or one laid out
/// now. `None` when the box lays out no runs.
pub(crate) fn visit_inline_layout<R>(
  cache: Option<&InlineCache<'_>>,
  node: &RenderNode,
  node_id: NodeId,
  layout: Layout,
  visit: impl FnOnce(&PaintedInline<'_>, &SizedFontStyle<'_>) -> R,
) -> Result<Option<R>, PdfError> {
  if let Some(prepared) = cache.map(|cache| cache.get(node_id)).transpose()?.flatten() {
    return Ok(Some(visit(&prepared.painted, prepared.font_style)));
  }
  let font_style = SizedFontStyle::from_style(&node.context.style, &node.context);

  OwnContent::of(node)
    .lay_out_inline(&font_style, layout)
    .map(|painted| Ok(visit(&painted.map_err(PdfError::Font)?, &font_style)))
    .transpose()
}

/// A text box's fragment items, without resolving its glyphs, which only painting needs. `None`
/// when the box lays out no runs.
pub(crate) fn fragment_items(node: &RenderNode, layout: Layout) -> Option<Rc<FragmentItems>> {
  let font_style = SizedFontStyle::from_style(&node.context.style, &node.context);

  OwnContent::of(node).fragment_items(&font_style, layout)
}

/// One atom per line box: the lines stack edge to edge, so a cut between two
/// of them straddles neither.
pub(crate) fn text_line_atoms(
  items: &FragmentItems,
  layout: Layout,
  y: f32,
  atoms: &mut Vec<Atom>,
) {
  let content_y = y + layout.content_box_offset().y;

  atoms.extend(
    items
      .line_boxes()
      .iter()
      .map(|(top, bottom)| (content_y + top, content_y + bottom)),
  );
}

/// Atomic vertical bands occupied by inline boxes: in-flow ones and floats
/// alike, so a page cut slices through neither.
pub(crate) fn inline_box_atoms(
  inline_boxes: &[VisualInlineBox],
  layout: Layout,
  y: f32,
  atoms: &mut Vec<Atom>,
) {
  let content_y = layout.content_box_offset().y;

  for inline_box in inline_boxes {
    let top = y + content_y + inline_box.y;

    atoms.push((top, top + inline_box.height));
  }
}
