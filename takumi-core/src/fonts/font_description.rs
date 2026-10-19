//! What shaping reads of a font's style, named for Blink's `FontDescription`.

use std::rc::Rc;

use harfrust::{Feature, Language, Variation};
use parley::fontique::Attributes;

use crate::font_style::ExpandedFontFamily;

/// The style a font is matched and shaped with.
#[derive(Clone)]
pub(crate) struct FontDescription {
  /// The `font-family` list.
  pub(crate) family: Rc<ExpandedFontFamily>,
  /// `font-weight`, `font-style`, and `font-stretch` as font matching takes them.
  pub(crate) attributes: Attributes,
  /// Blink's `ComputedSize`.
  pub(crate) computed_size: f32,
  /// Blink's `SpecifiedSize`.
  pub(crate) specified_size: f32,
  /// Blink's `FontFeatures`.
  pub(crate) features: Vec<Feature>,
  /// `font-variation-settings`.
  pub(crate) variation_settings: Vec<Variation>,
  /// Blink's `LocaleOrDefault` as a HarfBuzz language.
  pub(crate) locale: Option<Language>,
}
