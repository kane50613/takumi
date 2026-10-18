//! A font description with the fonts it falls back through, named for Blink's `Font`.

use std::rc::Rc;

use super::{
  font_description::FontDescription, font_fallback_list::FontFallbackList,
  simple_font_data::SimpleFontData,
};
use crate::resources::font::Fonts;

/// A font as shaping takes it.
pub(crate) struct Font {
  pub(crate) description: FontDescription,
  pub(crate) fallback_list: FontFallbackList,
}

impl Font {
  pub(crate) fn new(fonts: &mut Fonts, description: FontDescription) -> Self {
    let fallback_list = FontFallbackList::new(fonts, &description);

    Self {
      description,
      fallback_list,
    }
  }

  /// Blink's `PrimaryFont`.
  pub(crate) fn primary_font(&self) -> Option<Rc<SimpleFontData>> {
    self.fallback_list.primary_simple_font_data()
  }
}
