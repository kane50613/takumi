//! Unsplittable vertical extents and paragraphs collected from the laid-out
//! scene, which pagination cuts around.

use std::ops::Range;

use takumi_core::{
  font_style::SizedFontStyle,
  geometry::{ComputedLayout as Layout, NodeId},
  layout::{
    node::{Node, NodeKind},
    tree::{LayoutResults, RenderNode},
  },
  scene::{NodePaint, PaintItemKind, StackingContextNode},
  style::{Affine, BreakBetween, BreakInside},
};

use crate::{
  inline::{InlineMap, build_inline_runs, inline_box_atoms, node_inline_items, text_line_atoms},
  interactive::is_form_control,
  options::PdfError,
  pagination::{Atom, Paragraph},
};

/// What the cut search works around, in content coordinates.
#[derive(Default)]
pub(crate) struct Atoms {
  /// Unsplittable extents: text lines, images, `break-inside: avoid` boxes and
  /// transformed subtrees.
  pub(crate) extents: Vec<Atom>,
  /// Where `break-before` / `break-after: page` force a cut.
  pub(crate) forced: Vec<f32>,
  /// Text boxes with their `widows` / `orphans` minimums.
  pub(crate) paragraphs: Vec<Paragraph>,
}

impl Atoms {
  /// Records the box's lines as a [`Paragraph`] for the widow/orphan solver.
  fn push_paragraph(&mut self, node: &RenderNode, lines: Range<usize>) {
    let style = &node.context.style;
    let before = style.orphans.get();
    let after = style.widows.get();

    if lines.len() < 2 || (before <= 1 && after <= 1) {
      return;
    }
    let mut lines = self.extents[lines].to_vec();

    lines.sort_by(|a, b| a.0.total_cmp(&b.0));
    self.paragraphs.push(Paragraph {
      lines,
      before,
      after,
    });
  }
}

/// Walks the scene like the emitter, recording unsplittable vertical extents
/// instead of painting.
pub(crate) struct AtomCollector<'a> {
  pub(crate) root: &'a RenderNode,
  pub(crate) contexts: &'a [StackingContextNode],
  pub(crate) results: &'a LayoutResults,
  pub(crate) inline: Option<&'a InlineMap<'a>>,
}

impl AtomCollector<'_> {
  pub(crate) fn collect(&self) -> Result<Atoms, PdfError> {
    let mut atoms = Atoms::default();

    self.context_atoms(0, Affine::IDENTITY, &mut atoms)?;
    Ok(atoms)
  }

  fn context_atoms(&self, id: usize, parent: Affine, atoms: &mut Atoms) -> Result<(), PdfError> {
    let Some(context) = self.contexts.get(id) else {
      return Ok(());
    };

    let child_frame = match context.root() {
      Some(paint) => self.box_atoms(paint, parent, atoms)?,
      None => parent,
    };

    for bucket in context.in_paint_order() {
      for item in bucket {
        match &item.kind {
          PaintItemKind::Node(paint) => {
            self.box_atoms(paint, child_frame, atoms)?;
          }
          PaintItemKind::Context(child) => {
            self.context_atoms(*child, child_frame, atoms)?;
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
    atoms: &mut Atoms,
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
        atoms
          .extents
          .push((bounds.top as f32, bounds.bottom as f32));
      }
      return Ok(parent * relative);
    }
    let y = relative.y;
    let style = &node.context.style;

    if style.break_before == BreakBetween::Page {
      atoms.forced.push(y);
    }
    if style.break_after == BreakBetween::Page {
      atoms.forced.push(y + layout.size.height);
    }
    // A widget annotation has one rectangle on one page, so the control it
    // covers cannot straddle a break.
    let control = node
      .node
      .as_ref()
      .and_then(Node::tag_name)
      .is_some_and(is_form_control);

    if style.break_inside == BreakInside::Avoid || control {
      atoms.extents.push((y, y + layout.size.height));
    }

    if node.should_create_inline_layout() {
      self.text_atoms(node, paint.node_id, layout, y, atoms)?;
    } else if !node.has_anonymous_text_item_child() {
      match node.node.as_ref().map(|n| &n.kind) {
        Some(NodeKind::Text(_)) => {
          self.text_atoms(node, paint.node_id, layout, y, atoms)?;
        }
        Some(NodeKind::Image(_)) => {
          atoms.extents.push((y, y + layout.size.height));
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
    node_id: NodeId,
    layout: Layout,
    y: f32,
    atoms: &mut Atoms,
  ) -> Result<(), PdfError> {
    let start = atoms.extents.len();
    let owned_runs;
    let runs = match self.inline.and_then(|map| map.get(&node_id)) {
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

    text_line_atoms(runs, layout, y, &mut atoms.extents);
    // Box bands are indivisible but not text lines, so widow/orphan control
    // does not count them.
    let paragraph_end = atoms.extents.len();
    inline_box_atoms(runs, layout, y, &mut atoms.extents);
    atoms.push_paragraph(node, start..paragraph_end);
    Ok(())
  }
}
