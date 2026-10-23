//! Text boxes' inline layout, read from the fragment items layout left on each
//! box: page emission keeps a box's resolved runs while consecutive pages show
//! it.

use std::{cell::RefCell, collections::HashMap, mem::replace, rc::Rc};

use takumi_core::{
  font_style::SizedFontStyle,
  geometry::{ComputedLayout as Layout, NodeId},
  layout::tree::RenderNode,
  painter::{OwnContent, PaintedInline},
  scene::NodePaint,
};

use crate::{options::PdfError, tree::PreparedTree};

/// A box and which of its fragments.
type FragmentKey = (NodeId, usize);

/// A text box's resolved runs, kept for the pages that show it.
struct PreparedInline<'c> {
  painted: PaintedInline<'c>,
  font_style: &'c SizedFontStyle<'c>,
}

/// The text boxes' resolved runs, resolved on first use and dropped once a page
/// goes by without using them.
pub(crate) struct InlineCache<'c> {
  boxes: HashMap<FragmentKey, &'c TextBox<'c>>,
  /// Each built layout, and whether the page being emitted used it.
  built: RefCell<HashMap<FragmentKey, (Rc<PreparedInline<'c>>, bool)>>,
}

/// A text-bearing box's fragment in a prepared tree, with the resolved font style its inline layout
/// borrows.
pub(crate) struct TextBox<'t> {
  node: &'t RenderNode,
  key: FragmentKey,
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
    let Ok(layout) = tree.scene.enter(paint) else {
      return;
    };
    if matches!(OwnContent::of(node), OwnContent::Inline(_)) {
      boxes.push(Self {
        node,
        key: (paint.node_id, paint.fragment_index),
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
        .map(|text_box| (text_box.key, text_box))
        .collect(),
      built: RefCell::default(),
    }
  }

  /// The layout of the fragment `key` names of a text box, built now if no page kept it.
  fn get(&self, key: FragmentKey) -> Result<Option<Rc<PreparedInline<'c>>>, PdfError> {
    if let Some((prepared, used)) = self.built.borrow_mut().get_mut(&key) {
      *used = true;
      return Ok(Some(Rc::clone(prepared)));
    }
    let Some(text_box) = self.boxes.get(&key) else {
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
      .insert(key, (Rc::clone(&prepared), true));
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
  let key = (node_id, node.context.fragment_index());

  if let Some(prepared) = cache.map(|cache| cache.get(key)).transpose()?.flatten() {
    return Ok(Some(visit(&prepared.painted, prepared.font_style)));
  }
  let font_style = SizedFontStyle::from_style(&node.context.style, &node.context);

  OwnContent::of(node)
    .lay_out_inline(&font_style, layout)
    .map(|painted| Ok(visit(&painted.map_err(PdfError::Font)?, &font_style)))
    .transpose()
}
