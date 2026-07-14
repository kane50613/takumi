use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case};

#[cfg(feature = "svg")]
use crate::style::properties::filter_reference::FilterReference;
use crate::style::{
  Angle, Animatable, Color, CssDescriptorKind, CssExpectedMessage, CssToken, FromCss, Length,
  ListInterpolationStrategy, MakeComputed, ParseResult, PercentageNumber, SizingContext,
  TextShadow, ToCss, tw::TailwindPropertyParser, unexpected_token,
};

macro_rules! interpolate_field {
  ($variant:path, $from:ident, $to:ident, $progress:expr, $sizing:expr, $cc:expr) => {{
    let mut value = $from;
    value.interpolate(&$from, &$to, $progress, $sizing, $cc);
    $variant(value)
  }};
  ($variant:path, $from_a:ident, $to_a:ident, $from_b:ident, $to_b:ident, $progress:expr, $sizing:expr, $cc:expr) => {{
    let mut a = $from_a;
    a.interpolate(&$from_a, &$to_a, $progress, $sizing, $cc);
    let mut b = $from_b;
    b.interpolate(&$from_b, &$to_b, $progress, $sizing, $cc);
    $variant(a, b)
  }};
}
pub(crate) use interpolate_field;

/// Lookup table for a single 8-bit channel transition.
pub type TransferTable = [u8; 256];

pub(crate) fn build_transfer_table<F: Fn(usize) -> f32>(f: F) -> TransferTable {
  let mut table = [0u8; 256];
  for (i, entry) in table.iter_mut().enumerate() {
    *entry = f(i).clamp(0.0, 255.0) as u8;
  }
  table
}

/// Represents a single CSS filter operation
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Filter {
  /// Brightness multiplier (1 = unchanged). Accepts number or percentage
  Brightness(PercentageNumber),
  /// Contrast multiplier (1 = unchanged). Accepts number or percentage
  Contrast(PercentageNumber),
  /// Grayscale amount (0..1). Accepts number or percentage
  Grayscale(PercentageNumber),
  /// Saturate multiplier (1 = unchanged). Accepts number or percentage
  Saturate(PercentageNumber),
  /// Hue rotation in degrees
  HueRotate(Angle),
  /// Invert amount (0..1). Accepts number or percentage
  Invert(PercentageNumber),
  /// Sepia amount (0..1). Accepts number or percentage
  Sepia(PercentageNumber),
  /// Opacity amount (0..1). Accepts number or percentage
  Opacity(PercentageNumber),
  /// Blur radius in pixels
  Blur(Length),
  /// Drop shadow effect with offset, blur, and color (reuses TextShadow parsing)
  DropShadow(TextShadow),
  /// An SVG filter referenced with `url(data:image/svg+xml,...)`
  #[cfg(feature = "svg")]
  Reference(std::sync::Arc<FilterReference>),
}

/// Rec. 709 luma weights for the red, green, and blue channels. The CSS
/// `grayscale()` and `saturate()` filters project color onto this luma.
pub const LUMA_WEIGHTS: [f32; 3] = [0.2126, 0.7152, 0.0722];

/// Per-output-channel weights for the CSS `sepia()` filter, row-major: each row
/// is the `[r, g, b]` contribution to one output channel (R, G, B).
pub const SEPIA_WEIGHTS: [[f32; 3]; 3] = [
  [0.393, 0.769, 0.189],
  [0.349, 0.686, 0.168],
  [0.272, 0.534, 0.131],
];

/// How a CSS blur radius maps to a Gaussian σ, which sets how far the blur
/// spreads past its box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlurType {
  /// CSS `filter: blur()` — radius equals σ (standard deviation).
  Filter,
  /// CSS `box-shadow` / `text-shadow` blur — radius equals 2σ.
  Shadow,
}

impl BlurType {
  /// Maps a CSS blur radius to its Gaussian σ.
  #[inline]
  pub fn to_sigma(self, css_radius: f32) -> f32 {
    match self {
      BlurType::Filter => css_radius,
      BlurType::Shadow => css_radius * 0.5,
    }
  }

  /// Multiplier from radius to the extent the blur visibly reaches.
  #[inline]
  pub fn extent_multiplier(self) -> f32 {
    match self {
      BlurType::Filter => 3.0,
      BlurType::Shadow => 1.5,
    }
  }
}

impl Filter {
  /// Whether this is `drop-shadow()`, which `backdrop-filter` ignores.
  pub fn is_drop_shadow(&self) -> bool {
    matches!(self, Filter::DropShadow(_))
  }
}

/// An ordered list of [`Filter`] values.
pub type Filters = Vec<Filter>;

