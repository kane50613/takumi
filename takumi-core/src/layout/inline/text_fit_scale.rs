//! Blink's [`TextFitScale`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/text_fit_scale.h).

use std::{
  fmt::{self, Debug, Formatter},
  rc::Rc,
};

use crate::fonts::font::Font;

/// Blink's `TextFitScale`: how `text-fit` scales a text item.
#[derive(Clone)]
pub(crate) struct TextFitScale {
  pub(crate) scale: f32,
  /// The font the item reshaped with, for a `letter-spacing` or `word-spacing` the scale must not
  /// grow.
  pub(crate) font: Option<Rc<Font>>,
}

impl Debug for TextFitScale {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("TextFitScale")
      .field("scale", &self.scale)
      .field(
        "font_size",
        &self
          .font
          .as_ref()
          .map(|font| font.description.fallback.computed_size),
      )
      .finish()
  }
}

impl Default for TextFitScale {
  fn default() -> Self {
    Self {
      scale: 1.0,
      font: None,
    }
  }
}

/// Blink's `TextFitBlockScale`: the scale a box's block metrics take from its first text.
#[derive(Clone)]
pub(crate) struct TextFitBlockScale {
  pub(crate) paint_scale: f32,
  pub(crate) scaled_font: Option<Rc<Font>>,
}

impl Default for TextFitBlockScale {
  fn default() -> Self {
    Self {
      paint_scale: 1.0,
      scaled_font: None,
    }
  }
}

impl TextFitBlockScale {
  /// Blink's `TotalScale`: the paint scale, times the scaled font's size against `original_font`'s.
  pub(crate) fn total_scale(&self, original_font: &Font) -> f32 {
    let Some(scaled_font) = &self.scaled_font else {
      return self.paint_scale;
    };
    let original_font_size = original_font.description.fallback.computed_size;

    if original_font_size > 0.0 {
      return scaled_font.description.fallback.computed_size / original_font_size
        * self.paint_scale;
    }

    self.paint_scale
  }
}
