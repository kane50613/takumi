use cssparser::{Parser, match_ignore_ascii_case};
use typed_builder::TypedBuilder;

use crate::layout::style::{
  CssDescriptorKind, CssToken, FromCss, MakeComputed, ParseResult, declare_enum_from_css_impl,
  tw::TailwindPropertyParser,
};

/// Controls how text should be wrapped.
/// Construct with [`TextWrap::builder`].
#[derive(Debug, Clone, Copy, PartialEq, Default, TypedBuilder)]
#[non_exhaustive]
#[builder(field_defaults(default))]
pub struct TextWrap {
  /// Controls whether text should be wrapped.
  pub mode: TextWrapMode,
  /// Controls the style of text wrapping.
  pub style: TextWrapStyle,
}

impl MakeComputed for TextWrap {}

impl TailwindPropertyParser for TextWrap {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "wrap" => Some(TextWrap {
        mode: TextWrapMode::Wrap,
        style: TextWrapStyle::default(),
      }),
      "nowrap" => Some(TextWrap {
        mode: TextWrapMode::NoWrap,
        style: TextWrapStyle::default(),
      }),
      "balance" => Some(TextWrap {
        mode: TextWrapMode::default(),
        style: TextWrapStyle::Balance,
      }),
      "pretty" => Some(TextWrap {
        mode: TextWrapMode::default(),
        style: TextWrapStyle::Pretty,
      }),
      _ => None,
    }
  }
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

    Ok(TextWrap {
      mode: mode.unwrap_or_default(),
      style,
    })
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Descriptor(CssDescriptorKind::TextWrapMode),
    CssToken::Descriptor(CssDescriptorKind::TextWrapStyle),
  ];
}

/// Controls whether text should be wrapped.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
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
#[non_exhaustive]
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
