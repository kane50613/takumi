//! Replaced content placed in its box, after Blink's `ReplacedPainter`.

use super::BoxPainter;
use crate::{
  geometry::Size,
  layout::{decoration::ClipBox, replaced::ReplacedPlacement},
};

/// Replaced content placed in its content box, and what clips it.
#[derive(Debug, Clone, Copy)]
pub struct ReplacedContent {
  /// Where the content draws and how large, relative to the content box.
  pub placement: ReplacedPlacement,
  /// The content box's curve when the box is rounded, its rectangle when the content reaches past
  /// it, and `None` when nothing needs clipping.
  pub clip: Option<ClipBox>,
}

impl BoxPainter<'_> {
  /// Places replaced content of `intrinsic` size in the box by `object-fit` and
  /// `object-position`.
  pub fn replaced_content(&self, intrinsic: Size<f32>) -> ReplacedContent {
    let content_box = ClipBox::content_box(*self.border(), self.layout);
    let placement = ReplacedPlacement::new(self.context, content_box.size, intrinsic);
    let clip = if !content_box.border.is_zero() || placement.overflows(content_box.size) {
      Some(content_box)
    } else {
      None
    };

    ReplacedContent { placement, clip }
  }
}
