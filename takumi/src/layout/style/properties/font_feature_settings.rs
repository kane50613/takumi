use cssparser::Parser;
use parley::FontFeature;
use smallvec::SmallVec;

use crate::layout::style::{FromCss, ParseResult};

/// Controls OpenType font features via CSS font-feature-settings property.
///
/// This allows enabling/disabling specific typographic features in OpenType fonts
/// such as ligatures, kerning, small caps, and other advanced typography features.
pub type FontFeatureSettings = SmallVec<[FontFeature; 4]>;

impl<'i> FromCss<'i> for FontFeatureSettings {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(FontFeature::parse_list(input.current_line()).collect::<SmallVec<[FontFeature; 4]>>())
  }
}
