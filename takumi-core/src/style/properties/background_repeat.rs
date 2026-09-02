use std::{fmt, iter::successors};

use cssparser::{Parser, match_ignore_ascii_case};
use smallvec::{SmallVec, smallvec};

use super::background_image::parse_comma_list;
use crate::style::{
  Animatable, CssToken, FromCss, ListInterpolationStrategy, MakeComputed, ParseResult, ToCss,
  declare_enum_from_css_impl,
};

/// Tile origins along one axis for `background-repeat: repeat`: the first origin
/// at or before 0, then one per `tile_size` up to `area_size`.
pub(crate) fn collect_repeat_tile_positions(
  area_size: u32,
  tile_size: u32,
  origin: i32,
) -> SmallVec<[i32; 1]> {
  if tile_size == 0 {
    return SmallVec::default();
  }

  // The arithmetic runs in i64 so a dimension near the u32 range neither
  // wraps nor drops tiles; every emitted position also fits the area, which
  // callers keep within the canvas pixel budget.
  let area_end = i64::from(area_size);
  let tile_size = i64::from(tile_size);
  // Any origin normalizes to the phase-equivalent first tile at or just
  // before the area start, so a far-negative origin cannot make the walk
  // begin billions of tiles away.
  let rem = i64::from(origin).rem_euclid(tile_size);
  let start = if rem == 0 { 0 } else { rem - tile_size };

  successors(Some(start), |&x| Some(x + tile_size))
    .take_while(|&x| x < area_end)
    .filter_map(|x| i32::try_from(x).ok())
    .collect()
}

/// Tile origins for `background-repeat: space`: whole tiles spread to the edges
/// with equal gaps between them, or a single centered tile when only one fits.
pub(crate) fn collect_spaced_tile_positions(area_size: u32, tile_size: u32) -> SmallVec<[i32; 1]> {
  if tile_size == 0 {
    return SmallVec::default();
  }

  let count = area_size / tile_size;
  if count <= 1 {
    return smallvec![(area_size as i32 - tile_size as i32) / 2];
  }

  let gap = (area_size - count * tile_size) / (count - 1);
  let step = tile_size as i32 + gap as i32;

  successors(Some(0i32), move |&x| Some(x + step))
    .take(count as usize)
    .collect()
}

/// Tile origins for `background-repeat: round`, with the tile rescaled so a whole
/// number fit the area. Returns the origins and the rounded tile size.
pub(crate) fn collect_stretched_tile_positions(
  area_size: u32,
  tile_size: u32,
) -> (SmallVec<[i32; 1]>, u32) {
  if tile_size == 0 || area_size == 0 {
    return (SmallVec::default(), tile_size);
  }

  let count = (area_size as f32 / tile_size as f32).max(1.0) as u32;
  let new_tile_size = (area_size as f32 / count as f32) as u32;

  let positions = successors(Some(0i32), move |&x| Some(x + new_tile_size as i32))
    .take(count as usize)
    .collect();

  (positions, new_tile_size)
}

/// Per-axis repeat style.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BackgroundRepeatStyle {
  /// Tile as many times as needed with no extra spacing
  #[default]
  Repeat,
  /// Do not tile on this axis
  NoRepeat,
  /// Distribute leftover space evenly between tiles; edges flush with sides
  Space,
  /// Scale tile so an integer number fits exactly
  Round,
}

declare_enum_from_css_impl!(
  BackgroundRepeatStyle,
  "repeat" => BackgroundRepeatStyle::Repeat,
  "no-repeat" => BackgroundRepeatStyle::NoRepeat,
  "space" => BackgroundRepeatStyle::Space,
  "round" => BackgroundRepeatStyle::Round,
);

/// Combined repeat for X and Y axes.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BackgroundRepeat(pub BackgroundRepeatStyle, pub BackgroundRepeatStyle);

impl MakeComputed for BackgroundRepeat {}

impl Animatable for BackgroundRepeat {
  fn list_interpolation_strategy() -> ListInterpolationStrategy {
    ListInterpolationStrategy::RepeatToLcm
  }
}

impl BackgroundRepeat {
  /// Returns a repeat value that tiles on both the X and Y axes.
  pub const fn repeat() -> Self {
    Self(BackgroundRepeatStyle::Repeat, BackgroundRepeatStyle::Repeat)
  }

  /// Returns a repeat value that does not tile on either axis.
  pub(crate) const fn no_repeat() -> Self {
    Self(
      BackgroundRepeatStyle::NoRepeat,
      BackgroundRepeatStyle::NoRepeat,
    )
  }

  /// Returns a repeat value that distributes leftover space evenly between tiles; edges flush with sides.
  pub const fn space() -> Self {
    Self(BackgroundRepeatStyle::Space, BackgroundRepeatStyle::Space)
  }

