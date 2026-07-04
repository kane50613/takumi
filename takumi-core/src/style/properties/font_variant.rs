use std::fmt;

use cssparser::{Parser, Token, match_ignore_ascii_case};
use parley::{FontFeature, setting::Tag};

use crate::style::{
  Animatable, CssToken, FromCss, MakeComputed, ParseResult, ToCss, unexpected_token,
};

/// Tri-state for one `font-variant-ligatures` group.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum LigatureState {
  #[default]
  Normal,
  Enabled,
  Disabled,
}

/// `font-variant-ligatures`. swash enables `liga`/`clig`/`calt` by default, so each group
/// only emits a tag when the author overrides it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FontVariantLigatures {
  common: LigatureState,
  discretionary: LigatureState,
  historical: LigatureState,
  contextual: LigatureState,
}

impl FontVariantLigatures {
  fn none() -> Self {
    Self {
      common: LigatureState::Disabled,
      discretionary: LigatureState::Disabled,
      historical: LigatureState::Disabled,
      contextual: LigatureState::Disabled,
    }
  }

  fn set_keyword(&mut self, ident: &str) -> bool {
    match_ignore_ascii_case! { ident,
      "common-ligatures" => self.common = LigatureState::Enabled,
      "no-common-ligatures" => self.common = LigatureState::Disabled,
      "discretionary-ligatures" => self.discretionary = LigatureState::Enabled,
      "no-discretionary-ligatures" => self.discretionary = LigatureState::Disabled,
      "historical-ligatures" => self.historical = LigatureState::Enabled,
      "no-historical-ligatures" => self.historical = LigatureState::Disabled,
      "contextual" => self.contextual = LigatureState::Enabled,
      "no-contextual" => self.contextual = LigatureState::Disabled,
      _ => return false,
    }
    true
  }

  fn append_features(&self, out: &mut Vec<FontFeature>) {
    for (state, tags) in [
      (self.common, &[b"liga", b"clig"][..]),
      (self.discretionary, &[b"dlig"][..]),
      (self.historical, &[b"hlig"][..]),
      (self.contextual, &[b"calt"][..]),
    ] {
      let value = match state {
        LigatureState::Normal => continue,
        LigatureState::Enabled => 1,
        LigatureState::Disabled => 0,
      };
      for tag in tags {
        out.push(FontFeature::new(Tag::new(tag), value));
      }
    }
  }
}

impl MakeComputed for FontVariantLigatures {}
impl Animatable for FontVariantLigatures {}

impl<'i> FromCss<'i> for FontVariantLigatures {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut value = Self::default();
    while !input.is_exhausted() {
      let location = input.current_source_location();
      let ident = input.expect_ident()?;
      match_ignore_ascii_case! { ident,
        "normal" => {},
        "none" => value = Self::none(),
        _ => {
          if !value.set_keyword(ident) {
            return Err(unexpected_token!(location, &Token::Ident(ident.to_owned())));
          }
        },
      }
    }
    Ok(value)
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("normal"),
    CssToken::Keyword("none"),
    CssToken::Keyword("common-ligatures"),
    CssToken::Keyword("no-common-ligatures"),
    CssToken::Keyword("discretionary-ligatures"),
    CssToken::Keyword("no-discretionary-ligatures"),
    CssToken::Keyword("historical-ligatures"),
    CssToken::Keyword("no-historical-ligatures"),
    CssToken::Keyword("contextual"),
    CssToken::Keyword("no-contextual"),
  ];
}

