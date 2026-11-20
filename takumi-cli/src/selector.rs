//! `--selector`: the element whose box an image keeps. Approximate: it reads a compound of a type,
//! `#id`, and `.class` selectors, where a browser's `querySelector` takes any selector.

use std::fmt::{self, Display, Formatter};

use takumi::prelude::Bitmap;
use takumi_core::paint_tree::{ElementInfo, PaintDocument};

/// A compound selector, such as `div.card` or `#hero`, matched against the first element in
/// document order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ElementSelector {
  tag_name: Option<String>,
  id: Option<String>,
  class_names: Vec<String>,
}

impl ElementSelector {
  /// `text` as a selector, or why it is not one `--selector` reads.
  pub(crate) fn parse(text: &str) -> Result<Self, String> {
    let invalid = || {
      format!(
        "`{text}` is not a selector `--selector` reads. Use a type, `#id`, and `.class` together, such as `div.card`."
      )
    };
    let is_name = |character: char| character.is_alphanumeric() || matches!(character, '-' | '_');
    let mut selector = Self {
      tag_name: None,
      id: None,
      class_names: Vec::new(),
    };
    let mut rest = text.trim();
    let name_length = rest
      .find(|character| !is_name(character))
      .unwrap_or(rest.len());

    if name_length > 0 {
      selector.tag_name = Some(rest[..name_length].to_ascii_lowercase());
      rest = &rest[name_length..];
    }

    while let Some(marker) = rest.chars().next() {
      let tail = &rest[1..];
      let length = tail
        .find(|character| !is_name(character))
        .unwrap_or(tail.len());

      if length == 0 {
        return Err(invalid());
      }

      let name = tail[..length].to_owned();

      match marker {
        '#' if selector.id.is_none() => selector.id = Some(name),
        '.' => selector.class_names.push(name),
        _ => return Err(invalid()),
      }

      rest = &tail[length..];
    }

    if selector.tag_name.is_none() && selector.id.is_none() && selector.class_names.is_empty() {
      return Err(invalid());
    }

    Ok(selector)
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
      .find(|element| self.matches(element))
      .map(|element| &element.path)
    else {
      return Err(format!("no element matches `--selector {}`.", self));
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

    Ok(bitmap.crop(x as u32, y as u32, (x_end - x) as u32, (y_end - y) as u32))
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
  }

  #[test]
  fn refuses_what_it_cannot_match() {
    for text in ["", "div > p", "a b", "#a#b", ".", "div:hover", "[href]"] {
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
