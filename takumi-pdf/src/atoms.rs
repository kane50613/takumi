//! Unsplittable vertical extents and paragraphs collected from the laid-out
//! scene, which pagination cuts around.

use takumi_core::{
  font_style::SizedFontStyle,
  geometry::ComputedLayout as Layout,
  layout::{
    node::NodeKind,
    tree::{LayoutResults, RenderNode},
  },
  scene::{NodePaint, PaintItemKind, StackingContextNode},
  style::{Affine, BreakBetween, BreakInside},
};

use crate::{
  inline::{
    InlineMap, build_inline_runs, inline_box_atoms, inline_key, node_inline_items, text_line_atoms,
  },
  options::PdfError,
  pagination::{Atom, Paragraph},
};

/// Walks the scene like the emitter, recording unsplittable vertical extents
/// instead of painting.
pub(crate) struct AtomCollector<'a> {
  pub(crate) root: &'a RenderNode,
  pub(crate) contexts: &'a [StackingContextNode],
  pub(crate) results: &'a LayoutResults,
  pub(crate) inline: Option<&'a InlineMap<'a>>,
}

impl AtomCollector<'_> {
  pub(crate) fn collect(
    &self,
    atoms: &mut Vec<Atom>,
    forced: &mut Vec<f32>,
    paragraphs: &mut Vec<Paragraph>,
  ) -> Result<(), PdfError> {
    self.context_atoms(0, Affine::IDENTITY, atoms, forced, paragraphs)
  }
}

/// Records the box's lines as a [`Paragraph`] for the widow/orphan solver.
fn push_paragraph(node: &RenderNode, lines: &[Atom], paragraphs: &mut Vec<Paragraph>) {
  let style = &node.context.style;
  let before = style.orphans.get();
  let after = style.widows.get();

  if lines.len() < 2 || (before <= 1 && after <= 1) {
    return;
  }
  let mut lines = lines.to_vec();

  lines.sort_by(|a, b| a.0.total_cmp(&b.0));
  paragraphs.push(Paragraph {
    lines,
    before,
    after,
  });
}

impl AtomCollector<'_> {
  fn context_atoms(
    &self,
    id: usize,
    parent: Affine,
    atoms: &mut Vec<Atom>,
    forced: &mut Vec<f32>,
    paragraphs: &mut Vec<Paragraph>,
  ) -> Result<(), PdfError> {
    let Some(context) = self.contexts.get(id) else {
      return Ok(());
    };

    let child_frame = match context.root() {
      Some(paint) => self.box_atoms(paint, parent, atoms, forced, paragraphs)?,
      None => parent,
    };

    for bucket in context.in_paint_order() {
      for item in bucket {
        match &item.kind {
          PaintItemKind::Node(paint) => {
            self.box_atoms(paint, child_frame, atoms, forced, paragraphs)?;
          }
          PaintItemKind::Context(child) => {
            self.context_atoms(*child, child_frame, atoms, forced, paragraphs)?;
          }
        }
      }
    }
    Ok(())
  }

  /// Records one node's atoms and returns the frame its children sit in. A
  /// node painted under a non-translation transform becomes a single atom
  /// spanning its device bounds — windowing through a rotation would distort.
  fn box_atoms(
    &self,
    paint: &NodePaint,
    parent: Affine,
    atoms: &mut Vec<Atom>,
    forced: &mut Vec<f32>,
    paragraphs: &mut Vec<Paragraph>,
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
      self.text_atoms(node, layout, y, atoms, paragraphs)?;
    } else if !node.has_anonymous_text_item_child() {
      match node.node.as_ref().map(|n| &n.kind) {
        Some(NodeKind::Text(_)) => {
          self.text_atoms(node, layout, y, atoms, paragraphs)?;
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
  /// The lines also form one [`Paragraph`] for the widow/orphan solver.
  fn text_atoms(
    &self,
    node: &RenderNode,
    layout: Layout,
    y: f32,
    atoms: &mut Vec<Atom>,
    paragraphs: &mut Vec<Paragraph>,
  ) -> Result<(), PdfError> {
    let start = atoms.len();
    let owned_runs;
    let runs = match self.inline.and_then(|map| map.get(&inline_key(node))) {
      Some(prepared) => &prepared.runs,
      None => {
        let context = &node.context;
        let Some(items) = node_inline_items(node) else {
          return Ok(());
        };
        let font_style = SizedFontStyle::from_style(&context.style, context);
        let Some((_, runs)) = build_inline_runs(items, &font_style, context, layout)? else {
          return Ok(());
        };

        owned_runs = runs;
        &owned_runs
      }
    };

    text_line_atoms(runs, layout, y, atoms);
    // Box bands are indivisible but not text lines, so widow/orphan control
    // does not count them.
    let paragraph_end = atoms.len();
    inline_box_atoms(runs, layout, y, atoms);
    push_paragraph(node, &atoms[start..paragraph_end], paragraphs);
    Ok(())
  }
}