impl ToCss for FontVariantLigatures {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    let mut wrote = false;
    for (state, on, off) in [
      (self.common, "common-ligatures", "no-common-ligatures"),
      (
        self.discretionary,
        "discretionary-ligatures",
        "no-discretionary-ligatures",
      ),
      (
        self.historical,
        "historical-ligatures",
        "no-historical-ligatures",
      ),
      (self.contextual, "contextual", "no-contextual"),
    ] {
      let keyword = match state {
        LigatureState::Normal => continue,
        LigatureState::Enabled => on,
        LigatureState::Disabled => off,
      };
      if wrote {
        dest.write_str(" ")?;
      }
      dest.write_str(keyword)?;
      wrote = true;
    }
    if !wrote {
      dest.write_str("normal")?;
    }
    Ok(())
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum NumericFigure {
  #[default]
  Normal,
  Lining,
  Oldstyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum NumericSpacing {
  #[default]
  Normal,
  Proportional,
  Tabular,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum NumericFraction {
  #[default]
  Normal,
  Diagonal,
  Stacked,
}

/// `font-variant-numeric`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FontVariantNumeric {
  figure: NumericFigure,
  spacing: NumericSpacing,
  fraction: NumericFraction,
  ordinal: bool,
  slashed_zero: bool,
}

impl FontVariantNumeric {
  fn set_keyword(&mut self, ident: &str) -> bool {
    match_ignore_ascii_case! { ident,
      "lining-nums" => self.figure = NumericFigure::Lining,
      "oldstyle-nums" => self.figure = NumericFigure::Oldstyle,
      "proportional-nums" => self.spacing = NumericSpacing::Proportional,
      "tabular-nums" => self.spacing = NumericSpacing::Tabular,
      "diagonal-fractions" => self.fraction = NumericFraction::Diagonal,
      "stacked-fractions" => self.fraction = NumericFraction::Stacked,
      "ordinal" => self.ordinal = true,
      "slashed-zero" => self.slashed_zero = true,
      _ => return false,
    }
    true
  }

  fn append_features(&self, out: &mut Vec<FontFeature>) {
    match self.figure {
      NumericFigure::Normal => {}
      NumericFigure::Lining => out.push(FontFeature::new(Tag::new(b"lnum"), 1)),
      NumericFigure::Oldstyle => out.push(FontFeature::new(Tag::new(b"onum"), 1)),
    }
    match self.spacing {
      NumericSpacing::Normal => {}
      NumericSpacing::Proportional => out.push(FontFeature::new(Tag::new(b"pnum"), 1)),
      NumericSpacing::Tabular => out.push(FontFeature::new(Tag::new(b"tnum"), 1)),
    }
    match self.fraction {
      NumericFraction::Normal => {}
      NumericFraction::Diagonal => out.push(FontFeature::new(Tag::new(b"frac"), 1)),
      NumericFraction::Stacked => out.push(FontFeature::new(Tag::new(b"afrc"), 1)),
    }
    if self.ordinal {
      out.push(FontFeature::new(Tag::new(b"ordn"), 1));
    }
    if self.slashed_zero {
      out.push(FontFeature::new(Tag::new(b"zero"), 1));
    }
  }
}

impl MakeComputed for FontVariantNumeric {}
impl Animatable for FontVariantNumeric {}

impl<'i> FromCss<'i> for FontVariantNumeric {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut value = Self::default();
    while !input.is_exhausted() {
      let location = input.current_source_location();
      let ident = input.expect_ident()?;
      match_ignore_ascii_case! { ident,
        "normal" => {},
        _ => {
          if !value.set_keyword(ident) {
            return Err(unexpected_token!(location, &Token::Ident(ident.to_owned())));
          }
        },
      }
    }
    Ok(value)
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("normal"),
    CssToken::Keyword("lining-nums"),
    CssToken::Keyword("oldstyle-nums"),
    CssToken::Keyword("proportional-nums"),
    CssToken::Keyword("tabular-nums"),
    CssToken::Keyword("diagonal-fractions"),
    CssToken::Keyword("stacked-fractions"),
    CssToken::Keyword("ordinal"),
    CssToken::Keyword("slashed-zero"),
  ];
}

impl ToCss for FontVariantNumeric {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    let mut parts: Vec<&str> = Vec::new();
    match self.figure {
      NumericFigure::Normal => {}
      NumericFigure::Lining => parts.push("lining-nums"),
      NumericFigure::Oldstyle => parts.push("oldstyle-nums"),
    }
    match self.spacing {
      NumericSpacing::Normal => {}
      NumericSpacing::Proportional => parts.push("proportional-nums"),
      NumericSpacing::Tabular => parts.push("tabular-nums"),
    }
    match self.fraction {
      NumericFraction::Normal => {}
      NumericFraction::Diagonal => parts.push("diagonal-fractions"),
      NumericFraction::Stacked => parts.push("stacked-fractions"),
    }
    if self.ordinal {
      parts.push("ordinal");
    }
    if self.slashed_zero {
      parts.push("slashed-zero");
    }
    if parts.is_empty() {
      dest.write_str("normal")
    } else {
      dest.write_str(&parts.join(" "))
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum EastAsianForm {
  #[default]
  Normal,
  Jis78,
  Jis83,
  Jis90,
  Jis04,
  Simplified,
  Traditional,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum EastAsianWidth {
  #[default]
  Normal,
  Full,
  Proportional,
}

/// `font-variant-east-asian`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FontVariantEastAsian {
  form: EastAsianForm,
  width: EastAsianWidth,
  ruby: bool,
}

impl FontVariantEastAsian {
  fn set_keyword(&mut self, ident: &str) -> bool {
    match_ignore_ascii_case! { ident,
      "jis78" => self.form = EastAsianForm::Jis78,
      "jis83" => self.form = EastAsianForm::Jis83,
      "jis90" => self.form = EastAsianForm::Jis90,
      "jis04" => self.form = EastAsianForm::Jis04,
      "simplified" => self.form = EastAsianForm::Simplified,
      "traditional" => self.form = EastAsianForm::Traditional,
      "full-width" => self.width = EastAsianWidth::Full,
      "proportional-width" => self.width = EastAsianWidth::Proportional,
      "ruby" => self.ruby = true,
      _ => return false,
    }
    true
  }

  fn append_features(&self, out: &mut Vec<FontFeature>) {
    match self.form {
      EastAsianForm::Normal => {}
      EastAsianForm::Jis78 => out.push(FontFeature::new(Tag::new(b"jp78"), 1)),
      EastAsianForm::Jis83 => out.push(FontFeature::new(Tag::new(b"jp83"), 1)),
      EastAsianForm::Jis90 => out.push(FontFeature::new(Tag::new(b"jp90"), 1)),
      EastAsianForm::Jis04 => out.push(FontFeature::new(Tag::new(b"jp04"), 1)),
      EastAsianForm::Simplified => out.push(FontFeature::new(Tag::new(b"smpl"), 1)),
      EastAsianForm::Traditional => out.push(FontFeature::new(Tag::new(b"trad"), 1)),
    }
    match self.width {
      EastAsianWidth::Normal => {}
      EastAsianWidth::Full => out.push(FontFeature::new(Tag::new(b"fwid"), 1)),
      EastAsianWidth::Proportional => out.push(FontFeature::new(Tag::new(b"pwid"), 1)),
    }
    if self.ruby {
      out.push(FontFeature::new(Tag::new(b"ruby"), 1));
    }
  }
}

impl MakeComputed for FontVariantEastAsian {}
impl Animatable for FontVariantEastAsian {}

impl<'i> FromCss<'i> for FontVariantEastAsian {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut value = Self::default();
    while !input.is_exhausted() {
      let location = input.current_source_location();
      let ident = input.expect_ident()?;
      match_ignore_ascii_case! { ident,
        "normal" => {},
        _ => {
          if !value.set_keyword(ident) {
            return Err(unexpected_token!(location, &Token::Ident(ident.to_owned())));
          }
        },
      }
    }
    Ok(value)
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("normal"),
    CssToken::Keyword("jis78"),
    CssToken::Keyword("jis83"),
    CssToken::Keyword("jis90"),
    CssToken::Keyword("jis04"),
    CssToken::Keyword("simplified"),
    CssToken::Keyword("traditional"),
    CssToken::Keyword("full-width"),
    CssToken::Keyword("proportional-width"),
    CssToken::Keyword("ruby"),
  ];
}

impl ToCss for FontVariantEastAsian {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    let mut parts: Vec<&str> = Vec::new();
    match self.form {
      EastAsianForm::Normal => {}
      EastAsianForm::Jis78 => parts.push("jis78"),
      EastAsianForm::Jis83 => parts.push("jis83"),
      EastAsianForm::Jis90 => parts.push("jis90"),
      EastAsianForm::Jis04 => parts.push("jis04"),
      EastAsianForm::Simplified => parts.push("simplified"),
      EastAsianForm::Traditional => parts.push("traditional"),
    }
    match self.width {
      EastAsianWidth::Normal => {}
      EastAsianWidth::Full => parts.push("full-width"),
      EastAsianWidth::Proportional => parts.push("proportional-width"),
    }
    if self.ruby {
      parts.push("ruby");
    }
    if parts.is_empty() {
      dest.write_str("normal")
    } else {
      dest.write_str(&parts.join(" "))
    }
  }
}

/// `font-variant-caps`. Each value maps to OpenType caps features; unlike browsers, missing
/// features are not synthesized (e.g. small-caps from scaled capitals), so the result depends
/// on the font shipping `smcp`/`c2sc`/`pcap`/`c2pc`/`unic`/`titl`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FontVariantCaps {
  /// No caps feature.
  #[default]
  Normal,
  /// Small capitals for lowercase (`smcp`).
  SmallCaps,
  /// Small capitals for both lower- and uppercase (`smcp`, `c2sc`).
  AllSmallCaps,
  /// Petite capitals for lowercase (`pcap`).
  PetiteCaps,
  /// Petite capitals for both lower- and uppercase (`pcap`, `c2pc`).
  AllPetiteCaps,
  /// Unicase: small caps for uppercase, normal lowercase (`unic`).
  Unicase,
  /// Titling capitals (`titl`).
  TitlingCaps,
}

