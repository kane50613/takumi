use std::fmt;

use cssparser::{ParseError as CssParseError, Parser, Token, match_ignore_ascii_case};

#[cfg(feature = "svg")]
use std::{borrow::Cow, sync::Arc};

#[cfg(feature = "svg")]
use crate::style::{ValueError, properties::filter_reference::FilterReference};
use crate::{
  scene::SceneBounds,
  style::{
    Affine, Angle, Animatable, Color, CssExpectedMessage, FromCss, Length,
    ListInterpolationStrategy, MakeComputed, ParseResult, PercentageNumber, SizingContext,
    TextShadow, ToCss, discrete, tw::TailwindPropertyParser, unexpected_token,
  },
};

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
  Reference(Arc<FilterReference>),
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
  fn extent_multiplier(self) -> f32 {
    match self {
      BlurType::Filter => 3.0,
      BlurType::Shadow => 1.5,
    }
  }

  /// How far a blur of `radius` visibly reaches.
  #[inline]
  pub fn extent(self, radius: f32) -> f32 {
    radius * self.extent_multiplier()
  }
}

impl Filter {
  /// Whether this is `drop-shadow()`, which `backdrop-filter` ignores.
  pub fn is_drop_shadow(&self) -> bool {
    matches!(self, Filter::DropShadow(_))
  }

  /// How far the filter spreads a layer's ink, in local px.
  pub fn reach(&self, sizing: &SizingContext) -> f32 {
    match self {
      Filter::Blur(radius) => BlurType::Filter.extent(radius.to_px(sizing, 1.0)),
      Filter::DropShadow(shadow) => {
        let offset = shadow
          .offset_x
          .to_px(sizing, 1.0)
          .abs()
          .max(shadow.offset_y.to_px(sizing, 1.0).abs());

        offset + BlurType::Filter.extent(shadow.blur_radius.to_px(sizing, 1.0))
      }
      _ => 0.0,
    }
  }
}

/// Which way [`Filter::map_bounds`] maps, Skia's `SkImageFilter::MapDirection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapDirection {
  /// From the pixels a filter reads to the ones it writes.
  Forward,
  /// From the pixels a filter writes to the ones it reads.
  Reverse,
}

impl Filter {
  /// cc's `FilterOperation::MapRect` and `MapRectReverse`: the device pixels this filter writes
  /// from `bounds`, or reads to fill them, under `transform` from local to device px.
  /// Approximate: cc maps in the surface's scaled space and draws that into device space, where
  /// takumi maps in device space, so a deviation takes the transform's scale components and a
  /// shadow offset its whole linear part; a reference filter stays unmapped, where cc asks its
  /// image filter.
  /// <https://source.chromium.org/chromium/chromium/src/+/main:cc/paint/filter_operation.cc>
  pub fn map_bounds(
    &self,
    bounds: SceneBounds,
    sizing: &SizingContext,
    transform: Affine,
    direction: MapDirection,
  ) -> SceneBounds {
    let (scale_x, scale_y) = transform.scale_components();
    // cc's `MapStdDeviation`: three scaled standard deviations.
    let spread = |sigma: f32| ((sigma * scale_x * 3.0).abs(), (sigma * scale_y * 3.0).abs());
    let outset = |(x, y): (f32, f32), (dx, dy): (f32, f32)| {
      SceneBounds::enclosing(
        bounds.left as f32 - x + dx,
        bounds.top as f32 - y + dy,
        bounds.right as f32 + x + dx,
        bounds.bottom as f32 + y + dy,
      )
    };

    match self {
      Filter::Blur(radius) => outset(spread(radius.to_px(sizing, 1.0)), (0.0, 0.0)),
      Filter::DropShadow(shadow) => {
        let (dx, dy) = transform.transform_vector(
          shadow.offset_x.to_px(sizing, 1.0),
          shadow.offset_y.to_px(sizing, 1.0),
        );
        let offset = match direction {
          MapDirection::Forward => (dx, dy),
          MapDirection::Reverse => (-dx, -dy),
        };

        outset(spread(shadow.blur_radius.to_px(sizing, 1.0)), offset).union(bounds)
      }
      _ => bounds,
    }
  }
}

