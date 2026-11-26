//! HTML's [directionality](https://html.spec.whatwg.org/multipage/dom.html#the-directionality) of
//! an element and the style its `dir` attribute maps to, after Blink's `HTMLElement`.

use super::{Node, NodeKind};
use crate::{form_control::is_element, style::Direction, text::bidi_paragraph::BidiParagraph};

/// The declarations an element's `dir` attribute and tag give the cascade.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct DirectionStyle {
  /// html.css's `[dir=ltr i], [dir=rtl i], [dir=auto i] { unicode-bidi: isolate }`.
  pub(crate) isolate: bool,
  /// html.css's `textarea[dir=auto i], pre[dir=auto i] { unicode-bidi: plaintext }`.
  pub(crate) plaintext: bool,
  /// The `unicode-bidi: isolate` presentational hint `dir=auto` maps to elsewhere.
  pub(crate) auto_isolate: bool,
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

    DirectionStyle {
      isolate: self.has_valid_dir_attribute(),
      plaintext: auto && is_any(&["textarea", "pre"]),
      auto_isolate: auto && !is_any(&["bdo", "textarea", "pre"]),
      direction,
    }
  }

  /// The element's directionality, `None` when it takes its parent's. Approximate: an `<input>`
  /// reads its text as any element does, not its value.
  pub(crate) fn own_directionality(&self) -> Option<Direction> {
    if self.has_direction_auto() {
      return Some(self.auto_directionality());
    }

    self.metadata.dir
  }

  /// Blink's `HTMLElement::HasDirectionAuto`: a `dir` of `auto`, or a `<bdi>` without a valid one.
  fn has_direction_auto(&self) -> bool {
    self.has_dir_auto_attribute() || (is_element(self, "bdi") && !self.has_valid_dir_attribute())
  }

  /// Blink's `HTMLElement::IsValidDirAttribute`: `ltr`, `rtl`, or `auto`.
  fn has_valid_dir_attribute(&self) -> bool {
    self.metadata.dir.is_some() || self.has_dir_auto_attribute()
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
