//! Conditional processing, after Blink's `SVGSwitchElement` and `SVGTests::IsValid`.

use super::{
  Scope, Styled, SvgLayout,
  object::{Container, LayoutObject},
};
use crate::svg::element::SvgElementKind;

/// The languages of the user, which `systemLanguage` is matched against.
const LANGUAGES: [&str; 1] = ["en"];

impl SvgLayout<'_> {
  /// Lays out the first child of the `switch` whose conditions pass.
  pub(super) fn convert_switch(&mut self, styled: &Styled, scope: &Scope, parent: &mut Container) {
    let Some(child) = self
      .document
      .element_children(styled.index)
      .into_iter()
      .find(|child| {
        !matches!(
          self.document.elements[*child].kind,
          SvgElementKind::Unknown | SvgElementKind::Style | SvgElementKind::ForeignObject
        ) && self.passes_conditions(*child)
      })
    else {
      return;
    };
    let group = self.convert_group(styled, scope, false, parent, |layout, group| {
      layout.convert_element(child, &styled.context, scope, group);
    });

    if let Some(group) = group {
      parent
        .children
        .push(LayoutObject::Container(Box::new(group)));
    }
  }

  /// Whether the element's `requiredExtensions` and `systemLanguage` let it render, as Blink's
  /// `SVGTests::IsValid` reads them. Chrome dropped `requiredFeatures`.
  pub(super) fn passes_conditions(&self, index: usize) -> bool {
    if self.has_attribute(index, "requiredExtensions") {
      return false;
    }

    let Some(languages) = self.attribute(index, "systemLanguage") else {
      return true;
    };

    // A language matches when a user language equals it or is a prefix of it before a hyphen.
    languages.split(',').map(str::trim).any(|language| {
      LANGUAGES.contains(&language)
        || language
          .split_once('-')
          .is_some_and(|(prefix, _)| LANGUAGES.contains(&prefix))
    })
  }
}