/// cc's `FilterOperations::ExpandRect`: the pixels `filters` write from `bounds` and those they
/// read to fill them, each list mapped in order forward and in reverse order back.
/// <https://source.chromium.org/chromium/chromium/src/+/main:cc/paint/filter_operations.cc>
pub fn expand_filter_bounds(
  filters: &[Filter],
  bounds: SceneBounds,
  sizing: &SizingContext,
  transform: Affine,
) -> SceneBounds {
  let map =
    |bounds, filter: &Filter, direction| filter.map_bounds(bounds, sizing, transform, direction);
  let forward = filters.iter().fold(bounds, |bounds, filter| {
    map(bounds, filter, MapDirection::Forward)
  });
  let reverse = filters.iter().rev().fold(bounds, |bounds, filter| {
    map(bounds, filter, MapDirection::Reverse)
  });

  forward.union(reverse)
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
      Filter::DropShadow(shadow) => Filter::DropShadow(TextShadow::neutral_value_like(shadow)?),
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
    *self =
      match (from, to) {
        (Filter::Brightness(from), Filter::Brightness(to)) => Filter::Brightness(
          Animatable::interpolated(from, to, progress, sizing, current_color),
        ),
        (Filter::Contrast(from), Filter::Contrast(to)) => Filter::Contrast(
          Animatable::interpolated(from, to, progress, sizing, current_color),
        ),
        (Filter::Grayscale(from), Filter::Grayscale(to)) => Filter::Grayscale(
          Animatable::interpolated(from, to, progress, sizing, current_color),
        ),
        (Filter::Saturate(from), Filter::Saturate(to)) => Filter::Saturate(
          Animatable::interpolated(from, to, progress, sizing, current_color),
        ),
        (Filter::HueRotate(from), Filter::HueRotate(to)) => Filter::HueRotate(
          Animatable::interpolated(from, to, progress, sizing, current_color),
        ),
        (Filter::Invert(from), Filter::Invert(to)) => Filter::Invert(Animatable::interpolated(
          from,
          to,
          progress,
          sizing,
          current_color,
        )),
        (Filter::Sepia(from), Filter::Sepia(to)) => Filter::Sepia(Animatable::interpolated(
          from,
          to,
          progress,
          sizing,
          current_color,
        )),
        (Filter::Opacity(from), Filter::Opacity(to)) => Filter::Opacity(Animatable::interpolated(
          from,
          to,
          progress,
          sizing,
          current_color,
        )),
        (Filter::Blur(from), Filter::Blur(to)) => Filter::Blur(Animatable::interpolated(
          from,
          to,
          progress,
          sizing,
          current_color,
        )),
        (Filter::DropShadow(from), Filter::DropShadow(to)) => Filter::DropShadow(
          Animatable::interpolated(from, to, progress, sizing, current_color),
        ),
        _ => discrete(from, to, progress),
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
      Filter::Blur(_) | Filter::DropShadow(_) => FilterCategory::Complex(self),
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
  /// Complex filters that need special handling (blur, drop-shadow)
  Complex(&'f Filter),
}

impl<'i> FromCss<'i> for Filters {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
      return Ok(Filters::default());
    }

    let mut filters = Vec::new();
    while !input.is_exhausted() {
      filters.push(Filter::from_css(input)?);
    }

    Ok(filters)
  }

  const VALID_TOKENS: &'static [&'static str] = Filter::VALID_TOKENS;
  const EXPECT_MESSAGE: CssExpectedMessage = CssExpectedMessage::ValueOrNone;
}

#[cfg(feature = "svg")]
fn parse_filter_reference<'i>(url: &str) -> ParseResult<'i, Filter> {
  FilterReference::from_url(url)
    .map(|reference| Filter::Reference(Arc::new(reference)))
    .map_err(|error| CssParseError::custom(ValueError::from(Cow::<str>::Owned(error.to_string()))))
}

impl<'i> FromCss<'i> for Filter {
  fn from_css(parser: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let token = parser.next()?;

    #[cfg(feature = "svg")]
    if let Token::UnquotedUrl(url) = token {
      return parse_filter_reference(url);
    }

    let Token::Function(function) = token else {
      return Err(CssParseError::unexpected_token());
    };

    #[cfg(feature = "svg")]
    if function.eq_ignore_ascii_case("url") {
      return parser.parse_nested_block(|input| {
        let url = input.expect_string()?.clone();
        parse_filter_reference(&url)
      });
    }

    match_ignore_ascii_case! {function,
      "brightness" => parser.parse_nested_block(|input| PercentageNumber::from_css(input).map(Filter::Brightness)),
      "opacity" => parser.parse_nested_block(|input| PercentageNumber::from_css(input).map(Filter::Opacity)),
      "contrast" => parser.parse_nested_block(|input| PercentageNumber::from_css(input).map(Filter::Contrast)),
      "grayscale" => parser.parse_nested_block(|input| PercentageNumber::from_css(input).map(Filter::Grayscale)),
      "hue-rotate" => parser.parse_nested_block(|input| Angle::from_css(input).map(Filter::HueRotate)),
      "invert" => parser.parse_nested_block(|input| PercentageNumber::from_css(input).map(Filter::Invert)),
      "saturate" => parser.parse_nested_block(|input| PercentageNumber::from_css(input).map(Filter::Saturate)),
      "sepia" => parser.parse_nested_block(|input| PercentageNumber::from_css(input).map(Filter::Sepia)),
      "blur" => parser.parse_nested_block(|input| {
        // blur() radius is optional and defaults to 0; a present argument must be a valid length.
        let radius = if input.is_exhausted() {
          Length::zero()
        } else {
          Length::from_css(input)?
        };
        Ok(Filter::Blur(radius))
      }),
      "drop-shadow" => parser.parse_nested_block(|input| TextShadow::from_css(input).map(Filter::DropShadow)),
      _ => Err(unexpected_token!(token)),
    }
  }

