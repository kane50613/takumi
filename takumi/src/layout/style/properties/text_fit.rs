use cssparser::{Parser, Token};
use typed_builder::TypedBuilder;

use crate::layout::style::{
  Animatable, CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult,
  declare_enum_from_css_impl, unexpected_token,
};

/// Controls whether inline contents are scaled to fit their line box.
#[derive(Debug, Clone, Copy, PartialEq, Default, TypedBuilder)]
#[non_exhaustive]
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
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut mode = None;
    let mut target = None;
    let mut limit = None;

    while !input.is_exhausted() {
      if let Ok(parsed) = input.try_parse(TextFitMode::from_css) {
        if mode.replace(parsed).is_some() {
          return Err(input.new_error_for_next_token());
        }
        continue;
      }

      if let Ok(parsed) = input.try_parse(TextFitTarget::from_css) {
        if target.replace(parsed).is_some() {
          return Err(input.new_error_for_next_token());
        }
        continue;
      }

      let location = input.current_source_location();
      let token = input.next()?;
      match token {
        Token::Percentage { unit_value, .. } if limit.replace(unit_value.max(0.0)).is_none() => {}
        _ => return Err(unexpected_token!(location, token)),
      }
    }

    Ok(Self {
      mode: mode.unwrap_or_default(),
      target: target.unwrap_or_default(),
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
#[non_exhaustive]
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
#[non_exhaustive]
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
