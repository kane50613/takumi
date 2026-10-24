//! The SVG element tree, with the style of every element computed, after Blink's
//! `SVGElement` tree and the style resolution that runs over it.
//!
//! A `use` element holds a copy of the element it references, as Blink's `SVGUseElement` holds
//! its shadow tree. A style sheet matches the original element, as Blink does through
//! `SVGElement::CorrespondingElement`.

use std::{collections::HashMap, num::NonZeroU32, sync::Arc};

use roxmltree::{Node as XmlNode, ParsingOptions};
use thiserror::Error;

use super::{
  name::{Attr, Property, Tag},
  scan::parse_number_list,
  style::{StyleContext, SvgStyle},
  style_sheet::StyleSheet,
};
use crate::style::RawDeclaration;

#[cfg(test)]
mod tests;
mod text;

const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
const XLINK_NAMESPACE: &str = "http://www.w3.org/1999/xlink";
pub(super) const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";

const MAX_DEPTH: u32 = 1024;
const MAX_NODES: usize = 1_000_000;

/// Why markup is not an SVG document.
#[derive(Debug, Error)]
pub enum ParseError {
  /// The markup is not well-formed XML.
  #[error("SVG data parsing failed: {0}")]
  Xml(#[from] roxmltree::Error),
  /// The root element is not `svg`.
  #[error("the root element is not <svg>")]
  NotSvg,
  /// The document nests deeper or holds more elements than is allowed.
  #[error("the SVG has too many elements")]
  TooLarge,
}

/// What a document parses with.
#[derive(Debug, Clone)]
pub struct ParseOptions<'a> {
  /// What font sizes and absolute lengths resolve against.
  pub style: StyleContext,
  /// A style sheet that applies before the document's own.
  pub style_sheet: Option<&'a str>,
}

/// A node of a [`Document`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(NonZeroU32);

impl NodeId {
  fn new(index: usize) -> Self {
    Self(NonZeroU32::MIN.saturating_add(index as u32))
  }

  fn index(self) -> usize {
    self.0.get() as usize - 1
  }
}

#[derive(Debug)]
enum NodeKind {
  Root,
  Element {
    tag: Tag,
    attributes: std::ops::Range<usize>,
    style: Arc<SvgStyle>,
  },
  Text(String),
}

#[derive(Debug)]
struct NodeData {
  parent: Option<NodeId>,
  next_sibling: Option<NodeId>,
  children: Option<(NodeId, NodeId)>,
  kind: NodeKind,
}

/// An SVG document: elements, their attributes, and their computed styles.
#[derive(Debug)]
pub struct Document {
  nodes: Vec<NodeData>,
  attributes: Vec<(Attr, Box<str>)>,
  ids: HashMap<Box<str>, NodeId>,
  uses_host_color: bool,
}

impl Document {
  /// Parses SVG markup.
  pub fn parse(markup: &str, options: &ParseOptions) -> Result<Self, ParseError> {
    let xml = roxmltree::Document::parse_with_options(
      markup,
      ParsingOptions {
        allow_dtd: true,
        ..Default::default()
      },
    )?;

    Self::from_xml(&xml, options)
  }