  const VALID_TOKENS: &'static [&'static str] = &[
    "<brightness()>",
    "<opacity()>",
    "<contrast()>",
    "<grayscale()>",
    "<hue-rotate()>",
    "<invert()>",
    "<saturate()>",
    "<sepia()>",
    "<blur()>",
    "<drop-shadow()>",
    #[cfg(feature = "svg")]
    "<url()>",
  ];
}

impl ToCss for Filter {
  const LIST_SEPARATOR: &'static str = " ";
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
      // A minimal data URI rebuilt from the extracted markup, shorter than the
      // author's original. Only characters that would break the CSS string or
      // the data URI body are percent-encoded; `#` is handled on re-parse.
      Self::Reference(reference) => {
        dest.write_str("url(\"data:image/svg+xml,")?;
        for c in reference.markup.chars() {
          match c {
            '%' => dest.write_str("%25"),
            '"' => dest.write_str("%22"),
            '\n' => dest.write_str("%0A"),
            '\r' => dest.write_str("%0D"),
            _ => dest.write_char(c),
          }?;
        }
        dest.write_str("\")")
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::*;
  use crate::{
    geometry::Size,
    style::{CalcArena, Color, ColorInput, FromCssStr, Length::Px},
    viewport::Viewport,
  };

  fn sizing() -> SizingContext {
    SizingContext {
      viewport: Viewport::new((200, 100)),
      container_size: Size::NONE,
      container_read: Default::default(),
      font_size: 16.0,
      root_font_size: None,
      line_height: 0.0,
      root_line_height: None,
      ch: 16.0 / 2.0,
      calc_arena: Rc::new(CalcArena::default()),
    }
  }

  fn rect(left: i32, top: i32, right: i32, bottom: i32) -> SceneBounds {
    SceneBounds {
      left,
      top,
      right,
      bottom,
    }
  }

  fn map(css: &str, transform: Affine, direction: MapDirection) -> SceneBounds {
    Filter::from_css_str(css).unwrap().map_bounds(
      rect(0, 0, 100, 50),
      &sizing(),
      transform,
      direction,
    )
  }

  #[test]
  fn a_blur_reaches_three_scaled_deviations_both_ways() {
    for direction in [MapDirection::Forward, MapDirection::Reverse] {
      assert_eq!(
        map("blur(10px)", Affine::IDENTITY, direction),
        rect(-30, -30, 130, 80)
      );
      assert_eq!(
        map("blur(10px)", Affine::scale(2.0, 2.0), direction),
        rect(-60, -60, 160, 110)
      );
    }
  }

  #[test]
  fn a_rotated_blur_keeps_its_deviation_on_both_axes() {
    let quarter_turn = Affine {
      a: 0.0,
      b: 1.0,
      c: -1.0,
      d: 0.0,
      x: 0.0,
      y: 0.0,
    };

    assert_eq!(
      map("blur(10px)", quarter_turn, MapDirection::Forward),
      rect(-30, -30, 130, 80)
    );
  }

  #[test]
  fn a_drop_shadow_moves_its_blur_along_the_offset_and_keeps_the_source() {
    let shadow = "drop-shadow(10px 5px 2px red)";

    assert_eq!(
      map(shadow, Affine::IDENTITY, MapDirection::Forward),
      rect(0, -1, 116, 61)
    );
    assert_eq!(
      map(shadow, Affine::IDENTITY, MapDirection::Reverse),
      rect(-16, -11, 100, 51)
    );
  }

  #[test]
  fn expanding_unites_what_filters_write_and_read() {
    let filters = Filters::from_css_str("drop-shadow(10px 5px 2px red) grayscale(1)").unwrap();

    assert_eq!(
      expand_filter_bounds(&filters, rect(0, 0, 100, 50), &sizing(), Affine::IDENTITY),
      rect(-16, -11, 116, 61)
    );
  }

  #[test]
  fn a_filter_that_moves_no_pixels_keeps_the_bounds() {
    assert_eq!(
      map("grayscale(1)", Affine::IDENTITY, MapDirection::Reverse),
      rect(0, 0, 100, 50)
    );
  }

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
  fn reference_serializes_minimal_data_uri() {
    let filters = Filters::from_css_str(
      "url(\"data:image/svg+xml,%3Cfilter%3E%3CfeFlood flood-color='%23f00'/%3E%3C/filter%3E\")",
    )
    .unwrap();
    let css = filters.to_css_string();

    assert_eq!(
      css,
      r##"url("data:image/svg+xml,<filter id=%22takumi-filter%22><feFlood flood-color='#f00'/></filter>")"##
    );
    assert_eq!(Filters::from_css_str(&css).unwrap(), filters);
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
