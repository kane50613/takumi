//! `--selector`: the element whose box an image keeps. Approximate: it reads a compound of a type,
//! `#id`, and `.class` selectors, where a browser's `querySelector` takes any selector, and it picks
//! the first matching element that paints a box, which an inline element without a background or
//! border does not, so a later match can stand in for it.

use std::fmt::{self, Display, Formatter};

use cssparser::{Parser, Token};
use takumi::prelude::Bitmap;
use takumi_core::paint_tree::{ElementInfo, PaintDocument};

/// A compound selector, such as `div.card` or `#hero`, matched against the first element in
/// document order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ElementSelector {
  tag_name: Option<String>,
  id: Option<String>,
  class_names: Vec<String>,
}

impl ElementSelector {
  /// `text` as a selector, or why it is not one `--selector` reads.
  pub(crate) fn parse(text: &str) -> Result<Self, String> {
    Self::read(&mut Parser::new(text))
      .filter(|selector| *selector != Self::default())
      .ok_or_else(|| {
        format!("`{text}` is not a selector `--selector` reads. Use a type, `#id`, and `.class` together, such as `div.card`.")
      })
  }

  /// The compound `parser` holds, with CSS whitespace around it and nothing else.
  fn read(parser: &mut Parser<'_>) -> Option<Self> {
    let mut selector = Self::default();

    parser.skip_whitespace();

    if let Ok(name) = parser.try_parse(Parser::expect_ident_cloned) {
      selector.tag_name = Some(name.to_ascii_lowercase());
    }

    loop {
      match parser.next_including_whitespace() {
        Ok(Token::IDHash(name)) if selector.id.is_none() => selector.id = Some(name.to_string()),
        Ok(Token::Delim('.')) => match parser.next_including_whitespace() {
          Ok(Token::Ident(name)) => selector.class_names.push(name.to_string()),
          _ => return None,
        },
        Ok(Token::WhiteSpace(_)) => {
          parser.skip_whitespace();

          return parser.is_exhausted().then_some(selector);
        }
        Err(_) => return Some(selector),
        Ok(_) => return None,
      }
    }
  }

  fn matches(&self, element: &ElementInfo) -> bool {
    let classes = element.class_name.as_deref().unwrap_or_default();

    self.tag_name.as_deref().is_none_or(|tag_name| {
      element
        .tag_name
        .as_deref()
        .is_some_and(|name| name.eq_ignore_ascii_case(tag_name))
    }) && self
      .id
      .as_deref()
      .is_none_or(|id| element.id.as_deref() == Some(id))
      && self.class_names.iter().all(|class_name| {
        classes
          .split_ascii_whitespace()
          .any(|name| name == class_name)
      })
  }

  /// The part of `bitmap` the first matching element in `document` covers, or why there is none.
  pub(crate) fn crop(&self, document: &PaintDocument, bitmap: &Bitmap) -> Result<Bitmap, String> {
    let Some(path) = document
      .nodes
      .iter()
      .filter_map(|node| node.element.as_ref())
      .filter(|element| self.matches(element))
      .map(|element| &element.path)
      .min()
    else {
      return Err(format!(
        "no element matches `--selector {self}`. An inline element with no background or border draws no box of its own, so give it `display: inline-block`."
      ));
    };
    let (left, top, right, bottom) = document
      .nodes
      .iter()
      .filter(|node| {
        node
          .element
          .as_ref()
          .is_some_and(|element| &element.path == path)
      })
      .fold(
        (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
        |(left, top, right, bottom), node| {
          let bounds = &node.bounds;

          (
            left.min(bounds.x),
            top.min(bounds.y),
            right.max(bounds.x + bounds.width),
            bottom.max(bounds.y + bounds.height),
          )
        },
      );
    let clamp = |value: f32, limit: u32| value.clamp(0.0, limit as f32);
    let (x, y) = (
      clamp(left.floor(), bitmap.width()),
      clamp(top.floor(), bitmap.height()),
    );
    let (x_end, y_end) = (
      clamp(right.ceil(), bitmap.width()),
      clamp(bottom.ceil(), bitmap.height()),
    );

    if x_end <= x || y_end <= y {
      return Err(format!(
        "`--selector {}` matches an element with no area on the canvas.",
        self
      ));
    }

    bitmap
      .crop(x as u32, y as u32, (x_end - x) as u32, (y_end - y) as u32)
      .ok_or_else(|| format!("`--selector {self}` matches an element outside the canvas."))
  }
}

impl Display for ElementSelector {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    if let Some(tag_name) = &self.tag_name {
      formatter.write_str(tag_name)?;
    }
    if let Some(id) = &self.id {
      write!(formatter, "#{id}")?;
    }
    for class_name in &self.class_names {
      write!(formatter, ".{class_name}")?;
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reads_a_compound_selector() {
    let selector = ElementSelector::parse("Section#pro.plan.card").unwrap();

    assert_eq!(selector.to_string(), "section#pro.plan.card");
    assert_eq!(
      ElementSelector::parse(".card").unwrap().to_string(),
      ".card"
    );
    assert_eq!(
      ElementSelector::parse(" .w-1\\/2\t").unwrap().class_names,
      ["w-1/2"]
    );
    assert_eq!(
      ElementSelector::parse("#cafe\u{301}")
        .unwrap()
        .id
        .as_deref(),
      Some("cafe\u{301}")
    );
    assert_eq!(
      ElementSelector::parse(".card\u{a0}").unwrap().class_names,
      ["card\u{a0}"]
    );
  }

  #[test]
  fn refuses_what_it_cannot_match() {
    for text in [
      "",
      "div > p",
      "a b",
      "#a#b",
      ".",
      "div:hover",
      "[href]",
      ".123",
      "#123",
    ] {
      assert!(ElementSelector::parse(text).is_err(), "{text}");
    }
  }

  #[test]
  fn matches_every_part_of_the_compound() {
    let element = ElementInfo {
      id: Some("pro".into()),
      tag_name: Some("section".into()),
      class_name: Some("pricing-card plan".into()),
      path: vec![0],
    };
    let matches = |text: &str| ElementSelector::parse(text).unwrap().matches(&element);

    assert!(matches("section"));
    assert!(matches("#pro.plan"));
    assert!(matches("section.pricing-card.plan"));
    assert!(!matches("div.plan"));
    assert!(!matches(".pricing"));
  }
}
