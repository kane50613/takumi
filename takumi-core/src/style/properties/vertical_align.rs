use std::fmt;

use cssparser::Parser;
use parley::LineMetrics;

use crate::style::{SizingContext, ToCss, tw::TailwindPropertyParser, *};

/// Keyword values for the CSS `vertical-align` property.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum VerticalAlignKeyword {
  /// Aligns the baseline of the box with the baseline of the parent box.
  #[default]
  Baseline,
  /// Aligns the top of the box with the top of the line box.
  Top,
  /// Aligns the middle of the box with the baseline of the parent box plus half the x-height of the parent.
  Middle,
  /// Aligns the bottom of the box with the bottom of the line box.
  Bottom,
  /// Aligns the top of the box with the top of the parent's font.
  TextTop,
  /// Aligns the bottom of the box with the bottom of the parent's font.
  TextBottom,
  /// Aligns the baseline of the box with the subscript-baseline of the parent box.
  Sub,
  /// Aligns the baseline of the box with the superscript-baseline of the parent box.
  Super,
}

declare_enum_from_css_impl!(
  VerticalAlignKeyword,
  "baseline" => VerticalAlignKeyword::Baseline,
  "top" => VerticalAlignKeyword::Top,
  "middle" => VerticalAlignKeyword::Middle,
  "bottom" => VerticalAlignKeyword::Bottom,
  "text-top" => VerticalAlignKeyword::TextTop,
  "text-bottom" => VerticalAlignKeyword::TextBottom,
  "sub" => VerticalAlignKeyword::Sub,
  "super" => VerticalAlignKeyword::Super
);

/// Defines the vertical alignment of an inline-level box.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum VerticalAlign {
  /// A keyword-based alignment mode.
  Keyword(VerticalAlignKeyword),
  /// A baseline shift in `<length-percentage>` form.
  Length(Length),
}

impl ToCss for VerticalAlign {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Keyword(kw) => kw.to_css(dest),
      Self::Length(l) => l.to_css(dest),
    }
  }
}

/// Computed `vertical-align` data used for placement.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum ResolvedVerticalAlign {
  /// A keyword-based alignment mode.
  Keyword(VerticalAlignKeyword),
  /// A baseline shift resolved as: `px + line_height_relative * metrics_line_height`.
  BaselineShift {
    /// The absolute pixel component added to the final baseline shift.
    px: f32,
    /// The multiplier applied to metrics-derived line height.
    line_height_relative: f32,
  },
}

impl Default for VerticalAlign {
  fn default() -> Self {
    Self::Keyword(VerticalAlignKeyword::default())
  }
}

impl Default for ResolvedVerticalAlign {
  fn default() -> Self {
    Self::Keyword(VerticalAlignKeyword::default())
  }
}

impl<'i> FromCss<'i> for VerticalAlign {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    if let Ok(keyword) = input.try_parse(VerticalAlignKeyword::from_css) {
      return Ok(Self::Keyword(keyword));
    }

    Ok(Self::Length(Length::from_css(input)?))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("baseline"),
    CssToken::Keyword("top"),
    CssToken::Keyword("middle"),
    CssToken::Keyword("bottom"),
    CssToken::Keyword("text-top"),
    CssToken::Keyword("text-bottom"),
    CssToken::Keyword("sub"),
    CssToken::Keyword("super"),
    CssToken::Syntax(CssSyntaxKind::Length),
  ];
}

