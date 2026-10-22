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