impl FontVariantCaps {
  fn from_keyword(ident: &str) -> Option<Self> {
    Some(match_ignore_ascii_case! { ident,
      "normal" => Self::Normal,
      "small-caps" => Self::SmallCaps,
      "all-small-caps" => Self::AllSmallCaps,
      "petite-caps" => Self::PetiteCaps,
      "all-petite-caps" => Self::AllPetiteCaps,
      "unicase" => Self::Unicase,
      "titling-caps" => Self::TitlingCaps,
      _ => return None,
    })
  }

  fn append_features(&self, out: &mut Vec<FontFeature>) {
    match self {
      Self::Normal => {}
      Self::SmallCaps => out.push(FontFeature::new(Tag::new(b"smcp"), 1)),
      Self::AllSmallCaps => {
        out.push(FontFeature::new(Tag::new(b"c2sc"), 1));
        out.push(FontFeature::new(Tag::new(b"smcp"), 1));
      }
      Self::PetiteCaps => out.push(FontFeature::new(Tag::new(b"pcap"), 1)),
      Self::AllPetiteCaps => {
        out.push(FontFeature::new(Tag::new(b"c2pc"), 1));
        out.push(FontFeature::new(Tag::new(b"pcap"), 1));
      }
      Self::Unicase => out.push(FontFeature::new(Tag::new(b"unic"), 1)),
      Self::TitlingCaps => out.push(FontFeature::new(Tag::new(b"titl"), 1)),
    }
  }

