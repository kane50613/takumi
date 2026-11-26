//! HTML's [directionality](https://html.spec.whatwg.org/multipage/dom.html#the-directionality) of
//! an element and the style its `dir` attribute maps to, after Blink's `HTMLElement`.

use super::{Node, NodeKind};
use crate::{
  form_control::{InputType, is_element},
  style::{Direction, FromCssStr, UnicodeBidi},
  text::bidi_paragraph::BidiParagraph,
};

/// The declarations an element's `dir` attribute and tag give the cascade.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct DirectionStyle {
  /// html.css's `[dir=ltr i], [dir=rtl i], [dir=auto i] { unicode-bidi: isolate }`.
  pub(crate) isolate: bool,
  /// html.css's `textarea[dir=auto i], pre[dir=auto i] { unicode-bidi: plaintext }`.
  pub(crate) plaintext: bool,
  /// The `unicode-bidi` presentational hint `dir=auto` maps to, as Blink's
  /// `UnicodeBidiAttributeForDirAuto` has it: `plaintext` for a search, tel, url, or email input,
  /// and `isolate` elsewhere but `bdo`, `textarea`, and `pre`, which html.css covers.
  pub(crate) auto_unicode_bidi: Option<UnicodeBidi>,
  /// The `direction` presentational hint: the attribute's, the text's under auto, or `ltr` for a
  /// `<body>` whose `dir` is invalid.
  pub(crate) direction: Option<Direction>,
}

impl DirectionStyle {
  /// Whether it declares nothing.
  pub(crate) fn is_empty(&self) -> bool {
    *self == Self::default()
  }
}

impl Node {
  /// The style the element's `dir` maps to, from Blink's `CollectStyleForPresentationAttribute`
  /// and `MatchPresentationalHints`.
  pub(crate) fn direction_style(&self) -> DirectionStyle {
    let auto = self.has_dir_auto_attribute();
    let is_any = |tags: &[&str]| tags.iter().any(|tag| is_element(self, tag));
    let direction = self.own_directionality().or_else(|| {
      (self.attribute("dir").is_some() && is_element(self, "body")).then_some(Direction::Ltr)
    });

    let plaintext_input = matches!(
      InputType::of(self),
      Some(InputType::Search | InputType::Tel | InputType::Url | InputType::Email)
    );
    let auto_unicode_bidi = match () {
      _ if !auto || is_any(&["bdo", "textarea", "pre"]) => None,
      _ if plaintext_input => Some(UnicodeBidi::Plaintext),
      _ => Some(UnicodeBidi::Isolate),
    };

    DirectionStyle {
      isolate: self.has_valid_dir_attribute(),
      plaintext: auto && is_any(&["textarea", "pre"]),
      auto_unicode_bidi,
      direction,
    }
  }

  /// The element's directionality, `None` when it takes its parent's. Approximate: an `<input>`
  /// reads its text as any element does, not its value.
  pub(crate) fn own_directionality(&self) -> Option<Direction> {
    if self.has_direction_auto() {
      return Some(self.auto_directionality());
    }

    self.dir()
  }

  /// Blink's `ElementInheritsDirectionality` exception: a tel input without a valid `dir` is
  /// `ltr` rather than its parent's.
  pub(crate) fn keeps_ltr_directionality(&self) -> bool {
    InputType::of(self) == Some(InputType::Tel) && !self.has_valid_dir_attribute()
  }

  /// The element without its children, keeping the direction `dir=auto` reads from them as its
  /// own `dir`.
  pub(crate) fn without_children_keeping_direction(&self) -> Self {
    let mut shell = self.without_children();

    if self.has_direction_auto() {
      shell.metadata.dir = Some(self.auto_directionality());

      if let Some(attributes) = &mut shell.metadata.attributes {
        *attributes = attributes
          .iter()
          .filter(|(name, _)| !name.eq_ignore_ascii_case("dir"))
          .cloned()
          .collect();
      }
    }
    shell
  }

  /// The direction a `dir` of `ltr` or `rtl` gives, from the field or, in any case, the text.
  fn dir(&self) -> Option<Direction> {
    self.metadata.dir.or_else(|| {
      self
        .attribute("dir")
        .filter(|dir| !dir.eq_ignore_ascii_case("auto"))
        .and_then(|dir| Direction::from_css_str(dir).ok())
    })
  }

