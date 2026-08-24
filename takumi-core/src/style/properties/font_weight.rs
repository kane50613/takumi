use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case};
use parley::style::FontWeight as ParleyFontWeight;

use crate::style::tw::Namespace;
use crate::style::{
  Animatable, Color, CssSyntaxKind, CssToken, FromCss, MakeComputed, ParseResult, SizingContext,
  ToCss, lerp, tw::TailwindPropertyParser, unexpected_token,
};

/// Represents font weight value.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FontWeight {
  /// An absolute numeric weight (CSS `font-weight`, typically 1-1000).
  Absolute(f32),
  /// One step bolder than the inherited weight, resolved against the parent.
  Bolder,
  /// One step lighter than the inherited weight, resolved against the parent.
  Lighter,
}

impl Default for FontWeight {
  fn default() -> Self {
    FontWeight::from(400.0)
  }
}

// https://drafts.csswg.org/css-fonts-4/#font-weight-prop
fn bolder(weight: f32) -> f32 {
  if weight < 350.0 {
    400.0
  } else if weight < 550.0 {
    700.0
  } else if weight < 900.0 {
    900.0
  } else {
    weight
  }
}

fn lighter(weight: f32) -> f32 {
  if weight < 100.0 {
    weight
  } else if weight < 550.0 {
    100.0
  } else if weight < 750.0 {
    400.0
  } else {
    700.0
  }
}

impl MakeComputed for FontWeight {}

impl Animatable for FontWeight {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    _sizing: &SizingContext,
    _current_color: Color,
  ) {
    *self = FontWeight::from(lerp(from.value(), to.value(), progress));
  }
}

impl<'i> FromCss<'i> for FontWeight {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.next()?;

    match token {
      Token::Number { value, .. } => Ok((*value).into()),
      Token::Ident(ident) => match_ignore_ascii_case! { ident,
        "normal" => Ok(400.0.into()),
        "bold" => Ok(700.0.into()),
        "bolder" => Ok(FontWeight::Bolder),
        "lighter" => Ok(FontWeight::Lighter),
        _ => Err(unexpected_token!(location, token)),
      },
      _ => Err(unexpected_token!(location, token)),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Syntax(CssSyntaxKind::Number),
    CssToken::Keyword("normal"),
    CssToken::Keyword("bold"),
    CssToken::Keyword("bolder"),
    CssToken::Keyword("lighter"),
  ];
}

impl TailwindPropertyParser for FontWeight {
  const NAMESPACES: &'static [Namespace] = &[Namespace::FontWeight];

  fn parse_tw(token: &str) -> Option<Self> {
    if let Ok(value) = token.parse::<f32>() {
      return Some(value.into());
    }

    match_ignore_ascii_case! {&token,
      "thin" => Some(100.0.into()),
      "extralight" => Some(200.0.into()),
      "light" => Some(300.0.into()),
      "normal" => Some(400.0.into()),
      "medium" => Some(500.0.into()),
      "semibold" => Some(600.0.into()),
      "bold" => Some(700.0.into()),
      "extrabold" => Some(800.0.into()),
      "black" => Some(900.0.into()),
      _ => None,
    }
  }
}

impl FontWeight {
  /// The numeric weight (1-1000). Relative keywords fall back to the `normal`.
  pub fn value(self) -> f32 {
    match self {
      Self::Absolute(weight) => weight,
      Self::Bolder => bolder(400.0),
      Self::Lighter => lighter(400.0),
    }
  }

  /// Resolves `bolder`/`lighter` against the inherited parent weight; absolute
  /// weights pass through unchanged.
  pub(crate) fn resolve_against(self, parent: f32) -> Self {
    match self {
      Self::Bolder => FontWeight::from(bolder(parent)),
      Self::Lighter => FontWeight::from(lighter(parent)),
      absolute => absolute,
    }
  }

  pub(crate) fn into_parlance(self) -> ParleyFontWeight {
    ParleyFontWeight::new(self.value())
  }
}

impl From<f32> for FontWeight {
  fn from(value: f32) -> Self {
    FontWeight::Absolute(value)
  }
}

impl ToCss for FontWeight {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Self::Bolder => dest.write_str("bolder"),
      Self::Lighter => dest.write_str("lighter"),
      Self::Absolute(weight) => write!(dest, "{weight}"),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn parses_numeric_font_weight() {
    assert_eq!(FontWeight::from_css_str("700"), Ok(700.0.into()));
  }

  #[test]
  fn resolves_relative_keywords_against_parent() {
    assert_eq!(FontWeight::from_css_str("bolder"), Ok(FontWeight::Bolder));
    assert_eq!(FontWeight::from_css_str("lighter"), Ok(FontWeight::Lighter));

    assert_eq!(FontWeight::Bolder.resolve_against(400.0).value(), 700.0);
    assert_eq!(FontWeight::Bolder.resolve_against(700.0).value(), 900.0);
    assert_eq!(FontWeight::Bolder.resolve_against(900.0).value(), 900.0);
    assert_eq!(FontWeight::Lighter.resolve_against(400.0).value(), 100.0);
    assert_eq!(FontWeight::Lighter.resolve_against(600.0).value(), 400.0);
    assert_eq!(FontWeight::Lighter.resolve_against(900.0).value(), 700.0);
  }
}