impl VerticalAlign {
  /// Resolves the keyword or length into a `ResolvedVerticalAlign`.
  pub fn resolve(
    self,
    sizing: &SizingContext,
    font_size: f32,
    line_height: LineHeight,
  ) -> ResolvedVerticalAlign {
    match self {
      // Font-size fraction, independent of line-height (CSS sub/super, per Chromium).
      Self::Keyword(VerticalAlignKeyword::Super) => ResolvedVerticalAlign::BaselineShift {
        px: font_size / 3.0 + 1.0,
        line_height_relative: 0.0,
      },
      Self::Keyword(VerticalAlignKeyword::Sub) => ResolvedVerticalAlign::BaselineShift {
        px: -(font_size / 5.0 + 1.0),
        line_height_relative: 0.0,
      },
      Self::Keyword(keyword) => ResolvedVerticalAlign::Keyword(keyword),
      Self::Length(length) => {
        if line_height == LineHeight::Normal {
          if let Length::Percentage(value) = length {
            return ResolvedVerticalAlign::BaselineShift {
              px: 0.0,
              line_height_relative: value / 100.0,
            };
          }

          if let Length::Calc(formula) = length {
            let linear = formula.resolve(sizing);
            let (px, percent) = linear.components();

            return ResolvedVerticalAlign::BaselineShift {
              px,
              line_height_relative: percent,
            };
          }
        }

        let shift = match line_height {
          LineHeight::Normal => length.to_px(sizing, 0.0),
          LineHeight::Unitless(value) => length.to_px(sizing, value * font_size),
          LineHeight::Length(value) => length.to_px(sizing, value.to_px(sizing, font_size)),
        };
        ResolvedVerticalAlign::BaselineShift {
          px: shift,
          line_height_relative: 0.0,
        }
      }
    }
  }
}

impl MakeComputed for VerticalAlign {
  fn make_computed(&mut self, sizing: &SizingContext) {
    if let Self::Length(length) = self {
      length.make_computed(sizing);
    }
  }
}

impl ResolvedVerticalAlign {
  /// Writes the aligned `y` for a box on a line given its metrics.
  pub(crate) fn apply(
    self,
    y: &mut f32,
    metrics: &LineMetrics,
    box_height: f32,
    baseline_offset: Option<f32>,
    parent_x_height: Option<f32>,
    parent_text_metrics: Option<(f32, f32)>,
  ) {
    let baseline_offset = baseline_offset.unwrap_or(box_height).clamp(0.0, box_height);
    let baseline_top = metrics.baseline - baseline_offset;
    let (parent_text_ascent, parent_text_descent) =
      parent_text_metrics.unwrap_or((metrics.ascent, metrics.descent));

    match self {
      ResolvedVerticalAlign::Keyword(keyword) => match keyword {
        VerticalAlignKeyword::Baseline => *y = baseline_top,
        VerticalAlignKeyword::Top => {
          debug_assert!(metrics.block_min_coord.is_finite());
          *y = metrics.block_min_coord;
        }
        VerticalAlignKeyword::Middle => {
          let x_height = parent_x_height.unwrap_or(metrics.ascent * 0.5);
          *y = metrics.baseline - (x_height * 0.5) - (box_height / 2.0);
        }
        VerticalAlignKeyword::Bottom => {
          debug_assert!(metrics.block_max_coord.is_finite());
          *y = metrics.block_max_coord - box_height;
        }
        VerticalAlignKeyword::TextTop => *y = metrics.baseline - parent_text_ascent,
        VerticalAlignKeyword::TextBottom => {
          *y = metrics.baseline + parent_text_descent - box_height
        }
        // resolved to a `BaselineShift` in `VerticalAlign::resolve`
        VerticalAlignKeyword::Sub | VerticalAlignKeyword::Super => unreachable!(),
      },
      ResolvedVerticalAlign::BaselineShift {
        px,
        line_height_relative,
      } => {
        let line_height_component =
          (metrics.ascent - metrics.descent + metrics.leading) * line_height_relative;
        *y = baseline_top - (px + line_height_component);
      }
    }
  }
}