  /// Blink's `HTMLElement::HasDirectionAuto`: a `dir` of `auto`, or a `<bdi>` without a valid one.
  fn has_direction_auto(&self) -> bool {
    self.has_dir_auto_attribute() || (is_element(self, "bdi") && !self.has_valid_dir_attribute())
  }

  /// Blink's `HTMLElement::IsValidDirAttribute`: `ltr`, `rtl`, or `auto`.
  fn has_valid_dir_attribute(&self) -> bool {
    self.dir().is_some() || self.has_dir_auto_attribute()
  }

  fn has_dir_auto_attribute(&self) -> bool {
    self
      .attribute("dir")
      .is_some_and(|dir| dir.eq_ignore_ascii_case("auto"))
  }

  /// Blink's `Element::ResolveAutoDirectionality`, `ltr` when no strong character decides.
  fn auto_directionality(&self) -> Direction {
    self
      .contained_text_directionality()
      .unwrap_or(Direction::Ltr)
  }

  /// HTML's [contained text auto directionality](https://html.spec.whatwg.org/multipage/dom.html#contained-text-auto-directionality).
  fn contained_text_directionality(&self) -> Option<Direction> {
    match &self.kind {
      NodeKind::Text(text) => BidiParagraph::base_direction_for_string(&text.text, None),
      NodeKind::Image(_) => None,
      NodeKind::Container { children } => children
        .iter()
        .filter(|child| child.affects_contained_text())
        .find_map(Self::contained_text_directionality),
    }
  }

  /// Whether an ancestor's contained text reads this descendant's.
  fn affects_contained_text(&self) -> bool {
    !["bdi", "script", "style", "textarea"]
      .iter()
      .any(|tag| is_element(self, tag))
      && !self.has_valid_dir_attribute()
  }
}

#[cfg(test)]
mod tests {
  use std::collections::BTreeMap;

  use crate::{
    layout::node::Node,
    style::{Direction, UnicodeBidi},
  };

  fn element(tag: &str, attributes: &[(&str, &str)], children: Vec<Node>) -> Node {
    Node::container(children)
      .with_tag_name(tag)
      .with_attributes(
        attributes
          .iter()
          .map(|&(name, value)| (name.into(), value.into()))
          .collect::<BTreeMap<_, _>>(),
      )
  }

  #[test]
  fn dir_auto_on_a_search_input_maps_to_plaintext() {
    let style = |kind: &str| {
      element("input", &[("type", kind), ("dir", "auto")], vec![])
        .direction_style()
        .auto_unicode_bidi
    };

    assert_eq!(style("search"), Some(UnicodeBidi::Plaintext));
    assert_eq!(style("email"), Some(UnicodeBidi::Plaintext));
    assert_eq!(style("text"), Some(UnicodeBidi::Isolate));
  }

  #[test]
  fn a_dir_attribute_gives_direction_in_any_case() {
    assert_eq!(
      element("div", &[("dir", "RTL")], vec![]).own_directionality(),
      Some(Direction::Rtl)
    );

    let auto = element(
      "div",
      &[("dir", "auto")],
      vec![
        element("span", &[("dir", "ltr")], vec![Node::text("abc")]),
        Node::text("مرحبا"),
      ],
    );

    assert_eq!(auto.own_directionality(), Some(Direction::Rtl));
  }

  #[test]
  fn a_tel_input_keeps_ltr_without_its_own_dir() {
    assert!(element("input", &[("type", "tel")], vec![]).keeps_ltr_directionality());
    assert!(
      !element("input", &[("type", "tel"), ("dir", "rtl")], vec![]).keeps_ltr_directionality()
    );
  }

  #[test]
  fn a_shell_keeps_the_direction_its_text_gave_it() {
    let root = element("html", &[("dir", "auto")], vec![Node::text("مرحبا")]);

    assert_eq!(
      root
        .without_children_keeping_direction()
        .own_directionality(),
      Some(Direction::Rtl)
    );
    assert_eq!(
      root.without_children().own_directionality(),
      Some(Direction::Ltr)
    );
  }
}