  fn keyword(&self) -> &'static str {
    match self {
      Self::Normal => "normal",
      Self::SmallCaps => "small-caps",
      Self::AllSmallCaps => "all-small-caps",
      Self::PetiteCaps => "petite-caps",
      Self::AllPetiteCaps => "all-petite-caps",
      Self::Unicase => "unicase",
      Self::TitlingCaps => "titling-caps",
    }
  }
}

impl MakeComputed for FontVariantCaps {}
impl Animatable for FontVariantCaps {}

impl<'i> FromCss<'i> for FontVariantCaps {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let ident = input.expect_ident()?;
    Self::from_keyword(ident)
      .ok_or_else(|| unexpected_token!(location, &Token::Ident(ident.to_owned())))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("normal"),
    CssToken::Keyword("small-caps"),
    CssToken::Keyword("all-small-caps"),
    CssToken::Keyword("petite-caps"),
    CssToken::Keyword("all-petite-caps"),
    CssToken::Keyword("unicase"),
    CssToken::Keyword("titling-caps"),
  ];
}

impl ToCss for FontVariantCaps {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    dest.write_str(self.keyword())
  }
}

/// `font-variant-position`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FontVariantPosition {
  /// No positional feature.
  #[default]
  Normal,
  /// Subscript forms (`subs`).
  Sub,
  /// Superscript forms (`sups`).
  Super,
}