impl TailwindPropertyParser for VerticalAlign {
  fn parse_tw(token: &str) -> Option<Self> {
    VerticalAlignKeyword::from_css_str(token)
      .ok()
      .map(Self::Keyword)
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::*;
  use crate::{geometry::Size, viewport::Viewport};

  fn sizing() -> SizingContext {
    SizingContext {
      viewport: Viewport {
        size: (200, 100).into(),
        font_size: 16.0,
        device_pixel_ratio: 2.0,
      },
      container_size: Size::NONE,
      container_read: Default::default(),
      font_size: 10.0,
      root_font_size: None,
      line_height: 0.0,
      root_line_height: None,
      calc_arena: Rc::new(CalcArena::default()),
    }
  }

  fn line_metrics() -> LineMetrics {
    LineMetrics {
      ascent: 9.0,
      descent: 3.0,
      leading: 0.0,
      line_height: 14.0,
      baseline: 20.0,
      offset: 0.0,
      advance: 100.0,
      trailing_whitespace: 0.0,
      inline_min_coord: 0.0,
      inline_max_coord: 100.0,
      block_min_coord: 10.0,
      block_max_coord: 24.0,
    }
  }

  #[test]
  fn parse_keywords_and_length_percentage() {
    assert_eq!(
      VerticalAlign::from_css_str("baseline"),
      Ok(VerticalAlign::Keyword(VerticalAlignKeyword::Baseline))
    );
    assert_eq!(
      VerticalAlign::from_css_str("10px"),
      Ok(VerticalAlign::Length(Length::Px(10.0)))
    );
    assert_eq!(
      VerticalAlign::from_css_str("25%"),
      Ok(VerticalAlign::Length(Length::Percentage(25.0)))
    );
    assert_eq!(
      VerticalAlign::from_css_str("-0.5em"),
      Ok(VerticalAlign::Length(Length::Em(-0.5)))
    );
  }

  #[test]
  fn resolve_length_to_baseline_shift_px() {
    let resolved =
      VerticalAlign::Length(Length::Px(8.0)).resolve(&sizing(), 12.0, LineHeight::Unitless(1.5));
    assert_eq!(
      resolved,
      ResolvedVerticalAlign::BaselineShift {
        px: 16.0,
        line_height_relative: 0.0
      }
    );
  }

  #[test]
  fn resolve_percentage_uses_line_height_basis() {
    let unitless = VerticalAlign::Length(Length::Percentage(50.0)).resolve(
      &sizing(),
      12.0,
      LineHeight::Unitless(2.0),
    );
    assert_eq!(
      unitless,
      ResolvedVerticalAlign::BaselineShift {
        px: 12.0,
        line_height_relative: 0.0
      }
    );

    let fixed = VerticalAlign::Length(Length::Percentage(50.0)).resolve(
      &sizing(),
      12.0,
      LineHeight::Length(Length::Px(20.0)),
    );
    assert_eq!(
      fixed,
      ResolvedVerticalAlign::BaselineShift {
        px: 20.0,
        line_height_relative: 0.0
      }
    );

    let normal =
      VerticalAlign::Length(Length::Percentage(50.0)).resolve(&sizing(), 12.0, LineHeight::Normal);
    assert_eq!(
      normal,
      ResolvedVerticalAlign::BaselineShift {
        px: 0.0,
        line_height_relative: 0.5
      }
    );
  }

  #[test]
  fn apply_baseline_shift_raises_and_lowers() {
    let metrics = line_metrics();
    let baseline = metrics.baseline - 4.0;

    let mut y = 0.0;
    ResolvedVerticalAlign::BaselineShift {
      px: 5.0,
      line_height_relative: 0.0,
    }
    .apply(&mut y, &metrics, 4.0, None, None, None);
    assert_eq!(y, baseline - 5.0);

    ResolvedVerticalAlign::BaselineShift {
      px: -5.0,
      line_height_relative: 0.0,
    }
    .apply(&mut y, &metrics, 4.0, None, None, None);
    assert_eq!(y, baseline + 5.0);
  }

  #[test]
  fn apply_keyword_top_uses_block_min_coord() {
    let mut y = 0.0;
    let metrics = line_metrics();

    ResolvedVerticalAlign::Keyword(VerticalAlignKeyword::Top)
      .apply(&mut y, &metrics, 8.0, None, None, None);

    assert_eq!(y, metrics.block_min_coord);
  }

  #[test]
  fn apply_keyword_bottom_uses_block_max_coord() {
    let mut y = 0.0;
    let metrics = line_metrics();
    let box_height = 8.0;

    ResolvedVerticalAlign::Keyword(VerticalAlignKeyword::Bottom)
      .apply(&mut y, &metrics, box_height, None, None, None);

    assert_eq!(y, metrics.block_max_coord - box_height);
  }

  #[test]
  fn apply_metrics_relative_shift_uses_line_metrics_formula() {
    let metrics = line_metrics();
    let baseline = metrics.baseline - 4.0;
    let mut y = 0.0;

    ResolvedVerticalAlign::BaselineShift {
      px: 0.0,
      line_height_relative: 0.5,
    }
    .apply(&mut y, &metrics, 4.0, None, None, None);
    assert_eq!(
      y,
      baseline - ((metrics.ascent - metrics.descent + metrics.leading) * 0.5)
    );
  }

  #[test]
  fn resolve_normal_calc_percentage_uses_metrics_relative_px() {
    let Ok(length) = Length::from_css_str("calc(50% + 4px)") else {
      return;
    };
    let resolved = VerticalAlign::Length(length).resolve(&sizing(), 12.0, LineHeight::Normal);
    assert_eq!(
      resolved,
      ResolvedVerticalAlign::BaselineShift {
        px: 8.0,
        line_height_relative: 0.5
      }
    );
  }

  #[test]
  fn apply_metrics_relative_px_shift_uses_line_metrics_formula_plus_px() {
    let metrics = line_metrics();
    let baseline = metrics.baseline - 4.0;
    let mut y = 0.0;

    ResolvedVerticalAlign::BaselineShift {
      px: 3.0,
      line_height_relative: 0.5,
    }
    .apply(&mut y, &metrics, 4.0, None, None, None);
    assert_eq!(
      y,
      baseline - ((metrics.ascent - metrics.descent + metrics.leading) * 0.5 + 3.0)
    );
  }

  #[test]
  fn keyword_apply_matches_previous_baseline_behavior() {
    let metrics = line_metrics();
    let mut y = 0.0;
    ResolvedVerticalAlign::Keyword(VerticalAlignKeyword::Baseline)
      .apply(&mut y, &metrics, 4.0, None, None, None);
    assert_eq!(y, metrics.baseline - 4.0);
  }

  #[test]
  fn keyword_apply_uses_inline_box_baseline_when_available() {
    let metrics = line_metrics();
    let mut y = 0.0;
    ResolvedVerticalAlign::Keyword(VerticalAlignKeyword::Baseline).apply(
      &mut y,
      &metrics,
      20.0,
      Some(12.0),
      None,
      None,
    );
    assert_eq!(y, metrics.baseline - 12.0);
  }

  #[test]
  fn sub_and_super_shift_by_font_size_fraction() {
    let metrics = line_metrics();
    let font_size = 40.0;

    let sub = VerticalAlign::Keyword(VerticalAlignKeyword::Sub).resolve(
      &sizing(),
      font_size,
      LineHeight::Normal,
    );
    assert_eq!(
      sub,
      ResolvedVerticalAlign::BaselineShift {
        px: -(font_size / 5.0 + 1.0),
        line_height_relative: 0.0,
      }
    );
    let mut sub_y = 0.0;
    sub.apply(&mut sub_y, &metrics, 20.0, Some(12.0), None, None);
    assert_eq!(sub_y, (metrics.baseline - 12.0) + (font_size / 5.0 + 1.0));

    let sup = VerticalAlign::Keyword(VerticalAlignKeyword::Super).resolve(
      &sizing(),
      font_size,
      LineHeight::Normal,
    );
    let mut super_y = 0.0;
    sup.apply(&mut super_y, &metrics, 20.0, Some(12.0), None, None);
    assert_eq!(super_y, (metrics.baseline - 12.0) - (font_size / 3.0 + 1.0));
  }

  #[test]
  fn sub_and_super_shift_is_line_height_independent() {
    let font_size = 40.0;
    for keyword in [VerticalAlignKeyword::Sub, VerticalAlignKeyword::Super] {
      let normal =
        VerticalAlign::Keyword(keyword).resolve(&sizing(), font_size, LineHeight::Normal);
      let unitless =
        VerticalAlign::Keyword(keyword).resolve(&sizing(), font_size, LineHeight::Unitless(2.0));
      assert_eq!(normal, unitless);
    }
  }
}
