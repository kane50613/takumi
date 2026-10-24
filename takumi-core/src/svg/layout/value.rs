//! Typed reads of the attributes that are not style properties.

use tiny_skia::Transform;

use crate::{
  resvg::usvg::{BlendMode, SpreadMethod, Units},
  svg::{
    Attr, Length, Node, PreserveAspectRatio, ViewBox, parse_angle, parse_length, parse_length_list,
    parse_number, parse_number_list, parse_transform_list,
  },
};

/// An angle attribute, in degrees.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Angle(pub f32);

impl FromValue<'_> for Angle {
  fn parse(_: Node, _: Attr, value: &str) -> Option<Self> {
    parse_angle(value).map(Self)
  }
}

/// A type an attribute value parses into.
pub(crate) trait FromValue<'a>: Sized {
  fn parse(node: Node<'a>, attr: Attr, value: &'a str) -> Option<Self>;
}

impl<'a> FromValue<'a> for &'a str {
  fn parse(_: Node<'a>, _: Attr, value: &'a str) -> Option<Self> {
    Some(value)
  }
}

impl FromValue<'_> for f32 {
  fn parse(_: Node, _: Attr, value: &str) -> Option<Self> {
    parse_number(value)
  }
}

impl FromValue<'_> for Length {
  fn parse(_: Node, _: Attr, value: &str) -> Option<Self> {
    parse_length(value)
  }
}

impl FromValue<'_> for Vec<f32> {
  fn parse(_: Node, _: Attr, value: &str) -> Option<Self> {
    parse_number_list(value)
  }
}

impl FromValue<'_> for Vec<Length> {
  fn parse(_: Node, _: Attr, value: &str) -> Option<Self> {
    parse_length_list(value)
  }
}

impl FromValue<'_> for Transform {
  fn parse(_: Node, _: Attr, value: &str) -> Option<Self> {
    let transform = parse_transform_list(value)?;

    Some(if transform.is_valid() {
      transform
    } else {
      Transform::default()
    })
  }
}

impl FromValue<'_> for ViewBox {
  fn parse(_: Node, _: Attr, value: &str) -> Option<Self> {
    ViewBox::parse(value)
  }
}

impl FromValue<'_> for PreserveAspectRatio {
  fn parse(_: Node, _: Attr, value: &str) -> Option<Self> {
    PreserveAspectRatio::parse(value)
  }
}

impl FromValue<'_> for Units {
  fn parse(_: Node, _: Attr, value: &str) -> Option<Self> {
    match value {
      "userSpaceOnUse" => Some(Units::UserSpaceOnUse),
      "objectBoundingBox" => Some(Units::ObjectBoundingBox),
      _ => None,
    }
  }
}

impl FromValue<'_> for SpreadMethod {
  fn parse(_: Node, _: Attr, value: &str) -> Option<Self> {
    match value {
      "pad" => Some(SpreadMethod::Pad),
      "reflect" => Some(SpreadMethod::Reflect),
      "repeat" => Some(SpreadMethod::Repeat),
      _ => None,
    }
  }
}

impl FromValue<'_> for BlendMode {
  fn parse(_: Node, _: Attr, value: &str) -> Option<Self> {
    match value {
      "normal" => Some(BlendMode::Normal),
      "multiply" => Some(BlendMode::Multiply),
      "screen" => Some(BlendMode::Screen),
      "overlay" => Some(BlendMode::Overlay),
      "darken" => Some(BlendMode::Darken),
      "lighten" => Some(BlendMode::Lighten),
      "color-dodge" => Some(BlendMode::ColorDodge),
      "color-burn" => Some(BlendMode::ColorBurn),
      "hard-light" => Some(BlendMode::HardLight),
      "soft-light" => Some(BlendMode::SoftLight),
      "difference" => Some(BlendMode::Difference),
      "exclusion" => Some(BlendMode::Exclusion),
      "hue" => Some(BlendMode::Hue),
      "saturation" => Some(BlendMode::Saturation),
      "color" => Some(BlendMode::Color),
      "luminosity" => Some(BlendMode::Luminosity),
      _ => None,
    }
  }
}

impl<'a> FromValue<'a> for Node<'a> {
  fn parse(node: Node<'a>, _: Attr, value: &'a str) -> Option<Self> {
    node
      .document()
      .element_by_id(value.trim().strip_prefix('#')?)
  }
}

impl<'a> Node<'a> {
  /// An attribute parsed as `T`, or `None` when it is absent or malformed.
  pub(crate) fn parse<T: FromValue<'a>>(&self, attr: Attr) -> Option<T> {
    let value = self.attribute(attr)?;

    match T::parse(*self, attr, value) {
      Some(parsed) => Some(parsed),
      None => {
        log::warn!("Failed to parse {} value: '{}'.", attr.name(), value);
        None
      }
    }
  }

  /// An attribute parsed as `T`, without a warning when it is malformed.
  pub(crate) fn try_parse<T: FromValue<'a>>(&self, attr: Attr) -> Option<T> {
    T::parse(*self, attr, self.attribute(attr)?)
  }

  /// Whether the attribute is present.
  pub(crate) fn has_attribute(&self, attr: Attr) -> bool {
    self.attribute(attr).is_some()
  }

  /// The element a `url(#id)` style reference names.
  pub(crate) fn reference(&self, id: &Option<std::sync::Arc<str>>) -> Option<Node<'a>> {
    self.document().element_by_id(id.as_deref()?)
  }
}
