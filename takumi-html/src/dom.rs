//! The DOM html5ever's tree builder fills: every node in one `Vec`, addressed by index.

use std::{
  borrow::Cow,
  cell::{Ref, RefCell},
  fmt, io, mem,
  sync::LazyLock,
};

use html5ever::{
  Attribute, LocalName, Namespace, QualName,
  interface::{ElemName, ElementFlags, NodeOrText, QuirksMode, TreeSink},
  serialize::{Serialize, Serializer, TraversalScope},
  tendril::StrTendril,
};

/// A node's index in its [`Dom`].
pub(crate) type NodeId = usize;

/// What a node is, and what it holds.
pub(crate) enum NodeData {
  Document,
  Doctype {
    name: StrTendril,
  },
  Text(StrTendril),
  Comment(StrTendril),
  ProcessingInstruction {
    target: StrTendril,
    contents: StrTendril,
  },
  Element {
    name: QualName,
    attrs: Vec<Attribute>,
    /// The document fragment a `<template>` keeps its contents in.
    template_contents: Option<NodeId>,
    mathml_annotation_xml_integration_point: bool,
  },
}

struct Node {
  data: NodeData,
  parent: Option<NodeId>,
  children: Vec<NodeId>,
}

/// A parsed document, its root at [`Dom::DOCUMENT`].
pub(crate) struct Dom {
  nodes: Vec<Node>,
}

impl Dom {
  const DOCUMENT: NodeId = 0;

  fn new() -> Self {
    let mut dom = Self { nodes: Vec::new() };

    dom.push(NodeData::Document);
    dom
  }

  pub(crate) fn document(&self) -> NodeRef<'_> {
    NodeRef {
      dom: self,
      id: Self::DOCUMENT,
    }
  }

  fn push(&mut self, data: NodeData) -> NodeId {
    self.nodes.push(Node {
      data,
      parent: None,
      children: Vec::new(),
    });

    self.nodes.len() - 1
  }

  /// The parent of `id` and the index of `id` among its children.
  fn position(&self, id: NodeId) -> Option<(NodeId, usize)> {
    let parent = self.nodes[id].parent?;
    let index = self.nodes[parent]
      .children
      .iter()
      .position(|&child| child == id)?;

    Some((parent, index))
  }

  fn detach(&mut self, id: NodeId) {
    if let Some((parent, index)) = self.position(id) {
      self.nodes[parent].children.remove(index);
      self.nodes[id].parent = None;
    }
  }

  /// Appends `text` to `id` when it is a text node.
  fn extend_text(&mut self, id: NodeId, text: &str) -> bool {
    match &mut self.nodes[id].data {
      NodeData::Text(contents) => {
        contents.push_slice(text);

        true
      }
      _ => false,
    }
  }

  fn node_for(&mut self, child: NodeOrText<NodeId>) -> NodeId {
    match child {
      NodeOrText::AppendNode(id) => id,
      NodeOrText::AppendText(text) => self.push(NodeData::Text(text)),
    }
  }

  fn insert(&mut self, parent: NodeId, index: usize, child: NodeId) {
    self.detach(child);
    self.nodes[child].parent = Some(parent);
    self.nodes[parent].children.insert(index, child);
  }

  /// Appends `child` to `parent`, merging text into a text node that ends it.
  fn append(&mut self, parent: NodeId, child: NodeOrText<NodeId>) {
    if let NodeOrText::AppendText(text) = &child
      && let Some(&last) = self.nodes[parent].children.last()
      && self.extend_text(last, text)
    {
      return;
    }

    let child = self.node_for(child);
    let index = self.nodes[parent].children.len();

    self.insert(parent, index, child);
  }

  /// Inserts `child` before `sibling`, merging text into a text node just before it.
  fn insert_before(&mut self, sibling: NodeId, child: NodeOrText<NodeId>) {
    let Some((parent, index)) = self.position(sibling) else {
      return;
    };

    if let NodeOrText::AppendText(text) = &child
      && let Some(&previous) = index
        .checked_sub(1)
        .and_then(|previous| self.nodes[parent].children.get(previous))
      && self.extend_text(previous, text)
    {
      return;
    }

    let child = self.node_for(child);

    self.insert(parent, index, child);
  }
}

/// One node of a [`Dom`].
#[derive(Clone, Copy)]
pub(crate) struct NodeRef<'a> {
  dom: &'a Dom,
  id: NodeId,
}

impl<'a> NodeRef<'a> {
  pub(crate) fn data(self) -> &'a NodeData {
    &self.dom.nodes[self.id].data
  }

  pub(crate) fn children(self) -> impl DoubleEndedIterator<Item = NodeRef<'a>> {
    let dom = self.dom;

    dom.nodes[self.id]
      .children
      .iter()
      .map(move |&id| NodeRef { dom, id })
  }

  /// The value of the attribute `name`, when this is an element that has it.
  pub(crate) fn attribute(self, name: &str) -> Option<&'a str> {
    let NodeData::Element { attrs, .. } = self.data() else {
      return None;
    };

    attrs
      .iter()
      .find(|attr| attr.name.local.as_ref() == name)
      .map(|attr| &*attr.value)
  }
}

