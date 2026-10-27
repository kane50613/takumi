//! The DOM a parser fills: every node in one `Vec`, addressed by index.

/// A node's index in its [`Dom`].
pub(crate) type NodeId = usize;

/// The namespace of an element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Namespace {
  Html,
  Svg,
  MathMl,
}

/// An element's namespace and local name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ElementName {
  pub(crate) ns: Namespace,
  pub(crate) local: Box<str>,
}

/// The namespace an attribute name carries, which serialization writes as its prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AttributeNamespace {
  None,
  Xlink,
  Xml,
  Xmlns,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Attribute {
  pub(crate) ns: AttributeNamespace,
  pub(crate) local: Box<str>,
  pub(crate) value: Box<str>,
}

/// What a node is, and what it holds.
pub(crate) enum NodeData {
  Document,
  Doctype {
    name: Box<str>,
  },
  Text(String),
  Comment(Box<str>),
  Element {
    name: ElementName,
    attrs: Vec<Attribute>,
  },
}

struct Node {
  data: NodeData,
  children: Vec<NodeId>,
}

/// A parsed document, its root at [`Dom::DOCUMENT`].
pub(crate) struct Dom {
  nodes: Vec<Node>,
}

impl Default for Dom {
  fn default() -> Self {
    let mut dom = Self { nodes: Vec::new() };

    dom.push(NodeData::Document);
    dom
  }
}

impl Dom {
  pub(crate) const DOCUMENT: NodeId = 0;

  pub(crate) fn document(&self) -> NodeRef<'_> {
    NodeRef {
      dom: self,
      id: Self::DOCUMENT,
    }
  }

  pub(crate) fn data(&self, id: NodeId) -> &NodeData {
    &self.nodes[id].data
  }

  /// The name of `id` when it is an element.
  pub(crate) fn element_name(&self, id: NodeId) -> Option<&ElementName> {
    match self.data(id) {
      NodeData::Element { name, .. } => Some(name),
      _ => None,
    }
  }

  pub(crate) fn data_mut(&mut self, id: NodeId) -> &mut NodeData {
    &mut self.nodes[id].data
  }

  pub(crate) fn push(&mut self, data: NodeData) -> NodeId {
    self.nodes.push(Node {
      data,
      children: Vec::new(),
    });

    self.nodes.len() - 1
  }

  pub(crate) fn append_node(&mut self, parent: NodeId, child: NodeId) {
    self.nodes[parent].children.push(child);
  }

  /// Appends `text` to `parent`, merging it into a text node that ends it.
  pub(crate) fn append_text(&mut self, parent: NodeId, text: &str) {
    if let Some(&last) = self.nodes[parent].children.last()
      && let NodeData::Text(contents) = &mut self.nodes[last].data
    {
      contents.push_str(text);
      return;
    }

    let child = self.push(NodeData::Text(text.into()));

    self.append_node(parent, child);
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
    self.dom.data(self.id)
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
      .find(|attr| &*attr.local == name)
      .map(|attr| &*attr.value)
  }

  /// This node and its subtree as HTML, escaped as the fragment serialization algorithm escapes them.
  /// <https://html.spec.whatwg.org/multipage/parsing.html#serialising-html-fragments>
  pub(crate) fn outer_html(self) -> Vec<u8> {
    enum Step<'a> {
      Open(NodeRef<'a>, Option<&'a ElementName>),
      Close(&'a ElementName),
    }

    let mut out = Vec::new();
    let mut steps = vec![Step::Open(self, None)];

    while let Some(step) = steps.pop() {
      match step {
        Step::Open(node, parent) => match node.data() {
          NodeData::Element { name, attrs, .. } => {
            out.push(b'<');
            out.extend_from_slice(name.local.as_bytes());

            for attr in attrs {
              out.push(b' ');
              out.extend_from_slice(match attr.ns {
                AttributeNamespace::None => b"",
                AttributeNamespace::Xlink => b"xlink:",
                AttributeNamespace::Xml => b"xml:",
                AttributeNamespace::Xmlns if &*attr.local == "xmlns" => b"",
                AttributeNamespace::Xmlns => b"xmlns:",
              });
              out.extend_from_slice(attr.local.as_bytes());
              out.extend_from_slice(b"=\"");
              escape_into(&mut out, &attr.value, true);
              out.push(b'"');
            }

            out.push(b'>');

            if name.ns == Namespace::Html && VOID_ELEMENTS.contains(&&*name.local) {
              continue;
            }

            steps.push(Step::Close(name));
            steps.extend(
              node
                .children()
                .rev()
                .map(|child| Step::Open(child, Some(name))),
            );
          }
          NodeData::Document => {
            steps.extend(node.children().rev().map(|child| Step::Open(child, None)))
          }
          NodeData::Doctype { name } => {
            out.extend_from_slice(b"<!DOCTYPE ");
            out.extend_from_slice(name.as_bytes());
            out.push(b'>');
          }
          NodeData::Text(contents) => {
            if parent.is_some_and(|parent| {
              parent.ns == Namespace::Html && UNESCAPED_TEXT_ELEMENTS.contains(&&*parent.local)
            }) {
              out.extend_from_slice(contents.as_bytes());
            } else {
              escape_into(&mut out, contents, false);
            }
          }
          NodeData::Comment(contents) => {
            out.extend_from_slice(b"<!--");
            out.extend_from_slice(contents.as_bytes());
            out.extend_from_slice(b"-->");
          }
        },
        Step::Close(name) => {
          out.extend_from_slice(b"</");
          out.extend_from_slice(name.local.as_bytes());
          out.push(b'>');
        }
      }
    }

    out
  }
}

/// HTML elements serialization writes without an end tag or children.
pub(crate) const VOID_ELEMENTS: [&str; 18] = [
  "area", "base", "basefont", "bgsound", "br", "col", "embed", "frame", "hr", "img", "input",
  "keygen", "link", "meta", "param", "source", "track", "wbr",
];

/// HTML elements whose text serialization writes unescaped, with scripting disabled.
const UNESCAPED_TEXT_ELEMENTS: [&str; 8] = [
  "style",
  "script",
  "xmp",
  "iframe",
  "noembed",
  "noframes",
  "plaintext",
  "noscript",
];

fn escape_into(out: &mut Vec<u8>, text: &str, attribute: bool) {
  for character in text.chars() {
    match character {
      '&' => out.extend_from_slice(b"&amp;"),
      '"' if attribute => out.extend_from_slice(b"&quot;"),
      '<' => out.extend_from_slice(b"&lt;"),
      '>' => out.extend_from_slice(b"&gt;"),
      '\u{a0}' => out.extend_from_slice(b"&nbsp;"),
      _ => {
        let mut buffer = [0; 4];

        out.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
      }
    }
  }
}