impl MakeComputed for Filter {
  fn make_computed(&mut self, sizing: &SizingContext) {
    match self {
      Filter::Blur(length) => length.make_computed(sizing),
      Filter::DropShadow(shadow) => shadow.make_computed(sizing),
      _ => {}
    }
  }
}

impl Animatable for Filter {
  fn list_interpolation_strategy() -> ListInterpolationStrategy {
    ListInterpolationStrategy::PadToLongestWithNeutral
  }

  fn neutral_value_like(other: &Self) -> Option<Self> {
    Some(match other {
      Filter::Brightness(_) => Filter::Brightness(PercentageNumber(1.0)),
      Filter::Contrast(_) => Filter::Contrast(PercentageNumber(1.0)),
      Filter::Grayscale(_) => Filter::Grayscale(PercentageNumber(0.0)),
      Filter::Saturate(_) => Filter::Saturate(PercentageNumber(1.0)),
      Filter::HueRotate(_) => Filter::HueRotate(Angle::zero()),
      Filter::Invert(_) => Filter::Invert(PercentageNumber(0.0)),
      Filter::Sepia(_) => Filter::Sepia(PercentageNumber(0.0)),
      Filter::Opacity(_) => Filter::Opacity(PercentageNumber(1.0)),
      Filter::Blur(_) => Filter::Blur(Length::zero()),
      Filter::DropShadow(_) => Filter::DropShadow(TextShadow {
        offset_x: Length::zero(),
        offset_y: Length::zero(),
        blur_radius: Length::zero(),
        color: Color::transparent().into(),
      }),
      #[cfg(feature = "svg")]
      Filter::Reference(_) => return None,
    })
  }

  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    *self = match (from, to) {
      (&Filter::Brightness(from), &Filter::Brightness(to)) => interpolate_field!(
        Filter::Brightness,
        from,
        to,
        progress,
        sizing,
        current_color
      ),
      (&Filter::Contrast(from), &Filter::Contrast(to)) => {
        interpolate_field!(Filter::Contrast, from, to, progress, sizing, current_color)
      }
      (&Filter::Grayscale(from), &Filter::Grayscale(to)) => {
        interpolate_field!(Filter::Grayscale, from, to, progress, sizing, current_color)
      }
      (&Filter::Saturate(from), &Filter::Saturate(to)) => {
        interpolate_field!(Filter::Saturate, from, to, progress, sizing, current_color)
      }
      (&Filter::HueRotate(from), &Filter::HueRotate(to)) => {
        interpolate_field!(Filter::HueRotate, from, to, progress, sizing, current_color)
      }
      (&Filter::Invert(from), &Filter::Invert(to)) => {
        interpolate_field!(Filter::Invert, from, to, progress, sizing, current_color)
      }
      (&Filter::Sepia(from), &Filter::Sepia(to)) => {
        interpolate_field!(Filter::Sepia, from, to, progress, sizing, current_color)
      }
      (&Filter::Opacity(from), &Filter::Opacity(to)) => {
        interpolate_field!(Filter::Opacity, from, to, progress, sizing, current_color)
      }
      (&Filter::Blur(from), &Filter::Blur(to)) => {
        interpolate_field!(Filter::Blur, from, to, progress, sizing, current_color)
      }
      (&Filter::DropShadow(from), &Filter::DropShadow(to)) => interpolate_field!(
        Filter::DropShadow,
        from,
        to,
        progress,
        sizing,
        current_color
      ),
      _ => {
        if progress >= 0.5 {
          to.clone()
        } else {
          from.clone()
        }
      }
    };
  }
}

impl TailwindPropertyParser for Filters {
  fn parse_tw(token: &str) -> Option<Self> {
    if token.eq_ignore_ascii_case("none") {
      return Some(Filters::default());
    }
    None
  }
}

impl Filter {
  /// Classifies the filter as a pixel or complex operation.
  pub fn categorize(&self) -> FilterCategory<'_> {
    match self {
      Filter::Blur(_) | Filter::DropShadow(_) | Filter::HueRotate(_) => {
        FilterCategory::Complex(self)
      }
      #[cfg(feature = "svg")]
      Filter::Reference(_) => FilterCategory::Complex(self),
      _ => FilterCategory::Pixel(self),
    }
  }

  /// Returns the 1D channel transfer table for this filter, if any.
  pub fn transfer_table(&self) -> Option<TransferChannel> {
    match *self {
      Filter::Brightness(PercentageNumber(v)) => {
        Some(TransferChannel::Rgb(build_transfer_table(|i| i as f32 * v)))
      }
      Filter::Contrast(PercentageNumber(v)) => {
        Some(TransferChannel::Rgb(build_transfer_table(|i| {
          (i as f32 - 128.0) * v + 128.0
        })))
      }
      Filter::Invert(PercentageNumber(v)) => {
        Some(TransferChannel::Rgb(build_transfer_table(|i| {
          let inverted = 255 - i as u8;
          (i as f32 * (1.0 - v)) + (inverted as f32 * v)
        })))
      }
      Filter::Opacity(PercentageNumber(v)) => {
        Some(TransferChannel::Alpha(build_transfer_table(|i| {
          i as f32 * v
        })))
      }
      _ => None,
    }
  }
}