  /// Returns a repeat value that scales tile so an integer number fits exactly.
  pub const fn round() -> Self {
    Self(BackgroundRepeatStyle::Round, BackgroundRepeatStyle::Round)
  }
}

impl<'i> FromCss<'i> for BackgroundRepeat {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let state = input.state();
    let ident = input.expect_ident()?;

    match_ignore_ascii_case! { ident,
      "repeat-x" => return Ok(BackgroundRepeat(BackgroundRepeatStyle::Repeat, BackgroundRepeatStyle::NoRepeat)),
      "repeat-y" => return Ok(BackgroundRepeat(BackgroundRepeatStyle::NoRepeat, BackgroundRepeatStyle::Repeat)),
      _ => {}
    }

    input.reset(&state);

    let x = BackgroundRepeatStyle::from_css(input)?;
    let y = input
      .try_parse(BackgroundRepeatStyle::from_css)
      .unwrap_or(x);
    Ok(BackgroundRepeat(x, y))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("repeat-x"),
    CssToken::Keyword("repeat-y"),
    CssToken::Keyword("repeat"),
    CssToken::Keyword("no-repeat"),
    CssToken::Keyword("space"),
    CssToken::Keyword("round"),
  ];
}

/// An ordered list of [`BackgroundRepeat`] values.
pub type BackgroundRepeats = Box<[BackgroundRepeat]>;

impl<'i> FromCss<'i> for BackgroundRepeats {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    parse_comma_list(input, BackgroundRepeat::from_css)
  }

  const VALID_TOKENS: &'static [CssToken] = BackgroundRepeat::VALID_TOKENS;
}

impl ToCss for BackgroundRepeat {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match (self.0, self.1) {
      (BackgroundRepeatStyle::Repeat, BackgroundRepeatStyle::NoRepeat) => {
        dest.write_str("repeat-x")
      }
      (BackgroundRepeatStyle::NoRepeat, BackgroundRepeatStyle::Repeat) => {
        dest.write_str("repeat-y")
      }
      (x, y) => {
        if x == y {
          x.to_css(dest)
        } else {
          x.to_css(dest)?;
          dest.write_str(" ")?;
          y.to_css(dest)
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn test_parse_background_repeat() {
    for (css, expected) in [
      (
        "repeat-x",
        BackgroundRepeat(
          BackgroundRepeatStyle::Repeat,
          BackgroundRepeatStyle::NoRepeat,
        ),
      ),
      (
        "repeat-y",
        BackgroundRepeat(
          BackgroundRepeatStyle::NoRepeat,
          BackgroundRepeatStyle::Repeat,
        ),
      ),
      (
        "repeat no-repeat",
        BackgroundRepeat(
          BackgroundRepeatStyle::Repeat,
          BackgroundRepeatStyle::NoRepeat,
        ),
      ),
      (
        "space",
        BackgroundRepeat(BackgroundRepeatStyle::Space, BackgroundRepeatStyle::Space),
      ),
    ] {
      assert_eq!(
        BackgroundRepeat::from_css_str(css),
        Ok(expected),
        "failed for {css}"
      );
    }
  }

  #[test]
  fn test_background_repeat_round_trip() {
    for css in ["repeat-x", "repeat-y", "repeat no-repeat", "space", "round"] {
      let parsed = BackgroundRepeat::from_css_str(css).unwrap();
      let reparsed = BackgroundRepeat::from_css_str(&parsed.to_css_string()).unwrap();
      assert_eq!(parsed, reparsed, "failed for {css}");
    }
  }

  #[test]
  fn oversized_repeat_tile_does_not_wrap_into_an_infinite_iterator() {
    assert_eq!(
      collect_repeat_tile_positions(100, u32::MAX, 0).as_slice(),
      &[0]
    );
  }

  #[test]
  fn far_negative_origin_normalizes_to_the_area_edge() {
    // i32::MIN is a multiple of 1: the walk starts at 0, not two billion
    // tiles below the area.
    let positions = collect_repeat_tile_positions(100, 1, i32::MIN);

    assert_eq!(positions.len(), 100);
    assert_eq!(positions.first(), Some(&0));
    assert_eq!(positions.last(), Some(&99));
    // The phase survives normalization: -25 and -5 are the same tile grid.
    assert_eq!(
      collect_repeat_tile_positions(30, 10, -25).as_slice(),
      &[-5, 5, 15, 25]
    );
  }

  #[test]
  fn test_parse_background_repeat_invalid() {
    assert!(BackgroundRepeat::from_css_str("123").is_err());
    assert!(BackgroundRepeat::from_css_str("bogus").is_err());
  }
}
