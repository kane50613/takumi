//! CSS fragmentation properties: `break-before`, `break-after`, `break-inside`.
//! Only the paged backend consumes them; other backends ignore them.

use crate::style::declare_enum_from_css_impl;

/// A forced-break value for `break-before` / `break-after`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum BreakBetween {
  /// No forced break.
  #[default]
  Auto,
  /// Force a page break on this edge of the box.
  Page,
}

declare_enum_from_css_impl!(
  BreakBetween,
  "auto" => BreakBetween::Auto,
  "page" => BreakBetween::Page
);

/// A `break-inside` value.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum BreakInside {
  /// The box may be split across pages.
  #[default]
  Auto,
  /// Keep the whole box on one page.
  Avoid,
}

declare_enum_from_css_impl!(
  BreakInside,
  "auto" => BreakInside::Auto,
  "avoid" => BreakInside::Avoid
);

/// A `box-decoration-break` value, deciding how box decorations paint across
/// page fragments.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum BoxDecorationBreak {
  /// Decorations paint as if the box were unfragmented, then get sliced: the
  /// edge at a break is open.
  #[default]
  Slice,
  /// Every fragment paints its own complete decorations.
  Clone,
}

declare_enum_from_css_impl!(
  BoxDecorationBreak,
  "slice" => BoxDecorationBreak::Slice,
  "clone" => BoxDecorationBreak::Clone
);

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::properties::traits::FromCssStr;

  #[test]
  fn parses_break_between() {
    assert_eq!(BreakBetween::from_css_str("auto"), Ok(BreakBetween::Auto));
    assert_eq!(BreakBetween::from_css_str("page"), Ok(BreakBetween::Page));
    assert!(BreakBetween::from_css_str("column").is_err());
  }

  #[test]
  fn parses_box_decoration_break() {
    assert_eq!(
      BoxDecorationBreak::from_css_str("slice"),
      Ok(BoxDecorationBreak::Slice)
    );
    assert_eq!(
      BoxDecorationBreak::from_css_str("clone"),
      Ok(BoxDecorationBreak::Clone)
    );
  }

  #[test]
  fn parses_break_inside() {
    assert_eq!(BreakInside::from_css_str("auto"), Ok(BreakInside::Auto));
    assert_eq!(BreakInside::from_css_str("avoid"), Ok(BreakInside::Avoid));
    assert!(BreakInside::from_css_str("avoid-page").is_err());
  }
}