  /// Builds a document from parsed XML.
  pub fn from_xml<'a, 'input: 'a>(
    xml: &'a roxmltree::Document<'input>,
    options: &'a ParseOptions<'a>,
  ) -> Result<Self, ParseError> {
    let mut builder = Builder::new(xml, options);

    builder.document.nodes.push(NodeData {
      parent: None,
      next_sibling: None,
      children: None,
      kind: NodeKind::Root,
    });

    let initial = Arc::new(SvgStyle::initial(&options.style));

    builder.children(xml.root(), xml.root(), NodeId::new(0), &initial, false, 0)?;

    let mut document = builder.document;

    match document.root().first_element_child() {
      Some(element) if element.tag() == Some(Tag::Svg) => {}
      _ => return Err(ParseError::NotSvg),
    }

    for index in 0..document.nodes.len() {
      let id = NodeId::new(index);

      if let Some(value) = document.node(id).attribute(Attr::Id) {
        let value = Box::<str>::from(value);

        document.ids.entry(value).or_insert(id);
      }
    }
    document.uses_host_color = document.descendants().any(|node| {
      node
        .style()
        .is_some_and(|style| style.color.is_none() && style.uses_current_color())
    });
    Ok(document)
  }

  /// The node above every element.
  pub fn root(&self) -> Node<'_> {
    self.node(NodeId::new(0))
  }

  /// The `svg` element at the top.
  pub fn root_element(&self) -> Node<'_> {
    self.root().first_element_child().unwrap_or(self.root())
  }

  /// The first element with this `id`.
  pub fn element_by_id(&self, id: &str) -> Option<Node<'_>> {
    self.ids.get(id).map(|id| self.node(*id))
  }

  /// Every node below the root, in document order.
  pub fn descendants(&self) -> Descendants<'_> {
    self.root().descendants()
  }

  /// Whether any element paints with the color of the host, which `color` leaves unset.
  pub fn uses_host_color(&self) -> bool {
    self.uses_host_color
  }

  fn node(&self, id: NodeId) -> Node<'_> {
    Node { document: self, id }
  }
}

/// A node of a [`Document`] with its tree and style.
#[derive(Debug, Clone, Copy)]
pub struct Node<'a> {
  document: &'a Document,
  id: NodeId,
}

impl PartialEq for Node<'_> {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id && std::ptr::eq(self.document, other.document)
  }
}

impl<'a> Node<'a> {
  fn data(&self) -> &'a NodeData {
    &self.document.nodes[self.id.index()]
  }

  /// The id of this node in its document.
  pub fn id(&self) -> NodeId {
    self.id
  }

  /// The document this node belongs to.
  pub fn document(&self) -> &'a Document {
    self.document
  }

  /// Whether this node is an element.
  pub fn is_element(&self) -> bool {
    matches!(self.data().kind, NodeKind::Element { .. })
  }

  /// Whether this node is character data.
  pub fn is_text(&self) -> bool {
    matches!(self.data().kind, NodeKind::Text(_))
  }

  /// The element name, or `None` for the root and for text.
  pub fn tag(&self) -> Option<Tag> {
    match self.data().kind {
      NodeKind::Element { tag, .. } => Some(tag),
      _ => None,
    }
  }

  /// The computed style, or `None` for the root and for text.
  pub fn style(&self) -> Option<&'a SvgStyle> {
    match &self.data().kind {
      NodeKind::Element { style, .. } => Some(style),
      _ => None,
    }
  }

  /// The attributes that are not style properties.
  pub fn attributes(&self) -> &'a [(Attr, Box<str>)] {
    match &self.data().kind {
      NodeKind::Element { attributes, .. } => &self.document.attributes[attributes.clone()],
      _ => &[],
    }
  }

  /// The value of an attribute.
  pub fn attribute(&self, attr: Attr) -> Option<&'a str> {
    self
      .attributes()
      .iter()
      .find(|(name, _)| *name == attr)
      .map(|(_, value)| &**value)
  }

  /// The `id` attribute, empty when absent.
  pub fn element_id(&self) -> &'a str {
    self.attribute(Attr::Id).unwrap_or("")
  }

  /// A number attribute, or `None` when it is absent or malformed.
  pub fn number(&self, attr: Attr) -> Option<f32> {
    super::scan::parse_number(self.attribute(attr)?)
  }

  /// A list-of-numbers attribute, or `None` when it is absent or malformed.
  pub fn numbers(&self, attr: Attr) -> Option<Vec<f32>> {
    parse_number_list(self.attribute(attr)?)
  }

  /// The element a `href` or other reference attribute names by `#id`.
  pub fn linked(&self, attr: Attr) -> Option<Node<'a>> {
    let id = self.attribute(attr)?.trim().strip_prefix('#')?;

    self.document.element_by_id(id)
  }

  /// The text of a text node, or the first text child of an element.
  pub fn text(&self) -> &'a str {
    match &self.data().kind {
      NodeKind::Text(text) => text,
      NodeKind::Element { .. } => match self.first_child() {
        Some(child) => match &child.data().kind {
          NodeKind::Text(text) => text,
          _ => "",
        },
        None => "",
      },
      NodeKind::Root => "",
    }
  }

  /// The parent node.
  pub fn parent(&self) -> Option<Node<'a>> {
    self.data().parent.map(|id| self.document.node(id))
  }

  /// The parent element.
  pub fn parent_element(&self) -> Option<Node<'a>> {
    self.parent().filter(Node::is_element)
  }

  /// The next sibling.
  pub fn next_sibling(&self) -> Option<Node<'a>> {
    self.data().next_sibling.map(|id| self.document.node(id))
  }

  /// The first child.
  pub fn first_child(&self) -> Option<Node<'a>> {
    self
      .data()
      .children
      .map(|(first, _)| self.document.node(first))
  }

  /// The first child that is an element.
  pub fn first_element_child(&self) -> Option<Node<'a>> {
    self.children().find(Node::is_element)
  }

  /// Whether this node has children.
  pub fn has_children(&self) -> bool {
    self.data().children.is_some()
  }

  /// This node and its ancestors, innermost first.
  pub fn ancestors(&self) -> impl Iterator<Item = Node<'a>> {
    std::iter::successors(Some(*self), Node::parent)
  }

  /// The children in order.
  pub fn children(&self) -> impl Iterator<Item = Node<'a>> {
    std::iter::successors(self.first_child(), Node::next_sibling)
  }

  /// This node and everything below it, in document order.
  pub fn descendants(&self) -> Descendants<'a> {
    Descendants {
      root: *self,
      next: Some(*self),
    }
  }
}

