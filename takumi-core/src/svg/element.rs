//! The SVG elements takumi knows, and the CSS declarations their attributes stand for.

use std::{collections::BTreeMap, str::FromStr, sync::Arc};

use crate::{
  path_data::PathData,
  style::{CssInput, PropertyId, Style, StyleDeclaration, SvgPath, Transforms},
};

/// The SVG elements usvg reads. Any other element is dropped with its subtree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SvgElementKind {
  Svg,
  G,
  Defs,
  Symbol,
  Use,
  Switch,
  Rect,
  Circle,
  Ellipse,
  Line,
  Polyline,
  Polygon,
  Path,
  Image,
  Text,
  Tspan,
  TextPath,
  Tref,
  LinearGradient,
  RadialGradient,
  Stop,
  Pattern,
  ClipPath,
  Mask,
  Marker,
  Filter,
  FilterPrimitive,
  Style,
  ForeignObject,
  /// An element takumi does not draw, as Blink's `SVGUnknownElement` and the elements of other
  /// namespaces, which stay in the tree for selectors.
  Unknown,
  /// A text node.
  CharacterData,
}

impl SvgElementKind {
  /// The element a tag names, where `a` is a `g`, as usvg treats links.
  pub(crate) fn from_tag(tag: &str) -> Self {
    match tag {
      "svg" => Self::Svg,
      "g" | "a" => Self::G,
      "defs" => Self::Defs,
      "symbol" => Self::Symbol,
      "use" => Self::Use,
      "switch" => Self::Switch,
      "rect" => Self::Rect,
      "circle" => Self::Circle,
      "ellipse" => Self::Ellipse,
      "line" => Self::Line,
      "polyline" => Self::Polyline,
      "polygon" => Self::Polygon,
      "path" => Self::Path,
      "image" => Self::Image,
      "text" => Self::Text,
      "tspan" => Self::Tspan,
      "textPath" => Self::TextPath,
      "tref" => Self::Tref,
      "linearGradient" => Self::LinearGradient,
      "radialGradient" => Self::RadialGradient,
      "stop" => Self::Stop,
      "pattern" => Self::Pattern,
      "clipPath" => Self::ClipPath,
      "mask" => Self::Mask,
      "marker" => Self::Marker,
      "filter" => Self::Filter,
      "style" => Self::Style,
      "foreignObject" => Self::ForeignObject,
      "feBlend"
      | "feColorMatrix"
      | "feComponentTransfer"
      | "feComposite"
      | "feConvolveMatrix"
      | "feDiffuseLighting"
      | "feDisplacementMap"
      | "feDistantLight"
      | "feDropShadow"
      | "feFlood"
      | "feFuncA"
      | "feFuncB"
      | "feFuncG"
      | "feFuncR"
      | "feGaussianBlur"
      | "feImage"
      | "feMerge"
      | "feMergeNode"
      | "feMorphology"
      | "feOffset"
      | "fePointLight"
      | "feSpecularLighting"
      | "feSpotLight"
      | "feTile"
      | "feTurbulence" => Self::FilterPrimitive,
      _ => Self::Unknown,
    }
  }

  /// Blink's graphics elements, which paint.
  pub(crate) fn is_graphic(self) -> bool {
    matches!(
      self,
      Self::Circle
        | Self::Ellipse
        | Self::Image
        | Self::Line
        | Self::Path
        | Self::Polygon
        | Self::Polyline
        | Self::Rect
        | Self::Text
        | Self::Use
    )
  }

  pub(crate) fn is_shape(self) -> bool {
    matches!(
      self,
      Self::Circle
        | Self::Ellipse
        | Self::Line
        | Self::Path
        | Self::Polygon
        | Self::Polyline
        | Self::Rect
    )
  }

  pub(crate) fn is_gradient(self) -> bool {
    matches!(self, Self::LinearGradient | Self::RadialGradient)
  }

  pub(crate) fn is_paint_server(self) -> bool {
    self.is_gradient() || self == Self::Pattern
  }

  /// Whether `svg.css` clips the element's overflow.
  fn clips_overflow(self, outermost: bool) -> bool {
    match self {
      Self::Svg => !outermost,
      Self::Symbol | Self::Image | Self::Marker | Self::Pattern => true,
      _ => false,
    }
  }

