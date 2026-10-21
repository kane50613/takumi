//! Where an inline formatting context's boxes paint.

/// How an inline box sits in its line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InlineBoxKind {
  /// An atomic inline on the line.
  InFlow,
  /// An absolutely positioned box, at its static position.
  OutOfFlow,
  /// A float beside the lines.
  Float,
}

#[derive(Clone, Copy, Debug)]
/// An inline box resolved to its painted position and size.
pub struct VisualInlineBox {
  /// Index into the span list.
  pub id: u64,
  /// Left edge.
  pub x: f32,
  /// Top edge.
  pub y: f32,
  /// Box width.
  pub width: f32,
  /// Box height.
  pub height: f32,
  /// Baseline of the in-flow line that owns this box, relative to the inline formatting context's
  /// content-box top edge.
  pub line_baseline: Option<f32>,
  /// How the box sits in its line.
  pub kind: InlineBoxKind,
}

/// Which draws of an inline formatting context one pass paints: CSS 2.1 Appendix E paints the
/// floats in a phase of their own, before the line content, and out-of-flow boxes with their
/// stacking context rather than here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlinePass {
  /// The text and every box that does not float.
  Content,
  /// The floats alone.
  Floats,
}

impl InlinePass {
  /// Whether the pass paints `inline_box`.
  pub fn paints(self, inline_box: &VisualInlineBox) -> bool {
    match inline_box.kind {
      InlineBoxKind::InFlow => self == Self::Content,
      InlineBoxKind::Float => self == Self::Floats,
      InlineBoxKind::OutOfFlow => false,
    }
  }
}
