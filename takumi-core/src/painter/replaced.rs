//! Replaced content placed in its box, after Blink's `ReplacedPainter`.

use super::BoxPainter;
use crate::{
  geometry::{Rect, Size},
  layout::{
    decoration::{ClipBox, ContourOrigin},
    replaced::ReplacedPlacement,
  },
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
  /// `object-position`, snapped to pixels as Blink's `ImagePainter::PaintIntoRect` draws it.
  pub fn replaced_content(&self, intrinsic: Size<f32>) -> ReplacedContent {
    let layout = self.layout;
    let content_box = ClipBox::content_box(*self.border(), layout);
    let unsnapped = ReplacedPlacement::new(self.context, content_box.size, intrinsic);
    let content_offset = layout.content_box_offset();
    let snapped_offset = self.snapped.offset();
    let (offset, size) = self
      .snapped
      .snap(content_offset + unsnapped.offset, unsnapped.size);
    let placement = ReplacedPlacement {
      offset: offset + snapped_offset - content_offset,
      size,
    };
    let insets = Rect {
      top: layout.border.top + layout.padding.top,
      right: layout.border.right + layout.padding.right,
      bottom: layout.border.bottom + layout.padding.bottom,
      left: layout.border.left + layout.padding.left,
    };
    let (clip_offset, clip_size) = if content_box.border.is_zero() {
      self.snapped.inset(insets)
    } else {
      self.snapped.contoured_inset(insets, false)
    };
    let clip =
      (!content_box.border.is_zero() || unsnapped.overflows(content_box.size)).then(|| ClipBox {
        offset: clip_offset + snapped_offset,
        size: clip_size,
        origin: content_box.origin.map(|origin| ContourOrigin {
          size: self.snapped.size(),
          offset: snapped_offset,
          ..origin
        }),
        ..content_box
      });

    ReplacedContent { placement, clip }
  }
}