impl Serialize for NodeRef<'_> {
  fn serialize<S: Serializer>(
    &self,
    serializer: &mut S,
    traversal_scope: TraversalScope,
  ) -> io::Result<()> {
    enum Step<'a> {
      Open(NodeRef<'a>),
      Close(&'a QualName),
    }

    let mut steps = match traversal_scope {
      TraversalScope::IncludeNode => vec![Step::Open(*self)],
      TraversalScope::ChildrenOnly(_) => self.children().rev().map(Step::Open).collect(),
    };

    while let Some(step) = steps.pop() {
      match step {
        Step::Open(node) => match node.data() {
          NodeData::Element { name, attrs, .. } => {
            serializer.start_elem(
              name.clone(),
              attrs.iter().map(|attr| (&attr.name, &*attr.value)),
            )?;
            steps.push(Step::Close(name));
            steps.extend(node.children().rev().map(Step::Open));
          }
          NodeData::Document => steps.extend(node.children().rev().map(Step::Open)),
          NodeData::Doctype { name } => serializer.write_doctype(name)?,
          NodeData::Text(contents) => serializer.write_text(contents)?,
          NodeData::Comment(contents) => serializer.write_comment(contents)?,
          NodeData::ProcessingInstruction { target, contents } => {
            serializer.write_processing_instruction(target, contents)?;
          }
        },
        Step::Close(name) => serializer.end_elem(name.clone())?,
      }
    }

    Ok(())
  }
}

/// The [`TreeSink`] that builds a [`Dom`] while html5ever parses.
pub(crate) struct DomSink {
  dom: RefCell<Dom>,
}

impl Default for DomSink {
  fn default() -> Self {
    Self {
      dom: RefCell::new(Dom::new()),
    }
  }
}

/// An element's name, borrowed from the [`DomSink`] it is in.
pub(crate) struct ElementName<'a>(Ref<'a, QualName>);

impl fmt::Debug for ElementName<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.0.fmt(f)
  }
}

impl ElemName for ElementName<'_> {
  fn ns(&self) -> &Namespace {
    &self.0.ns
  }

  fn local_name(&self) -> &LocalName {
    &self.0.local
  }
}

/// The name `elem_name` gives a node that is not an element, which the tree builder never asks.
static NO_NAME: LazyLock<QualName> =
  LazyLock::new(|| QualName::new(None, Namespace::default(), LocalName::default()));

impl TreeSink for DomSink {
  type Handle = NodeId;
  type Output = Dom;
  type ElemName<'a>
    = ElementName<'a>
  where
    Self: 'a;

  fn finish(self) -> Dom {
    self.dom.into_inner()
  }

  fn parse_error(&self, _msg: Cow<'static, str>) {}

  fn get_document(&self) -> NodeId {
    Dom::DOCUMENT
  }

  fn elem_name<'a>(&'a self, target: &'a NodeId) -> ElementName<'a> {
    ElementName(Ref::map(self.dom.borrow(), |dom| {
      match &dom.nodes[*target].data {
        NodeData::Element { name, .. } => name,
        _ => &NO_NAME,
      }
    }))
  }

  fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> NodeId {
    let mut dom = self.dom.borrow_mut();
    let template_contents = flags.template.then(|| dom.push(NodeData::Document));

    dom.push(NodeData::Element {
      name,
      attrs,
      template_contents,
      mathml_annotation_xml_integration_point: flags.mathml_annotation_xml_integration_point,
    })
  }

  fn create_comment(&self, text: StrTendril) -> NodeId {
    self.dom.borrow_mut().push(NodeData::Comment(text))
  }

  fn create_pi(&self, target: StrTendril, data: StrTendril) -> NodeId {
    self.dom.borrow_mut().push(NodeData::ProcessingInstruction {
      target,
      contents: data,
    })
  }

  fn append(&self, parent: &NodeId, child: NodeOrText<NodeId>) {
    self.dom.borrow_mut().append(*parent, child);
  }

  fn append_before_sibling(&self, sibling: &NodeId, child: NodeOrText<NodeId>) {
    self.dom.borrow_mut().insert_before(*sibling, child);
  }

  fn append_based_on_parent_node(
    &self,
    element: &NodeId,
    prev_element: &NodeId,
    child: NodeOrText<NodeId>,
  ) {
    let mut dom = self.dom.borrow_mut();

    if dom.nodes[*element].parent.is_some() {
      dom.insert_before(*element, child);
    } else {
      dom.append(*prev_element, child);
    }
  }

  fn append_doctype_to_document(
    &self,
    name: StrTendril,
    _public_id: StrTendril,
    _system_id: StrTendril,
  ) {
    let mut dom = self.dom.borrow_mut();
    let doctype = dom.push(NodeData::Doctype { name });

    dom.append(Dom::DOCUMENT, NodeOrText::AppendNode(doctype));
  }

  fn get_template_contents(&self, target: &NodeId) -> NodeId {
    match &self.dom.borrow().nodes[*target].data {
      NodeData::Element {
        template_contents: Some(contents),
        ..
      } => *contents,
      _ => *target,
    }
  }

  fn same_node(&self, x: &NodeId, y: &NodeId) -> bool {
    x == y
  }

  fn set_quirks_mode(&self, _mode: QuirksMode) {}

  fn add_attrs_if_missing(&self, target: &NodeId, attrs: Vec<Attribute>) {
    if let NodeData::Element {
      attrs: existing, ..
    } = &mut self.dom.borrow_mut().nodes[*target].data
    {
      for attr in attrs {
        if !existing.iter().any(|present| present.name == attr.name) {
          existing.push(attr);
        }
      }
    }
  }

  fn remove_from_parent(&self, target: &NodeId) {
    self.dom.borrow_mut().detach(*target);
  }

  fn reparent_children(&self, node: &NodeId, new_parent: &NodeId) {
    let mut dom = self.dom.borrow_mut();
    let children = mem::take(&mut dom.nodes[*node].children);

    for &child in &children {
      dom.nodes[child].parent = Some(*new_parent);
    }

    dom.nodes[*new_parent].children.extend(children);
  }

  fn is_mathml_annotation_xml_integration_point(&self, target: &NodeId) -> bool {
    matches!(
      self.dom.borrow().nodes[*target].data,
      NodeData::Element {
        mathml_annotation_xml_integration_point: true,
        ..
      }
    )
  }
}
