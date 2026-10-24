//! The markup an SVG document is built from: parsed XML, or the element nodes of an inline `svg`.

use roxmltree::{Node as XmlNode, NodeType};

use super::{SVG_NAMESPACE, XLINK_NAMESPACE, XML_NAMESPACE};
use crate::layout::node::{Node, NodeKind};

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
    for child in self.children() {
      if child.element().is_some_and(|(name, _)| name == "style")
        && matches!(child.attribute("type"), None | Some("text/css"))
        && let Some(text) = child.children().find_map(Markup::text)
      {
        sheets.push(text);
      }

      child.style_sheets(sheets);
    }
  }
}

impl<'a, 'input> Markup<'a> for XmlNode<'a, 'input> {
  fn element(self) -> Option<(&'a str, bool)> {
    if !self.is_element() {
      return None;
    }

    let name = self.tag_name();

    Some((
      name.name(),
      matches!(name.namespace(), None | Some(SVG_NAMESPACE)),
    ))
  }

  fn text(self) -> Option<&'a str> {
    match self.node_type() {
      NodeType::Text => XmlNode::text(&self),
      _ => None,
    }
  }

  fn attributes(self) -> impl Iterator<Item = (Option<&'a str>, &'a str, &'a str)> {
    XmlNode::attributes(&self)
      .map(|attribute| (attribute.namespace(), attribute.name(), attribute.value()))
  }

  fn children(self) -> impl Iterator<Item = Self> {
    XmlNode::children(&self)
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