impl FontVariantPosition {
  fn from_keyword(ident: &str) -> Option<Self> {
    Some(match_ignore_ascii_case! { ident,
      "normal" => Self::Normal,
      "sub" => Self::Sub,
      "super" => Self::Super,
      _ => return None,
    })
  }

  fn append_features(&self, out: &mut Vec<FontFeature>) {
    match self {
      Self::Normal => {}
      Self::Sub => out.push(FontFeature::new(Tag::new(b"subs"), 1)),
      Self::Super => out.push(FontFeature::new(Tag::new(b"sups"), 1)),
    }
  }

  fn keyword(&self) -> &'static str {
    match self {
      Self::Normal => "normal",
      Self::Sub => "sub",
      Self::Super => "super",
    }
  }
}

impl MakeComputed for FontVariantPosition {}
impl Animatable for FontVariantPosition {}

impl<'i> FromCss<'i> for FontVariantPosition {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let ident = input.expect_ident()?;
    Self::from_keyword(ident)
      .ok_or_else(|| unexpected_token!(location, &Token::Ident(ident.to_owned())))
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("normal"),
    CssToken::Keyword("sub"),
    CssToken::Keyword("super"),
  ];
}

impl ToCss for FontVariantPosition {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    dest.write_str(self.keyword())
  }
}

/// Appends every resolved `font-variant-*` feature to `out`, in property order. The caller
/// appends `font-feature-settings` afterwards so explicit settings win on tag conflicts.
pub(crate) fn append_variant_features(
  ligatures: &FontVariantLigatures,
  numeric: &FontVariantNumeric,
  east_asian: &FontVariantEastAsian,
  caps: &FontVariantCaps,
  position: &FontVariantPosition,
  out: &mut Vec<FontFeature>,
) {
  ligatures.append_features(out);
  numeric.append_features(out);
  east_asian.append_features(out);
  caps.append_features(out);
  position.append_features(out);
}

/// The `font-variant` shorthand. Only the subset of values takumi maps to OpenType features is
/// supported; `font-variant-alternates` and `font-variant-emoji` are out of scope.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FontVariant {
  /// `font-variant-ligatures` component.
  pub ligatures: FontVariantLigatures,
  /// `font-variant-numeric` component.
  pub numeric: FontVariantNumeric,
  /// `font-variant-east-asian` component.
  pub east_asian: FontVariantEastAsian,
  /// `font-variant-caps` component.
  pub caps: FontVariantCaps,
  /// `font-variant-position` component.
  pub position: FontVariantPosition,
}

impl<'i> FromCss<'i> for FontVariant {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let mut value = Self::default();
    while !input.is_exhausted() {
      let location = input.current_source_location();
      let ident = input.expect_ident()?;

      if ident.eq_ignore_ascii_case("normal") {
        continue;
      }
      if ident.eq_ignore_ascii_case("none") {
        value.ligatures = FontVariantLigatures::none();
        continue;
      }
      if value.ligatures.set_keyword(ident)
        || value.numeric.set_keyword(ident)
        || value.east_asian.set_keyword(ident)
      {
        continue;
      }
      if let Some(caps) = FontVariantCaps::from_keyword(ident) {
        value.caps = caps;
        continue;
      }
      if let Some(position) = FontVariantPosition::from_keyword(ident) {
        value.position = position;
        continue;
      }
      return Err(unexpected_token!(location, &Token::Ident(ident.to_owned())));
    }
    Ok(value)
  }

  const VALID_TOKENS: &'static [CssToken] = &VARIANT_TOKENS;
}

const VARIANT_TOKEN_LISTS: &[&[CssToken]] = &[
  FontVariantLigatures::VALID_TOKENS,
  FontVariantNumeric::VALID_TOKENS,
  FontVariantEastAsian::VALID_TOKENS,
  FontVariantCaps::VALID_TOKENS,
  FontVariantPosition::VALID_TOKENS,
];

const VARIANT_TOKENS: [CssToken; CssToken::merged_len(VARIANT_TOKEN_LISTS)] =
  CssToken::merge_lists(VARIANT_TOKEN_LISTS);