/// The nodes of a subtree in document order.
#[derive(Debug, Clone)]
pub struct Descendants<'a> {
  root: Node<'a>,
  next: Option<Node<'a>>,
}

impl<'a> Iterator for Descendants<'a> {
  type Item = Node<'a>;

  fn next(&mut self) -> Option<Node<'a>> {
    let current = self.next?;

    self.next = current.first_child().or_else(|| {
      let mut node = current;

      loop {
        if node == self.root {
          return None;
        }
        if let Some(sibling) = node.next_sibling() {
          return Some(sibling);
        }
        node = node.parent()?;
      }
    });
    Some(current)
  }
}

struct Builder<'a, 'input> {
  document: Document,
  sheet: StyleSheet<'a>,
  xml_ids: HashMap<&'a str, XmlNode<'a, 'input>>,
  context: StyleContext,
  inherited: Option<(*const SvgStyle, Arc<SvgStyle>)>,
}

impl<'a, 'input: 'a> Builder<'a, 'input> {
  fn new(xml: &'a roxmltree::Document<'input>, options: &'a ParseOptions<'a>) -> Self {
    let mut xml_ids = HashMap::new();

    for node in xml.descendants() {
      if let Some(id) = node.attribute("id") {
        xml_ids.entry(id).or_insert(node);
      }
    }

    let mut sheet = StyleSheet::default();

    // An injected sheet does not override the document's, so it parses first.
    if let Some(injected) = options.style_sheet {
      sheet.parse_more(injected);
    }
    for node in xml.descendants().filter(|node| node.has_tag_name("style")) {
      if node
        .attribute("type")
        .is_some_and(|kind| kind != "text/css")
      {
        continue;
      }
      if let Some(text) = node.text() {
        sheet.parse_more(text);
      }
    }

    Self {
      document: Document {
        nodes: Vec::new(),
        attributes: Vec::new(),
        ids: HashMap::new(),
        uses_host_color: false,
      },
      sheet,
      xml_ids,
      context: options.style,
      inherited: None,
    }
  }

  fn children(
    &mut self,
    parent: XmlNode<'a, 'input>,
    origin: XmlNode<'a, 'input>,
    parent_id: NodeId,
    parent_style: &Arc<SvgStyle>,
    ignore_ids: bool,
    depth: u32,
  ) -> Result<(), ParseError> {
    for node in parent.children() {
      self.node(node, origin, parent_id, parent_style, ignore_ids, depth)?;
    }
    Ok(())
  }

  fn node(
    &mut self,
    node: XmlNode<'a, 'input>,
    origin: XmlNode<'a, 'input>,
    parent_id: NodeId,
    parent_style: &Arc<SvgStyle>,
    ignore_ids: bool,
    depth: u32,
  ) -> Result<(), ParseError> {
    if depth > MAX_DEPTH {
      return Err(ParseError::TooLarge);
    }

    let Some(mut tag) = svg_tag(node) else {
      return Ok(());
    };

    if tag == Tag::Style {
      return Ok(());
    }
    if tag == Tag::A {
      tag = Tag::G;
    }

    let id = self.element(node, parent_id, tag, parent_style, ignore_ids)?;
    let style = self.style_of(id);

    match tag {
      Tag::Text => self.text(node, id, &style)?,
      Tag::Use => self.use_element(node, origin, id, &style, depth + 1)?,
      _ => self.children(node, origin, id, &style, ignore_ids, depth + 1)?,
    }
    Ok(())
  }

  fn style_of(&self, id: NodeId) -> Arc<SvgStyle> {
    match &self.document.nodes[id.index()].kind {
      NodeKind::Element { style, .. } => Arc::clone(style),
      _ => unreachable!("only elements are built here"),
    }
  }

  fn append(&mut self, parent: NodeId, kind: NodeKind) -> Result<NodeId, ParseError> {
    if self.document.nodes.len() > MAX_NODES {
      return Err(ParseError::TooLarge);
    }

    let id = NodeId::new(self.document.nodes.len());

    self.document.nodes.push(NodeData {
      parent: Some(parent),
      next_sibling: None,
      children: None,
      kind,
    });

    let siblings = self.document.nodes[parent.index()].children;

    if let Some((_, last)) = siblings {
      self.document.nodes[last.index()].next_sibling = Some(id);
    }
    self.document.nodes[parent.index()].children =
      Some((siblings.map_or(id, |(first, _)| first), id));
    Ok(id)
  }

  fn element(
    &mut self,
    node: XmlNode<'a, 'input>,
    parent: NodeId,
    tag: Tag,
    parent_style: &Arc<SvgStyle>,
    ignore_ids: bool,
  ) -> Result<NodeId, ParseError> {
    let start = self.document.attributes.len();

    self.collect_attributes(node, tag, ignore_ids);

    let attributes = start..self.document.attributes.len();
    let style = self.compute_style(node, parent_style);

    self.append(
      parent,
      NodeKind::Element {
        tag,
        attributes,
        style,
      },
    )
  }

  /// Copies the attributes that are not style properties, `href` taking precedence over
  /// `xlink:href` whatever their order, as SVG 2 requires.
  fn collect_attributes(&mut self, node: XmlNode<'a, 'input>, tag: Tag, ignore_ids: bool) {
    let mut href: Option<(usize, bool)> = None;

    for attribute in node.attributes() {
      let namespace = attribute.namespace();

      if !matches!(
        namespace,
        None | Some(SVG_NAMESPACE) | Some(XLINK_NAMESPACE) | Some(XML_NAMESPACE)
      ) {
        continue;
      }

      let Some(name) = Attr::from_name(attribute.name()) else {
        continue;
      };

      if (ignore_ids && name == Attr::Id)
        || (name == Attr::Space && namespace != Some(XML_NAMESPACE))
        || (tag == Tag::Tspan && name == Attr::Href)
      {
        continue;
      }
      if name == Attr::Href {
        let unprefixed = namespace.is_none();

        if !unprefixed && namespace != Some(XLINK_NAMESPACE) {
          continue;
        }
        if let Some((index, _)) = href {
          if unprefixed {
            self.document.attributes[index].1 = attribute.value().into();
          }
          continue;
        }
        href = Some((self.document.attributes.len(), unprefixed));
      }
      self
        .document
        .attributes
        .push((name, attribute.value().into()));
    }
  }

  /// Blink's cascade: presentation attributes, then style sheet rules by specificity, then the
  /// `style` attribute, with `!important` declarations after all of them.
  fn compute_style(&mut self, node: XmlNode<'a, 'input>, parent: &Arc<SvgStyle>) -> Arc<SvgStyle> {
    let mut declarations: Vec<(Property, &str, bool)> = Vec::new();

    for attribute in node.attributes() {
      if !matches!(
        attribute.namespace(),
        None | Some(SVG_NAMESPACE) | Some(XML_NAMESPACE)
      ) {
        continue;
      }
      if attribute.namespace() == Some(XML_NAMESPACE) {
        if attribute.name() == "space" {
          let value = if attribute.value() == "preserve" {
            "pre"
          } else {
            "normal"
          };

          declarations.push((Property::WhiteSpace, value, false));
        }
      } else if let Some(property) = Property::from_presentation_attribute(attribute.name()) {
        declarations.push((property, attribute.value(), false));
      }
    }
    self.sheet.cascade(node, |declaration| {
      if let Some(property) = Property::from_name(&declaration.name) {
        declarations.push((property, declaration.value, declaration.important));
      }
    });
    if let Some(inline) = node.attribute("style") {
      for declaration in RawDeclaration::parse_list(inline) {
        if let Some(property) = Property::from_name(&declaration.name) {
          declarations.push((property, declaration.value, declaration.important));
        }
      }
    }

    let base = self.inherited_by_children_of(parent);

    if declarations.is_empty() {
      return base;
    }

    let mut style = (*base).clone();

    for important in [false, true] {
      for (property, value, _) in declarations
        .iter()
        .filter(|(_, _, flag)| *flag == important)
      {
        style.apply(*property, value, parent, &self.context);
      }
    }
    Arc::new(style)
  }

  /// The style of a child of `parent` that declares nothing, shared between siblings.
  fn inherited_by_children_of(&mut self, parent: &Arc<SvgStyle>) -> Arc<SvgStyle> {
    let key = Arc::as_ptr(parent);

    if let Some((cached, style)) = &self.inherited
      && *cached == key
    {
      return Arc::clone(style);
    }

    let style = Arc::new(parent.inherit(&self.context));

    self.inherited = Some((key, Arc::clone(&style)));
    style
  }

  fn use_element(
    &mut self,
    node: XmlNode<'a, 'input>,
    origin: XmlNode<'a, 'input>,
    id: NodeId,
    style: &Arc<SvgStyle>,
    depth: u32,
  ) -> Result<(), ParseError> {
    let Some(link) = self.xml_link(node) else {
      return Ok(());
    };

    if link == node || link == origin || svg_tag(link).is_none() {
      return Ok(());
    }

    let refers_back = link
      .descendants()
      .skip(1)
      .filter(|descendant| descendant.has_tag_name((SVG_NAMESPACE, "use")))
      .any(|descendant| {
        self
          .xml_link(descendant)
          .is_some_and(|target| target == node || target == link)
      });

    if refers_back {
      return Ok(());
    }
    self.node(link, node, id, style, true, depth + 1)
  }

  fn xml_link(&self, node: XmlNode<'a, 'input>) -> Option<XmlNode<'a, 'input>> {
    let value = node
      .attributes()
      .find(|attribute| attribute.name() == "href" && attribute.namespace().is_none())
      .or_else(|| {
        node.attributes().find(|attribute| {
          attribute.name() == "href" && attribute.namespace() == Some(XLINK_NAMESPACE)
        })
      })?
      .value();

    self.xml_ids.get(value.trim().strip_prefix('#')?).copied()
  }
}

fn svg_tag(node: XmlNode) -> Option<Tag> {
  if !node.is_element() || !matches!(node.tag_name().namespace(), None | Some(SVG_NAMESPACE)) {
    return None;
  }
  Tag::from_name(node.tag_name().name())
}
