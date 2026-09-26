//! Unsplittable vertical extents, paragraphs and content boxes collected from
//! the laid-out scene, which pagination cuts around.

use std::ops::Range;

use takumi_core::{
  geometry::{ComputedLayout as Layout, NodeId},
  layout::tree::RenderNode,
  painter::BoxPainter,
  scene::{NodePaint, PaintItemKind, Scene},
  style::{Affine, BreakBetween, BreakInside},
};

use crate::{
  inline::{InlineMap, inline_box_atoms, text_line_atoms, visit_inline_layout},
  options::PdfError,
  pagination::{Atom, Paragraph},
  tree::OwnContent,
};

/// What the cut search works around, in content coordinates.
#[derive(Default)]
pub(crate) struct Atoms {
  /// Unsplittable extents: text lines, images, `break-inside: avoid` boxes and
  /// transformed subtrees.
  pub(crate) extents: Vec<Atom>,
  /// Where `break-before` / `break-after: page` force a cut.
  pub(crate) forced: Vec<f32>,
  /// Boxes that show on the page; spacing between them does not.
  pub(crate) content: Vec<Atom>,
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
  pub(crate) scene: &'a Scene,
  pub(crate) inline: Option<&'a InlineMap<'a>>,
}

impl AtomCollector<'_> {
  pub(crate) fn collect(&self) -> Result<Atoms, PdfError> {
    let mut atoms = Atoms::default();

    self.context_atoms(0, Affine::IDENTITY, &mut atoms)?;
    Ok(atoms)
  }

  fn context_atoms(&self, id: usize, parent: Affine, atoms: &mut Atoms) -> Result<(), PdfError> {
    let Some(context) = self.scene.contexts.get(id) else {
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
    let Some(node) = self.scene.root.node_at_path(&paint.path) else {
      return Ok(parent);
    };
    let Ok(layout) = self.scene.results.layout(paint.node_id) else {
      return Ok(parent);
    };

    let relative = parent.invert().unwrap_or(Affine::IDENTITY) * paint.transform;
    if !relative.only_translation() {
      if let Some(bounds) = paint.paint_bounds {
        let extent = (bounds.top as f32, bounds.bottom as f32);

        atoms.extents.push(extent);
        if extent.1 > extent.0 {
          atoms.content.push(extent);
        }
      }
      return Ok(parent * relative);
    }
    let y = relative.y;
    let extent = (y, y + layout.size.height);
    let style = &node.context.style;

    if style.break_before == BreakBetween::Page {
      atoms.forced.push(y);
    }
    if style.break_after == BreakBetween::Page {
      atoms.forced.push(y + layout.size.height);
    }
    if style.break_inside == BreakInside::Avoid {
      atoms.extents.push(extent);
    }
    let mut shows = node
      .children
      .as_deref()
      .is_none_or(<[RenderNode]>::is_empty)
      || BoxPainter::new(&node.context, layout).paints_decorations();

    match OwnContent::of(node) {
      OwnContent::Text => {
        shows = true;
        self.text_atoms(node, paint.node_id, layout, y, atoms)?;
      }
      OwnContent::Image(_) => {
        shows = true;
        atoms.extents.push(extent);
      }
      OwnContent::None => {}
    }
    if shows && extent.1 > extent.0 {
      atoms.content.push(extent);
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
    visit_inline_layout(self.inline, node, node_id, layout, |_, runs, _| {
      let start = atoms.extents.len();

      text_line_atoms(runs, layout, y, &mut atoms.extents);
      // Box bands are indivisible but not text lines, so widow/orphan control
      // does not count them.
      let paragraph_end = atoms.extents.len();

      inline_box_atoms(runs, layout, y, &mut atoms.extents);
      atoms.push_paragraph(node, start..paragraph_end);
    })?;
    Ok(())
  }
}
