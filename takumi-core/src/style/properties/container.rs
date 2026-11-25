//! The CSS Containment `container-type` and `container-name` properties, which make a box a query
//! container for `@container` rules.

use std::fmt;

use cssparser::{Parser, Token, serialize_identifier};

use crate::style::{
  Animatable, FromCss, MakeComputed, ParseResult, ToCss, impl_css_enum,
  properties::is_reserved_custom_ident, unexpected_token,
};

/// The `container-type` property: which of a box's axes `@container` rules can query, each one
/// size-contained. <https://drafts.csswg.org/css-contain-3/#container-type>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ContainerType {
  /// Not a size query container.
  #[default]
  Normal,
  /// Both axes, each size-contained.
  Size,
  /// The inline axis, inline-size-contained.
  InlineSize,
}

impl_css_enum!(
  ContainerType,
  "normal" => ContainerType::Normal,
  "size" => ContainerType::Size,
  "inline-size" => ContainerType::InlineSize,
);

impl ContainerType {
  /// Whether it contains the box's inline size.
  pub(crate) fn contains_inline_size(self) -> bool {
    matches!(self, Self::Size | Self::InlineSize)
  }

  /// Whether it contains the box's block size.
  pub(crate) fn contains_block_size(self) -> bool {
    self == Self::Size
  }
}

/// The `container-name` property: the names `@container` rules pick a container by, none for
/// `none`. <https://drafts.csswg.org/css-contain-3/#container-name>
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContainerName(pub(crate) Box<[Box<str>]>);

impl ContainerName {
  /// A `<custom-ident>` other than a CSS-wide keyword, `none`, `and`, `or`, or `not`, which
  /// `container-name` and `@container` take a name as.
  pub(crate) fn parse_name<'i>(input: &mut Parser<'i>) -> ParseResult<'i, Box<str>> {
    let token = input.next()?.clone();

    match &token {
      Token::Ident(ident)
        if !is_reserved_custom_ident(ident)
          && !["none", "and", "or", "not"]
            .iter()
            .any(|reserved| ident.eq_ignore_ascii_case(reserved)) =>
      {
        Ok(ident.as_ref().into())
      }
      _ => Err(unexpected_token!(&token)),
    }
  }

  /// Whether one of its names is `name`.
  pub(crate) fn has(&self, name: &str) -> bool {
    self.0.iter().any(|own| own.as_ref() == name)
  }
}

impl MakeComputed for ContainerName {}

impl Animatable for ContainerName {}

impl<'i> FromCss<'i> for ContainerName {
  const VALID_TOKENS: &'static [&'static str] = &["none", "<custom-ident>"];

  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("none"))
      .is_ok()
    {
      return Ok(Self::default());
    }

    let mut names = Vec::new();

    while let Ok(name) = input.try_parse(Self::parse_name) {
      names.push(name);
    }
    if names.is_empty() {
      return Err(unexpected_token!(input.next()?,));
    }
    Ok(Self(names.into()))
  }
}

impl ToCss for ContainerName {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    if self.0.is_empty() {
      return dest.write_str("none");
    }
    for (index, name) in self.0.iter().enumerate() {
      if index > 0 {
        dest.write_str(" ")?;
      }
      serialize_identifier(name, dest)?;
    }
    Ok(())
  }
}

/// The `container` shorthand: `<'container-name'> [ / <'container-type'> ]?`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Container {
  /// The names.
  pub name: ContainerName,
  /// The type, `normal` when left out.
  pub container_type: ContainerType,
}

impl<'i> FromCss<'i> for Container {
  const VALID_TOKENS: &'static [&'static str] = &["none", "<custom-ident>", "/"];

  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let name = ContainerName::from_css(input)?;
    let container_type = if input.try_parse(|input| input.expect_delim('/')).is_ok() {
      ContainerType::from_css(input)?
    } else {
      ContainerType::Normal
    };

    Ok(Self {
      name,
      container_type,
    })
  }
}

impl ToCss for Container {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    self.name.to_css(dest)?;
    if self.container_type != ContainerType::Normal {
      dest.write_str(" / ")?;
      self.container_type.to_css(dest)?;
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn container_takes_names_and_a_type() {
    let container = Container::from_css_str("card sidebar / inline-size").unwrap();

    assert!(container.name.has("card") && container.name.has("sidebar"));
    assert_eq!(container.container_type, ContainerType::InlineSize);
    assert_eq!(
      Container::from_css_str("none").unwrap(),
      Container::default()
    );
    assert!(ContainerName::from_css_str("and").is_err());
    assert_eq!(
      ContainerName::from_css_str(r"foo\ bar")
        .unwrap()
        .to_css_string(),
      r"foo\ bar"
    );
    assert!(ContainerName::from_css_str("").is_err());
  }
}
