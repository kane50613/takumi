//! Conditional processing, after Blink's `SVGSwitchElement` and `SVGTests::IsValid`.

use super::{
  Scope, Styled, SvgLayout,
  object::{Container, LayoutObject},
};
use crate::svg::element::SvgElementKind;

/// The SVG 1.1 feature strings `requiredFeatures` may name.
/// https://www.w3.org/TR/SVG11/feature.html
const FEATURES: [&str; 25] = [
  "http://www.w3.org/TR/SVG11/feature#SVGDOM-static",
  "http://www.w3.org/TR/SVG11/feature#SVG-static",
  "http://www.w3.org/TR/SVG11/feature#CoreAttribute",
  "http://www.w3.org/TR/SVG11/feature#Structure",
  "http://www.w3.org/TR/SVG11/feature#BasicStructure",
  "http://www.w3.org/TR/SVG11/feature#ContainerAttribute",
  "http://www.w3.org/TR/SVG11/feature#ConditionalProcessing",
  "http://www.w3.org/TR/SVG11/feature#Image",
  "http://www.w3.org/TR/SVG11/feature#Style",
  "http://www.w3.org/TR/SVG11/feature#Shape",
  "http://www.w3.org/TR/SVG11/feature#Text",
  "http://www.w3.org/TR/SVG11/feature#BasicText",
  "http://www.w3.org/TR/SVG11/feature#PaintAttribute",
  "http://www.w3.org/TR/SVG11/feature#BasicPaintAttribute",
  "http://www.w3.org/TR/SVG11/feature#OpacityAttribute",
  "http://www.w3.org/TR/SVG11/feature#GraphicsAttribute",
  "http://www.w3.org/TR/SVG11/feature#BasicGraphicsAttribute",
  "http://www.w3.org/TR/SVG11/feature#Marker",
  "http://www.w3.org/TR/SVG11/feature#Gradient",
  "http://www.w3.org/TR/SVG11/feature#Pattern",
  "http://www.w3.org/TR/SVG11/feature#Clip",
  "http://www.w3.org/TR/SVG11/feature#BasicClip",
  "http://www.w3.org/TR/SVG11/feature#Mask",
  "http://www.w3.org/TR/SVG11/feature#Filter",
  "http://www.w3.org/TR/SVG11/feature#BasicFilter",
];

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

  /// Whether the element's `requiredExtensions`, `requiredFeatures`, and `systemLanguage` let it
  /// render.
  pub(super) fn passes_conditions(&self, index: usize) -> bool {
    if self.has_attribute(index, "requiredExtensions") {
      return false;
    }

    // Every named feature must be one the renderer supports.
    if let Some(features) = self.attribute(index, "requiredFeatures")
      && !features
        .split_ascii_whitespace()
        .all(|feature| FEATURES.contains(&feature))
    {
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
