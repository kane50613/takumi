//! SVG documents resolved through takumi's CSS cascade and laid out to drawing ops, after Blink's
//! `SVGImage`. An element's style comes from its `<style>` rules, its `style` attribute, and its
//! presentation attributes, and its layout from the shapes, containers, and resources it names.

mod approx_eq;
mod element;
#[cfg(feature = "svg-raster")]
pub(crate) mod filter;
mod layout;
mod length;
mod style;
#[cfg(test)]
mod tests;

use std::{
  collections::{BTreeMap, HashMap},
  sync::Arc,
};

use roxmltree::{Node as XmlNode, NodeType};

use self::element::SvgElementKind;
#[cfg(feature = "svg-raster")]
use self::filter::SvgFilter;
use crate::{
  geometry::Size,
  layout::node::{MAXIMUM_DOM_TREE_DEPTH, Node, NodeKind},
  resources::{
    font::FontsSnapshot,
    image::{ImageError, SvgOp},
    svg_size::{SvgSize, parse_svg_document},
  },
  style::{Color, Lang, Style, StyleDeclarationBlock, StyleSheet},
};

const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
const XLINK_NAMESPACE: &str = "http://www.w3.org/1999/xlink";
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";

/// Where an element sits in the document, in the preorder the selector matcher numbers nodes by.
#[derive(Debug)]
pub(super) struct ElementEntry {
  pub(super) kind: SvgElementKind,
  pub(super) parent: Option<usize>,
  pub(super) children: Vec<usize>,
}

/// An SVG document parsed into takumi nodes, ready to resolve styles and lay out.
#[derive(Debug)]
pub(crate) struct SvgDocument {
  pub(super) root: Node,
  pub(super) elements: Vec<ElementEntry>,
  pub(super) stylesheet: Arc<StyleSheet>,
  /// The element an `id` names for `use`, which takes the first of several.
  first_ids: HashMap<Box<str>, usize>,
  /// The element an `id` names for every other reference, which takes the last.
  pub(super) last_ids: HashMap<Box<str>, usize>,
  pub(super) size: SvgSize,
  /// The size of the canvas, which the extent of the content gives when the root sets none.
  canvas: Size<f32>,
  mentions_current_color: bool,
  has_color: bool,
}

impl SvgDocument {
  /// Parses SVG markup. The root must be an `svg` element in the SVG namespace or in none.
  pub(crate) fn parse(markup: &str) -> Result<Self, ImageError> {
    let xml = parse_svg_document(markup).map_err(ImageError::svg_parse)?;
    let root = xml.root_element();

    if !is_svg_element(root) || root.tag_name().name() != "svg" {
      return Err(ImageError::svg_parse(roxmltree::Error::NoRootNode));
    }

    let size = SvgSize::from_root(root).map_err(ImageError::svg_parse)?;
    let sheets: Vec<&str> = xml
      .descendants()
      .filter(|node| node.is_element() && node.tag_name().name() == "style")
      .filter(|node| matches!(node.attribute("type"), None | Some("text/css")))
      .filter_map(|node| node.text())
      .collect();
    let mut builder = TreeBuilder::default();
    let root_node = builder.element(root, None, 0);

    let mut document = Self {
      stylesheet: Arc::new(StyleSheet::parse_list_loosy(&sheets)),
      mentions_current_color: builder.mentions_current_color
        || sheets
          .iter()
          .any(|sheet| contains_ignore_case(sheet, "currentcolor")),
      has_color: root.attribute("color").is_some(),
      root: root_node,
      elements: builder.elements,
      first_ids: builder.first_ids,
      last_ids: builder.last_ids,
      canvas: Size::new(size.width, size.height),
      size,
    };

    document.canvas = layout::SvgLayout::new(&document, Color([0, 0, 0, 255]), None).canvas_size();

    Ok(document)
  }

  /// The size of the canvas in pixels.
  pub(crate) fn canvas_size(&self) -> (f32, f32) {
    (self.canvas.width, self.canvas.height)
  }

  /// Whether the document holds a `text` element, which draws with the registered fonts.
  #[cfg(feature = "svg-raster")]
  pub(crate) fn has_text(&self) -> bool {
    self
      .elements
      .iter()
      .any(|element| element.kind == SvgElementKind::Text)
  }

  /// Every node in the preorder the selector matcher numbers them by.
  fn nodes(&self) -> Vec<&Node> {
    let mut nodes = Vec::with_capacity(self.elements.len());
    let mut stack = vec![&self.root];

    while let Some(node) = stack.pop() {
      nodes.push(node);

      if let Some(children) = node.children() {
        stack.extend(children.iter().rev());
      }
    }

    nodes
  }

  /// The children of the node at `index` that are elements, text left out.
  fn element_children(&self, index: usize) -> Vec<usize> {
    self.elements[index]
      .children
      .iter()
      .copied()
      .filter(|child| self.elements[*child].kind != SvgElementKind::CharacterData)
      .collect()
  }

