use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case};

use crate::style::{
  Animatable, Color, CssSyntaxKind, CssToken, FromCss, Length, MakeComputed, ParseResult,
  SizingContext, ToCss, unexpected_token,
};

/// Controls indentation of the first line, or hanging/each-line variants.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct TextIndent {
  /// The indent amount.
  pub amount: Length,
  /// Apply the indent after every hard line break.
  pub each_line: bool,
  /// Indent continuation lines instead of the first line.
  pub hanging: bool,
}

impl Default for TextIndent {
  fn default() -> Self {
    Self::new(Length::zero())
  }
}

impl TextIndent {
  /// Creates a text indent with the given amount and default keyword options.
  pub const fn new(amount: Length) -> Self {
    Self {
      amount,
      each_line: false,
      hanging: false,
    }
  }

  /// Sets whether the indent also applies after explicit line breaks.
  pub const fn with_each_line(mut self, each_line: bool) -> Self {
    self.each_line = each_line;
    self
  }

  /// Sets whether continuation lines are indented instead of the first line.
  pub const fn with_hanging(mut self, hanging: bool) -> Self {
    self.hanging = hanging;
    self
  }

  /// Resolves the indent amount to pixels against the line width.
  pub fn resolve_px(self, sizing: &SizingContext, line_width: f32) -> f32 {
    self.amount.to_px(sizing, line_width)
  }
}

impl MakeComputed for TextIndent {}

impl Animatable for TextIndent {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    self
      .amount
      .interpolate(&from.amount, &to.amount, progress, sizing, current_color);
    self.each_line = if progress >= 0.5 {
      to.each_line
    } else {
      from.each_line
    };
    self.hanging = if progress >= 0.5 {
      to.hanging
    } else {
      from.hanging
    };
  }
}

impl<'i> FromCss<'i> for TextIndent {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut amount = None;
    let mut each_line = false;
    let mut hanging = false;

    while !input.is_exhausted() {
      if amount.is_none()
        && let Ok(length) = input.try_parse(Length::from_css)
      {
        amount = Some(length);
        continue;
      }

      let location = input.current_source_location();
      match input.next()? {
        Token::Ident(keyword) => match_ignore_ascii_case! {keyword.as_ref(),
          "each-line" if !each_line => each_line = true,
          "hanging" if !hanging => hanging = true,
          _ => return Err(unexpected_token!(location, &Token::Ident(keyword.clone()))),
        },
        token => return Err(unexpected_token!(location, token)),
      }
    }

    Ok(Self {
      amount: amount.unwrap_or(Length::zero()),
      each_line,
      hanging,
    })
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::Length),
    CssToken::Keyword("each-line"),
    CssToken::Keyword("hanging"),
  ];
}

impl ToCss for TextIndent {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    self.amount.to_css(dest)?;
    if self.each_line {
      dest.write_str(" each-line")?;
    }
    if self.hanging {
      dest.write_str(" hanging")?;
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use crate::style::{FromCssStr, Length, TextIndent};

  #[test]
  fn parses_indent_keywords_in_any_order() {
    assert_eq!(
      TextIndent::from_css_str("hanging 2em each-line"),
      Ok(TextIndent {
        amount: Length::Em(2.0),
        each_line: true,
        hanging: true,
      })
    );
  }

  #[test]
  fn defaults_to_zero_indent() {
    assert_eq!(TextIndent::default().amount, Length::Px(0.0));
  }

  #[test]
  fn keyword_only_keeps_zero_amount() {
    assert_eq!(
      TextIndent::from_css_str("hanging"),
      Ok(TextIndent {
        amount: Length::Px(0.0),
        each_line: false,
        hanging: true,
      })
    );
  }
}
