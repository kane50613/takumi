//! Unsplittable vertical extents, paragraphs and content boxes collected from
//! the laid-out scene, which pagination cuts around.

use std::{mem::take, ops::Range};

use takumi_core::{
  geometry::{ComputedLayout as Layout, NodeId},
  layout::tree::RenderNode,
  painter::{BoxPainter, OwnContent},
  scene::{NodePaint, PaintItemKind, Scene},
  style::{Affine, BreakBetween, BreakInside},
};

use crate::{
  inline::{inline_box_atoms, text_line_atoms, visit_inline_layout},
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
  /// Boxes that show on the page; spacing between them does not.
  pub(crate) content: Vec<Atom>,
  /// Text boxes with their `widows` / `orphans` minimums.
  pub(crate) paragraphs: Vec<Paragraph>,
  /// Boxes with block-start or block-end border and padding.
  pub(crate) decorated: Vec<Decorated>,
}

/// A box's border-box edges around its content box's, in content coordinates.
pub(crate) struct Decorated {
  top: f32,
  content: Atom,
  bottom: f32,
}

impl Atoms {
  /// Binds each box's block-start border and padding to the first atom inside
  /// it, and its block-end ones to the last. Blink takes a break before the
  /// first child as a break before the box, and one before block-end border
  /// and padding as a last resort (`fragmentation_utils.cc`). Naive: the
  /// atoms are matched by height alone, so a box can bind one beside it.
  fn bind_decorations(&mut self) {
    let mut by_top = self.extents.clone();
    let mut by_bottom = self.extents.clone();

    by_top.sort_by(|a, b| a.0.total_cmp(&b.0));
    by_bottom.sort_by(|a, b| a.1.total_cmp(&b.1));

    for decorated in take(&mut self.decorated) {
      let (content_top, content_bottom) = decorated.content;

      if content_top > decorated.top {
        let first = by_top.partition_point(|atom| atom.0 < content_top - 0.5);

        if let Some(atom) = by_top.get(first).filter(|atom| atom.0 < content_bottom) {
          self.extents.push((decorated.top, atom.1));
        }
      }
      if content_bottom < decorated.bottom {
        let past = by_bottom.partition_point(|atom| atom.1 <= content_bottom + 0.5);

        if let Some(atom) = past
          .checked_sub(1)
          .map(|last| by_bottom[last])
          .filter(|atom| atom.1 > content_top)
        {
          self.extents.push((atom.0, decorated.bottom));
        }
      }
    }
  }

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
}

impl AtomCollector<'_> {
  pub(crate) fn collect(&self) -> Result<Atoms, PdfError> {
    let mut atoms = Atoms::default();

    self.context_atoms(0, Affine::IDENTITY, &mut atoms)?;
    atoms.bind_decorations();
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
          // The node's own item already records its inline content, floats included.
          PaintItemKind::Floats(_) => {}
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
    let content = (
      y + layout.border.top + layout.padding.top,
      extent.1 - layout.border.bottom - layout.padding.bottom,
    );

    if content.0 > y || content.1 < extent.1 {
      atoms.decorated.push(Decorated {
        top: y,
        content,
        bottom: extent.1,
      });
    }

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
      || (node.paints_own_box() && BoxPainter::new(&node.context, layout).paints_decorations());

    match OwnContent::of(node) {
      OwnContent::Inline(_) => {
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

  /// One atom per line box.
  /// The lines also form one [`Paragraph`] for the widow/orphan solver.
  fn text_atoms(
    &self,
    node: &RenderNode,
    node_id: NodeId,
    layout: Layout,
    y: f32,
    atoms: &mut Atoms,
  ) -> Result<(), PdfError> {
    visit_inline_layout(None, node, node_id, layout, |built, runs, _| {
      let start = atoms.extents.len();

      text_line_atoms(built, layout, y, &mut atoms.extents);
      // Box bands are indivisible but not text lines, so widow/orphan control
      // does not count them.
      let paragraph_end = atoms.extents.len();

      inline_box_atoms(runs, layout, y, &mut atoms.extents);
      atoms.push_paragraph(node, start..paragraph_end);
    })?;
    Ok(())
  }
}