/// A 1D channel transfer for a filter, tagged with which channels it touches.
pub enum TransferChannel {
  /// Applies to R, G, B (leaves alpha untouched).
  Rgb(TransferTable),
  /// Applies to alpha only.
  Alpha(TransferTable),
}

/// Composes `next` after `existing` so that `existing[i] = next[existing[i]]`,
/// collapsing two LUTs into a single equivalent LUT.
#[inline]
pub fn compose_transfer_table(existing: &mut TransferTable, next: &TransferTable) {
  for entry in existing.iter_mut() {
    *entry = next[*entry as usize];
  }
}

/// Category of filters for optimization purposes.
pub enum FilterCategory<'f> {
  /// Pixel filters that can potentially be batched
  Pixel(&'f Filter),
  /// Complex filters that need special handling (blur, drop-shadow, hue-rotate)
  Complex(&'f Filter),
}

impl<'i> FromCss<'i> for Filters {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
      return Ok(Filters::default());
    }

    let mut filters = Vec::new();
    while !input.is_exhausted() {
      filters.push(Filter::from_css(input)?);
    }

    Ok(filters)
  }

  const VALID_TOKENS: &'static [CssToken] = Filter::VALID_TOKENS;
  const EXPECT_MESSAGE: CssExpectedMessage = CssExpectedMessage::ValueOrNone;
}

#[cfg(feature = "svg")]
fn parse_filter_reference<'i>(
  url: &str,
  location: cssparser::SourceLocation,
) -> ParseResult<'i, Filter> {
  FilterReference::from_url(url)
    .map(|reference| Filter::Reference(std::sync::Arc::new(reference)))
    .map_err(|error| cssparser::ParseError {
      kind: cssparser::ParseErrorKind::Custom(std::borrow::Cow::Owned(error.to_string())),
      location,
    })
}

