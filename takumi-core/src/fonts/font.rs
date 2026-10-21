//! A font description with the fonts it falls back through, named for Blink's `Font`.

use std::rc::Rc;

use super::{
  font_description::FontDescription, font_fallback_list::FontFallbackList,
  simple_font_data::SimpleFontData,
};
#[cfg(test)]
use crate::resources::font::Fonts;
use crate::resources::font::PrimaryFontMetrics;

/// A font as shaping takes it.
pub(crate) struct Font {
  pub(crate) description: FontDescription,
  pub(crate) fallback_list: Rc<FontFallbackList>,
}

impl Font {
  #[cfg(test)]
  pub(crate) fn new(fonts: &Fonts, description: FontDescription) -> Self {
    let fallback_list = Rc::new(FontFallbackList::new(fonts, &description.fallback));

    Self {
      description,
      fallback_list,
    }
  }

  /// Blink's `PrimaryFont`.
  pub(crate) fn primary_font(&self) -> Option<Rc<SimpleFontData>> {
    self.fallback_list.primary_simple_font_data()
  }

  /// Blink's `PrimaryFont()->GetFontMetrics()`.
  pub(crate) fn primary_font_metrics(&self) -> Option<PrimaryFontMetrics> {
    self
      .primary_font()
      .and_then(|font_data| font_data.font_metrics())
  }

  /// Blink's `PrimaryFontForTabSize`.
  pub(crate) fn primary_font_for_tab_size(&self) -> Option<Rc<SimpleFontData>> {
    self.primary_font()
  }

  /// Blink's `TabWidth` of a tab at `position`, written from css-text-3 §4.2, as Blink's
  /// `Font::TabWidth` is LGPL: the distance to the next tab stop, `tab_size` space advances
  /// apart, skipping a stop closer than half a space. <https://drafts.csswg.org/css-text-3/#tab-size-property>
  pub(crate) fn tab_width(
    &self,
    font_data: Option<&SimpleFontData>,
    tab_size: f32,
    position: Option<f32>,
  ) -> f32 {
    let letter_spacing = self.description.letter_spacing;
    let Some(font_data) = font_data else {
      return letter_spacing;
    };
    let space_width = font_data.space_width();
    let base_tab_width = tab_size * (space_width + letter_spacing + self.description.word_spacing);

    if base_tab_width == 0.0 {
      return letter_spacing;
    }

    let Some(position) = position else {
      return base_tab_width;
    };
    let distance_to_tab_stop = base_tab_width - position.rem_euclid(base_tab_width);

    if distance_to_tab_stop < space_width / 2.0 {
      distance_to_tab_stop + base_tab_width
    } else {
      distance_to_tab_stop
    }
  }
}
