//! The markup an SVG document is built from: parsed markup, or the element nodes of an inline `svg`.

use super::{XLINK_NAMESPACE, XML_NAMESPACE, XMLNS_NAMESPACE};
use crate::{
  html::dom::{AttributeNamespace, NodeData, NodeRef},
  layout::node::{Node, NodeKind},
};

/// A node of the markup: an element, a text, or a node the document leaves out.
pub(super) trait Markup<'a>: Copy {
  /// The local name of an element, and whether it is in the SVG namespace or none.
  fn element(self) -> Option<(&'a str, bool)>;

  /// The content of a text node.
  fn text(self) -> Option<&'a str>;

  /// Each attribute as its namespace, local name, and value.
  fn attributes(self) -> impl Iterator<Item = (Option<&'a str>, &'a str, &'a str)>;

  fn children(self) -> impl Iterator<Item = Self>;

  /// The value of the attribute `name` in no namespace.
  fn attribute(self, name: &str) -> Option<&'a str> {
    self
      .attributes()
      .find(|&(namespace, local, _)| namespace.is_none() && local == name)
      .map(|(_, _, value)| value)
  }

  /// The text of every `style` element under this node that holds CSS, in document order.
  fn style_sheets(self, sheets: &mut Vec<&'a str>) {
    let mut pending: Vec<Self> = self.children().collect();

    pending.reverse();

    while let Some(node) = pending.pop() {
      if node.element().is_some_and(|(name, _)| name == "style")
        && matches!(node.attribute("type"), None | Some("text/css"))
        && let Some(text) = node.children().find_map(Markup::text)
      {
        sheets.push(text);
      }

      let start = pending.len();

      pending.extend(node.children());
      pending[start..].reverse();
    }
  }
}

/// A node of an SVG document the HTML parser read, which puts `<svg>` and its descendants in the SVG
/// namespace.
impl<'a> Markup<'a> for NodeRef<'a> {
  fn element(self) -> Option<(&'a str, bool)> {
    match self.data() {
      NodeData::Element { name, .. } => Some((&name.local, self.svg_element_name().is_some())),
      _ => None,
    }
  }

  fn text(self) -> Option<&'a str> {
    match self.data() {
      NodeData::Text(text) => Some(text),
      _ => None,
    }
  }

  fn attributes(self) -> impl Iterator<Item = (Option<&'a str>, &'a str, &'a str)> {
    let attributes = match self.data() {
      NodeData::Element { attrs, .. } => attrs.as_slice(),
      _ => &[],
    };

    attributes.iter().map(|attribute| {
      let namespace = match attribute.ns {
        AttributeNamespace::None => None,
        AttributeNamespace::Xlink => Some(XLINK_NAMESPACE),
        AttributeNamespace::Xml => Some(XML_NAMESPACE),
        AttributeNamespace::Xmlns => Some(XMLNS_NAMESPACE),
      };

      (namespace, &*attribute.local, &*attribute.value)
    })
  }

  fn children(self) -> impl Iterator<Item = Self> {
    NodeRef::children(self)
  }
}

/// An inline `svg` element and its subtree, every attribute kept in the attribute map under its
/// qualified name.
impl<'a> Markup<'a> for &'a Node {
  fn element(self) -> Option<(&'a str, bool)> {
    match self.kind {
      NodeKind::Container { .. } => Some((self.tag_name()?, true)),
      _ => None,
    }
  }

  fn text(self) -> Option<&'a str> {
    match &self.kind {
      NodeKind::Text(data) => Some(&data.text),
      _ => None,
    }
  }

  fn attributes(self) -> impl Iterator<Item = (Option<&'a str>, &'a str, &'a str)> {
    self
      .metadata
      .attributes
      .iter()
      .flatten()
      .map(|(name, value)| {
        let (namespace, local) = match name.split_once(':') {
          Some(("xlink", local)) => (Some(XLINK_NAMESPACE), local),
          Some(("xml", local)) => (Some(XML_NAMESPACE), local),
          _ => (None, &**name),
        };

        (namespace, local, &**value)
      })
  }

  fn children(self) -> impl Iterator<Item = Self> {
    Node::children(self).into_iter().flatten()
  }
}
