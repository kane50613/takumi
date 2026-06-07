use std::fmt;

use crate::style::{ToCss, unexpected_token};
use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::style::{
  Angle, Animatable, Color, CssDescriptorKind, CssToken, FromCss, Length,
  ListInterpolationStrategy, MakeComputed, ParseResult, PercentageNumber, SizingContext,
  TextShadow, tw::TailwindPropertyParser,
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
#[derive(Debug, Clone, Copy, PartialEq)]
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
}

/// A list of filter operations
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
    Some(match *other {
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
    *self = match (*from, *to) {
      (Filter::Brightness(from), Filter::Brightness(to)) => interpolate_field!(
        Filter::Brightness,
        from,
        to,
        progress,
        sizing,
        current_color
      ),
      (Filter::Contrast(from), Filter::Contrast(to)) => {
        interpolate_field!(Filter::Contrast, from, to, progress, sizing, current_color)
      }
      (Filter::Grayscale(from), Filter::Grayscale(to)) => {
        interpolate_field!(Filter::Grayscale, from, to, progress, sizing, current_color)
      }
      (Filter::Saturate(from), Filter::Saturate(to)) => {
        interpolate_field!(Filter::Saturate, from, to, progress, sizing, current_color)
      }
      (Filter::HueRotate(from), Filter::HueRotate(to)) => {
        interpolate_field!(Filter::HueRotate, from, to, progress, sizing, current_color)
      }
      (Filter::Invert(from), Filter::Invert(to)) => {
        interpolate_field!(Filter::Invert, from, to, progress, sizing, current_color)
      }
      (Filter::Sepia(from), Filter::Sepia(to)) => {
        interpolate_field!(Filter::Sepia, from, to, progress, sizing, current_color)
      }
      (Filter::Opacity(from), Filter::Opacity(to)) => {
        interpolate_field!(Filter::Opacity, from, to, progress, sizing, current_color)
      }
      (Filter::Blur(from), Filter::Blur(to)) => {
        interpolate_field!(Filter::Blur, from, to, progress, sizing, current_color)
      }
      (Filter::DropShadow(from), Filter::DropShadow(to)) => interpolate_field!(
        Filter::DropShadow,
        from,
        to,
        progress,
        sizing,
        current_color
      ),
      _ => {
        if progress >= 0.5 {
          *to
        } else {
          *from
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
  pub fn categorize(&self) -> FilterCategory<'_> {
    match self {
      Filter::Blur(_) | Filter::DropShadow(_) | Filter::HueRotate(_) => {
        FilterCategory::Complex(self)
      }
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
    let mut filters = Vec::new();

    while !input.is_exhausted() {
      let filter = Filter::from_css(input)?;
      filters.push(filter);
    }

    Ok(filters)
  }

  const VALID_TOKENS: &'static [CssToken] = Filter::VALID_TOKENS;
}

impl<'i> FromCss<'i> for Filter {
  fn from_css(parser: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = parser.current_source_location();
    let token = parser.next()?;

    let Token::Function(function) = token else {
      return Err(
        location
          .new_basic_unexpected_token_error(token.clone())
          .into(),
      );
    };

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
        // blur() can have an optional radius, defaults to 0
        let radius = input
          .try_parse(Length::from_css)
          .unwrap_or(Length::zero());
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
  ];
}

impl ToCss for Filter {
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
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::{Color, ColorInput, Length::Px};

  #[test]
  fn test_parse_blur_filter() {
    assert_eq!(Filter::from_str("blur(5px)"), Ok(Filter::Blur(Px(5.0))));
  }

  #[test]
  fn test_parse_blur_filter_zero() {
    assert_eq!(Filter::from_str("blur()"), Ok(Filter::Blur(Length::zero())));
  }

  #[test]
  fn test_parse_drop_shadow_filter() {
    assert_eq!(
      Filter::from_str("drop-shadow(2px 4px 6px red)"),
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
      Filter::from_str("drop-shadow(red 2px 4px)"),
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
      Filter::from_str("drop-shadow(2px 4px)"),
      Ok(Filter::DropShadow(TextShadow {
        offset_x: Px(2.0),
        offset_y: Px(4.0),
        blur_radius: Length::zero(),
        color: ColorInput::CurrentColor,
      }))
    );
  }
}