  /// Blink's `svg.css` for the element: clipped overflow, and the transform origin that puts
  /// percentages and offsets at the viewport corner.
  pub(crate) fn preset(self, outermost: bool) -> Option<Arc<Style>> {
    let css = match (self.clips_overflow(outermost), outermost) {
      (true, _) => "overflow: hidden; transform-origin: 0 0",
      (false, false) => "transform-origin: 0 0",
      (false, true) => return None,
    };

    Style::from_str(css).ok().map(Arc::new)
  }

  /// The declarations the element's presentation attributes stand for, as Blink's
  /// `SVGElement::CollectStyleForPresentationAttribute`.
  pub(crate) fn presentation(
    self,
    attributes: &BTreeMap<Box<str>, Box<str>>,
  ) -> Option<Box<Style>> {
    let mut style = Style::default();

    for (name, value) in attributes {
      let name: &str = name;
      let value: &str = value;

      match name {
        "transform" if self.is_transformable() => Self::push_transform(&mut style, value),
        "gradientTransform" if self.is_gradient() => Self::push_transform(&mut style, value),
        "patternTransform" if self == Self::Pattern => Self::push_transform(&mut style, value),
        "d" if self == Self::Path => {
          let (commands, _) = PathData::parse(value);

          if !commands.is_empty() {
            style.push(StyleDeclaration::d(SvgPath::new(commands)), false);
          }
        }
        _ if PRESENTATION_ATTRIBUTES.contains(&name) || self.has_geometry_attribute(name) => {
          // Blink lays out `auto` as visible and `scroll` as hidden for SVG.
          let value = match (name, value) {
            ("overflow", "auto") => "visible",
            ("overflow", "scroll") => "hidden",
            _ => value,
          };

          if let Ok(declarations) = PropertyId::from_kebab_case(name)
            .parse_css_input_declarations(CssInput::Str(value.into()))
          {
            for declaration in declarations {
              style.push(declaration, false);
            }
          }
        }
        _ => {}
      }
    }

    (!style.declarations.is_empty()).then(|| Box::new(style))
  }

  fn push_transform(style: &mut Style, value: &str) {
    if let Some(transforms) = Transforms::from_svg_attribute(value) {
      style.push(StyleDeclaration::transform(Some(transforms)), false);
    }
  }

  /// Blink's transformable elements, and the clip path, which transforms its children.
  fn is_transformable(self) -> bool {
    self.is_graphic() || matches!(self, Self::G | Self::Switch | Self::ClipPath | Self::Svg)
  }

  /// Whether the attribute is one of the element's lengths that Blink maps to a CSS property.
  fn has_geometry_attribute(self, name: &str) -> bool {
    match self {
      Self::Rect => matches!(name, "x" | "y" | "width" | "height" | "rx" | "ry"),
      Self::Circle => matches!(name, "cx" | "cy" | "r"),
      Self::Ellipse => matches!(name, "cx" | "cy" | "rx" | "ry"),
      Self::Image | Self::Svg => matches!(name, "x" | "y" | "width" | "height"),
      Self::Use => matches!(name, "x" | "y"),
      _ => false,
    }
  }
}

/// The attributes that name a CSS property of the same name on any SVG element, from Blink's
/// `SVGElement::CssPropertyIdForSVGAttributeName`. `mix-blend-mode` and `isolation` are CSS-only.
const PRESENTATION_ATTRIBUTES: [&str; 59] = [
  "alignment-baseline",
  "baseline-shift",
  "clip-path",
  "clip-rule",
  "color",
  "color-interpolation",
  "color-interpolation-filters",
  "direction",
  "display",
  "dominant-baseline",
  "fill",
  "fill-opacity",
  "fill-rule",
  "filter",
  "flood-color",
  "flood-opacity",
  "font-family",
  "font-size",
  "font-stretch",
  "font-style",
  "font-variant",
  "font-weight",
  "image-rendering",
  "letter-spacing",
  "lighting-color",
  "marker-end",
  "marker-mid",
  "marker-start",
  "mask",
  "mask-type",
  "opacity",
  "overflow",
  "paint-order",
  "shape-rendering",
  "stop-color",
  "stop-opacity",
  "stroke",
  "stroke-dasharray",
  "stroke-dashoffset",
  "stroke-linecap",
  "stroke-linejoin",
  "stroke-miterlimit",
  "stroke-opacity",
  "stroke-width",
  "text-anchor",
  "text-decoration",
  "text-overflow",
  "transform-origin",
  "unicode-bidi",
  "vector-effect",
  "visibility",
  "word-spacing",
  "writing-mode",
  "font-kerning",
  "font-optical-sizing",
  "font-variation-settings",
  "text-rendering",
  "white-space",
  "background-color",
];
