use std::fmt;

use cssparser::{Parser, Token};
use typed_builder::TypedBuilder;

use crate::style::{
  Animatable, CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, ToCss,
  declare_enum_from_css_impl, unexpected_token,
};

/// Controls whether inline contents are scaled to fit their line box.
#[derive(Debug, Clone, Copy, PartialEq, Default, TypedBuilder)]
#[builder(field_defaults(default))]
pub struct TextFit {
  /// Selects whether fitting grows, shrinks, or leaves text unchanged.
  pub mode: TextFitMode,
  /// Selects whether fitting uses one scale or per-line scales.
  pub target: TextFitTarget,
  /// Optional scale clamp as a multiplier, parsed from a CSS percentage.
  pub limit: Option<f32>,
}

impl MakeComputed for TextFit {}
impl Animatable for TextFit {}

impl<'i> FromCss<'i> for TextFit {
  // Syntax: [ none | grow | shrink ] [ consistent | per-line | per-line-all ]? <percentage>?
  // The type keyword is mandatory and must appear first, matching the spec and Chromium.
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mode = TextFitMode::from_css(input)?;

    let target = input.try_parse(TextFitTarget::from_css).unwrap_or_default();

    let limit: Option<f32> = input
      .try_parse(|input| -> ParseResult<'i, f32> {
        let location = input.current_source_location();
        match input.next()? {
          Token::Percentage { unit_value, .. } => Ok(unit_value.max(0.0)),
          token => Err(unexpected_token!(location, token)),
        }
      })
      .ok();

    // Reject trailing tokens (e.g. duplicate target or two percentages).
    if !input.is_exhausted() {
      return Err(input.new_error_for_next_token());
    }

    Ok(Self {
      mode,
      target,
      limit,
    })
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("none"),
    CssToken::Keyword("grow"),
    CssToken::Keyword("shrink"),
    CssToken::Keyword("consistent"),
    CssToken::Keyword("per-line"),
    CssToken::Keyword("per-line-all"),
    CssToken::Syntax(CssSyntaxKind::Percentage),
  ];
}

/// Text fitting direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextFitMode {
  /// Do not scale inline contents.
  #[default]
  None,
  /// Scale inline contents up to fit.
  Grow,
  /// Scale inline contents down to fit.
  Shrink,
}

declare_enum_from_css_impl!(
  TextFitMode,
  "none" => TextFitMode::None,
  "grow" => TextFitMode::Grow,
  "shrink" => TextFitMode::Shrink,
);

/// Text fitting scale target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextFitTarget {
  /// Use one scale for all lines.
  #[default]
  Consistent,
  /// Use a separate scale for each line except the last.
  PerLine,
  /// Use a separate scale for each line including the last.
  PerLineAll,
}

declare_enum_from_css_impl!(
  TextFitTarget,
  "consistent" => TextFitTarget::Consistent,
  "per-line" => TextFitTarget::PerLine,
  "per-line-all" => TextFitTarget::PerLineAll,
);

impl ToCss for TextFit {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    self.mode.to_css(dest)?;
    if self.target != TextFitTarget::Consistent {
      dest.write_char(' ')?;
      self.target.to_css(dest)?;
    }
    if let Some(limit) = self.limit {
      dest.write_char(' ')?;
      write!(dest, "{}%", limit * 100.0)?;
    }
    Ok(())
  }
}
