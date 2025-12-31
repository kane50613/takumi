use cssparser::Parser;

use crate::layout::style::{CssToken, FromCss, ParseResult, declare_enum_from_css_impl};

/// Controls how text should be wrapped.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextWrap {
  /// Controls whether text should be wrapped.
  /// Marking it as optional since it can also be set by `white-space`.
  pub mode: Option<TextWrapMode>,
  /// Controls the style of text wrapping.
  pub style: TextWrapStyle,
}

impl<'i> FromCss<'i> for TextWrap {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut mode = None;
    let mut style = TextWrapStyle::default();

    while !input.is_exhausted() {
      if let Ok(parsed) = input.try_parse(TextWrapMode::from_css) {
        mode = Some(parsed);
        continue;
      }

      if let Ok(parsed) = input.try_parse(TextWrapStyle::from_css) {
        style = parsed;
        continue;
      }

      return Err(input.new_error_for_next_token());
    }

    Ok(TextWrap { mode, style })
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Token("text-wrap-mode"),
      CssToken::Token("text-wrap-style"),
    ]
  }
}

/// Controls whether text should be wrapped.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TextWrapMode {
  /// Text is wrapped across lines at appropriate characters to minimize overflow.
  #[default]
  Wrap,
  /// Text does not wrap across lines. It will overflow its containing element rather than breaking onto a new line.
  NoWrap,
}

impl From<TextWrapMode> for parley::TextWrapMode {
  fn from(value: TextWrapMode) -> Self {
    match value {
      TextWrapMode::Wrap => parley::TextWrapMode::Wrap,
      TextWrapMode::NoWrap => parley::TextWrapMode::NoWrap,
    }
  }
}

declare_enum_from_css_impl!(
  TextWrapMode,
  "wrap" => TextWrapMode::Wrap,
  "nowrap" => TextWrapMode::NoWrap,
);

/// Controls the style of text wrapping.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TextWrapStyle {
  /// Text is wrapped in the default way.
  #[default]
  Auto,
  /// Use binary search to find the minimum width that maintains the same number of lines.
  Balance,
  /// Try to avoid orphans (single short words on the last line) by adjusting line breaks.
  Pretty,
}

declare_enum_from_css_impl!(
  TextWrapStyle,
  "auto" => TextWrapStyle::Auto,
  "balance" => TextWrapStyle::Balance,
  "pretty" => TextWrapStyle::Pretty,
);