  /// Whether the rendering depends on the host `color`: the markup names `currentColor` and the
  /// root sets no `color` of its own.
  pub(crate) fn uses_current_color(&self) -> bool {
    self.mentions_current_color && !self.has_color
  }

  /// How many nodes the document retains, for cache weighing.
  pub(crate) fn node_count(&self) -> usize {
    self.elements.len()
  }

  /// The filter the element `id` names, laid out for content of `width` by `height`.
  #[cfg(feature = "svg-raster")]
  pub(crate) fn filter(&self, id: &str, width: f32, height: f32) -> Option<Arc<SvgFilter>> {
    layout::SvgLayout::new(self, Color([0, 0, 0, 255]), None).filter_by_id(id, width, height)
  }

  /// The drawing ops of the document in canvas coordinates, with `current_color` the `color` the
  /// root inherits from its host.
  pub(crate) fn ops(
    &self,
    current_color: Color,
    fonts: Option<&FontsSnapshot>,
  ) -> Option<Vec<SvgOp>> {
    layout::SvgLayout::new(self, current_color, fonts).layout()
  }
}

fn contains_ignore_case(text: &str, needle: &str) -> bool {
  text
    .as_bytes()
    .windows(needle.len())
    .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn is_svg_element(node: XmlNode) -> bool {
  node.is_element() && matches!(node.tag_name().namespace(), None | Some(SVG_NAMESPACE))
}

/// Builds the node tree and the element table together, numbering each node as the matcher will.
#[derive(Default)]
struct TreeBuilder {
  elements: Vec<ElementEntry>,
  first_ids: HashMap<Box<str>, usize>,
  last_ids: HashMap<Box<str>, usize>,
  mentions_current_color: bool,
}

impl TreeBuilder {
  /// The node for `xml` and its subtree, none for a comment or an instruction, or a subtree that
  /// nests too deep.
  fn node(&mut self, xml: XmlNode, parent: Option<usize>, depth: usize) -> Option<Node> {
    if depth >= MAXIMUM_DOM_TREE_DEPTH {
      return None;
    }

    match xml.node_type() {
      NodeType::Element => Some(self.element(xml, parent, depth)),
      NodeType::Text => {
        let text = xml.text()?;

        self.push_entry(SvgElementKind::CharacterData, parent);
        Some(Node::text(text))
      }
      _ => None,
    }
  }

  fn push_entry(&mut self, kind: SvgElementKind, parent: Option<usize>) -> usize {
    let index = self.elements.len();

    self.elements.push(ElementEntry {
      kind,
      parent,
      children: Vec::new(),
    });

    if let Some(parent) = parent {
      self.elements[parent].children.push(index);
    }

    index
  }

  fn element(&mut self, xml: XmlNode, parent: Option<usize>, depth: usize) -> Node {
    let kind = if is_svg_element(xml) {
      SvgElementKind::from_tag(xml.tag_name().name())
    } else {
      SvgElementKind::Unknown
    };
    let index = self.push_entry(kind, parent);
    let mut node = Node::container([]).with_tag_name(xml.tag_name().name());
    let mut attributes = BTreeMap::new();
    let mut href = None;

    for attribute in xml.attributes() {
      let value = attribute.value();

      match (attribute.namespace(), attribute.name()) {
        (None | Some(SVG_NAMESPACE), "class") => {
          node = node.with_class_name(value);
        }
        (None | Some(SVG_NAMESPACE), "id") => {
          node = node.with_id(value);
          self.first_ids.entry(value.into()).or_insert(index);
          self.last_ids.insert(value.into(), index);
        }
        (None | Some(SVG_NAMESPACE), "style") => {
          self.mentions_current_color |= contains_ignore_case(value, "currentcolor");
          node = node.with_style(Style::from(StyleDeclarationBlock::parse_loosy(value)));
        }
        (Some(XLINK_NAMESPACE), "href") => {
          href.get_or_insert(value);
        }
        (None | Some(SVG_NAMESPACE), "href") => {
          href = Some(value);
        }
        (Some(XML_NAMESPACE), "space") => {
          attributes.insert("xml:space".into(), value.into());
        }
        (Some(XML_NAMESPACE), "lang") | (None, "lang") => {
          if let Ok(lang) = Lang::parse(value) {
            node = node.with_lang(lang);
          }
        }
        (None | Some(SVG_NAMESPACE), name) => {
          self.mentions_current_color |= contains_ignore_case(value, "currentcolor");
          attributes.insert(name.into(), value.into());
        }
        _ => {}
      }
    }

    if let Some(href) = href {
      attributes.insert("href".into(), href.into());
    }

    if let Some(preset) = kind.preset(parent.is_none()) {
      node = node.with_preset(preset);
    }

    node.metadata.presentation = kind.presentation(&attributes);

    if !attributes.is_empty() {
      node = node.with_attributes(attributes);
    }

    let children = xml
      .children()
      .filter_map(|child| self.node(child, Some(index), depth + 1))
      .collect();

    node.kind = NodeKind::Container { children };
    node
  }
}