impl<'i> FromCss<'i> for Filter {
  fn from_css(parser: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = parser.current_source_location();
    let token = parser.next()?;

    #[cfg(feature = "svg")]
    if let Token::UnquotedUrl(url) = token {
      return parse_filter_reference(url, location);
    }

    let Token::Function(function) = token else {
      return Err(
        location
          .new_basic_unexpected_token_error(token.clone())
          .into(),
      );
    };

    #[cfg(feature = "svg")]
    if function.eq_ignore_ascii_case("url") {
      return parser.parse_nested_block(|input| {
        let url = input.expect_string()?.clone();
        parse_filter_reference(&url, location)
      });
    }

    match_ignore_ascii_case! {function,
      "brightness" => parser.parse_nested_block(|input| {
        Ok(Filter::Brightness(PercentageNumber::from_css(input)?))
      }),
      "opacity" => parser.parse_nested_block(|input| {
        Ok(Filter::Opacity(PercentageNumber::from_css(input)?))
      }),
      "contrast" => parser.parse_nested_block(|input| {
        Ok(Filter::Contrast(PercentageNumber::from_css(input)?))
      }),
      "grayscale" => parser.parse_nested_block(|input| {
        Ok(Filter::Grayscale(PercentageNumber::from_css(input)?))
      }),
      "hue-rotate" => parser.parse_nested_block(|input| {
        Ok(Filter::HueRotate(Angle::from_css(input)?))
      }),
      "invert" => parser.parse_nested_block(|input| {
        Ok(Filter::Invert(PercentageNumber::from_css(input)?))
      }),
      "saturate" => parser.parse_nested_block(|input| {
        Ok(Filter::Saturate(PercentageNumber::from_css(input)?))
      }),
      "sepia" => parser.parse_nested_block(|input| {
        Ok(Filter::Sepia(PercentageNumber::from_css(input)?))
      }),
      "blur" => parser.parse_nested_block(|input| {
        // blur() radius is optional and defaults to 0; a present argument must be a valid length.
        let radius = if input.is_exhausted() {
          Length::zero()
        } else {
          Length::from_css(input)?
        };
        Ok(Filter::Blur(radius))
      }),
      "drop-shadow" => parser.parse_nested_block(|input| {
        // drop-shadow uses the same syntax as text-shadow
        Ok(Filter::DropShadow(TextShadow::from_css(input)?))
      }),
      _ => Err(unexpected_token!(location, token)),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Descriptor(CssDescriptorKind::BrightnessFn),
    CssToken::Descriptor(CssDescriptorKind::OpacityFn),
    CssToken::Descriptor(CssDescriptorKind::ContrastFn),
    CssToken::Descriptor(CssDescriptorKind::GrayscaleFn),
    CssToken::Descriptor(CssDescriptorKind::HueRotateFn),
    CssToken::Descriptor(CssDescriptorKind::InvertFn),
    CssToken::Descriptor(CssDescriptorKind::SaturateFn),
    CssToken::Descriptor(CssDescriptorKind::SepiaFn),
    CssToken::Descriptor(CssDescriptorKind::BlurFn),
    CssToken::Descriptor(CssDescriptorKind::DropShadowFn),
    #[cfg(feature = "svg")]
    CssToken::Descriptor(CssDescriptorKind::UrlFn),
  ];
}

impl ToCss for Filter {
  // The `filter`/`backdrop-filter` grammar is space-separated, not comma.
  const LIST_SEPARATOR: &'static str = " ";
  // An empty filter list is the keyword `none`.
  const EMPTY_LIST_KEYWORD: Option<&'static str> = Some("none");

  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    macro_rules! write_fn {
      ($name:expr, $v:expr) => {{
        dest.write_str($name)?;
        dest.write_char('(')?;
        $v.to_css(dest)?;
        dest.write_char(')')
      }};
    }
    match self {
      Self::Brightness(v) => write_fn!("brightness", v),
      Self::Contrast(v) => write_fn!("contrast", v),
      Self::Grayscale(v) => write_fn!("grayscale", v),
      Self::Saturate(v) => write_fn!("saturate", v),
      Self::HueRotate(v) => write_fn!("hue-rotate", v),
      Self::Invert(v) => write_fn!("invert", v),
      Self::Sepia(v) => write_fn!("sepia", v),
      Self::Opacity(v) => write_fn!("opacity", v),
      Self::Blur(v) => write_fn!("blur", v),
      Self::DropShadow(v) => write_fn!("drop-shadow", v),
      #[cfg(feature = "svg")]
      Self::Reference(reference) => {
        dest.write_str("url(\"")?;
        for c in reference.uri.chars() {
          if matches!(c, '"' | '\\') {
            dest.write_char('\\')?;
          }
          dest.write_char(c)?;
        }
        dest.write_str("\")")
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::{Color, ColorInput, FromCssStr, Length::Px};

  #[test]
  fn test_parse_blur_filter() {
    assert_eq!(Filter::from_css_str("blur(5px)"), Ok(Filter::Blur(Px(5.0))));
  }

  #[test]
  fn test_parse_none_clears_filters() {
    assert_eq!(Filters::from_css_str("none").unwrap(), Filters::default());
  }

  #[test]
  fn filters_serialize_space_separated() {
    let filters = Filters::from_css_str("blur(3px) grayscale(0.5)").unwrap();
    assert_eq!(filters.to_css_string(), "blur(3px) grayscale(0.5)");
  }

  #[test]
  fn empty_filters_serialize_as_none() {
    assert_eq!(Filters::default().to_css_string(), "none");
    assert_eq!(
      Filters::from_css_str("none").unwrap().to_css_string(),
      "none"
    );
  }

  #[test]
  fn test_parse_blur_rejects_invalid_argument() {
    assert_eq!(
      Filter::from_css_str("blur()"),
      Ok(Filter::Blur(Length::zero()))
    );
    assert!(Filter::from_css_str("blur(red)").is_err());
  }

  #[test]
  fn test_parse_blur_filter_zero() {
    assert_eq!(
      Filter::from_css_str("blur()"),
      Ok(Filter::Blur(Length::zero()))
    );
  }

  #[test]
  fn test_parse_drop_shadow_filter() {
    assert_eq!(
      Filter::from_css_str("drop-shadow(2px 4px 6px red)"),
      Ok(Filter::DropShadow(TextShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: Px(6.0),
        color: ColorInput::Value(Color([255, 0, 0, 255])),
      }))
    );
  }

  #[test]
  fn test_parse_drop_shadow_color_first() {
    assert_eq!(
      Filter::from_css_str("drop-shadow(red 2px 4px)"),
      Ok(Filter::DropShadow(TextShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: Length::zero(),
        color: ColorInput::Value(Color([255, 0, 0, 255])),
      }))
    );
  }

  #[test]
  fn test_parse_drop_shadow_no_blur() {
    assert_eq!(
      Filter::from_css_str("drop-shadow(2px 4px)"),
      Ok(Filter::DropShadow(TextShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: Length::zero(),
        color: ColorInput::CurrentColor,
      }))
    );
  }
}
