//! Parse HTML markup into a [`Node`] tree.
//!
//! Mirrors the JavaScript `fromHtml()` helper so a Rust server can turn an
//! HTML + Tailwind template into a renderable tree without a Node.js sidecar.

use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;
use std::sync::LazyLock;

use html5ever::serialize::{SerializeOpts, TraversalScope, serialize};
use html5ever::tendril::TendrilSink;
use html5ever::{ParseOpts, QualName, local_name, ns, parse_fragment};
use markup5ever_rcdom::{Handle, NodeData, RcDom, SerializableHandle};

use crate::layout::node::{ImageData, ImageSourceInput, Node};
use crate::layout::style::{Direction, Style, StyleDeclarationBlock, tw::TailwindValues};

/// Tags whose content is dropped, matching the JS `isHtmlVoidElement` set.
const VOID_TAGS: [&str; 5] = ["head", "meta", "link", "style", "script"];

/// Default element style presets, ported from
/// `takumi-helpers/src/jsx/style-presets.ts`. Keep in sync.
///
/// Numeric CSS values from the TS source are written with explicit `px` units.
#[rustfmt::skip]
const DEFAULT_PRESETS: &[(&str, &str)] = &[
    ("html", "display:block"),
    ("head", "display:none"),
    ("meta", "display:none"),
    ("title", "display:none"),
    ("link", "display:none"),
    ("style", "display:none"),
    ("script", "display:none"),
    ("noscript", "display:none"),
    ("datalist", "display:none"),
    ("template", "display:none"),
    ("body", "margin:8px;display:block"),
    ("p", "margin-top:1em;margin-bottom:1em;display:block"),
    ("blockquote", "margin-top:1em;margin-bottom:1em;margin-left:40px;margin-right:40px;display:block"),
    ("figure", "margin-top:1em;margin-bottom:1em;margin-left:40px;margin-right:40px;display:block"),
    ("figcaption", "display:block"),
    ("address", "font-style:italic;display:block"),
    ("article", "display:block"),
    ("aside", "display:block"),
    ("footer", "display:block"),
    ("header", "display:block"),
    ("hgroup", "display:block"),
    ("main", "display:block"),
    ("nav", "display:block"),
    ("section", "display:block"),
    ("center", "text-align:center;display:block"),
    ("hr", "margin-top:0.5em;margin-bottom:0.5em;margin-left:auto;margin-right:auto;border-width:1px;display:block"),
    ("ul", "margin-top:1em;margin-bottom:1em;padding-left:40px;display:block"),
    ("ol", "margin-top:1em;margin-bottom:1em;padding-left:40px;display:block"),
    ("menu", "margin-top:1em;margin-bottom:1em;padding-left:40px;display:block"),
    ("li", "display:block"),
    ("dl", "margin-top:1em;margin-bottom:1em;display:block"),
    ("dt", "display:block"),
    ("dd", "margin-left:40px;display:block"),
    ("form", "display:block"),
    ("fieldset", "margin-left:2px;margin-right:2px;padding-top:0.35em;padding-right:0.75em;padding-bottom:0.625em;padding-left:0.75em;border-width:2px;display:block"),
    ("legend", "padding-left:2px;padding-right:2px;display:block"),
    ("details", "display:block"),
    ("summary", "display:block"),
    ("search", "display:block"),
    ("h1", "font-size:2em;margin-top:0.67em;margin-bottom:0.67em;margin-left:0;margin-right:0;font-weight:bold;display:block"),
    ("h2", "font-size:1.5em;margin-top:0.83em;margin-bottom:0.83em;margin-left:0;margin-right:0;font-weight:bold;display:block"),
    ("h3", "font-size:1.17em;margin-top:1em;margin-bottom:1em;margin-left:0;margin-right:0;font-weight:bold;display:block"),
    ("h4", "margin-top:1.33em;margin-bottom:1.33em;margin-left:0;margin-right:0;font-weight:bold;display:block"),
    ("h5", "font-size:0.83em;margin-top:1.67em;margin-bottom:1.67em;margin-left:0;margin-right:0;font-weight:bold;display:block"),
    ("h6", "font-size:0.67em;margin-top:2.33em;margin-bottom:2.33em;margin-left:0;margin-right:0;font-weight:bold;display:block"),
    ("u", "text-decoration:underline"),
    ("ins", "text-decoration:underline"),
    ("strong", "font-weight:bolder"),
    ("b", "font-weight:bolder"),
    ("i", "font-style:italic"),
    ("em", "font-style:italic"),
    ("cite", "font-style:italic"),
    ("dfn", "font-style:italic"),
    ("code", "font-family:monospace"),
    ("kbd", "font-family:monospace"),
    ("samp", "font-family:monospace"),
    ("pre", "font-family:monospace;white-space:pre;margin:1em 0;display:block"),
    ("mark", "background-color:yellow;color:black"),
    ("big", "font-size:larger"),
    ("small", "font-size:smaller"),
    ("s", "text-decoration:line-through"),
    ("del", "text-decoration:line-through"),
    ("sub", "font-size:smaller;vertical-align:sub"),
    ("sup", "font-size:smaller;vertical-align:super"),
    ("div", "display:block"),
];

static DEFAULT_STYLE_PRESETS: LazyLock<HashMap<Box<str>, Style>> = LazyLock::new(|| {
  DEFAULT_PRESETS
    .iter()
    .map(|&(tag, css)| (tag.into(), Style::from(parse_declarations(css))))
    .collect()
});

/// Errors raised while parsing HTML into a node tree.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum HtmlError {
  /// An `<img>` element lacked the required `src` attribute.
  #[error("image element must have a `src` attribute")]
  MissingImageSrc,
}

/// Per-tag default styles applied at the lowest cascade layer.
///
/// [`StylePresets::default`] is the built-in Chromium table, shared by reference
/// at no allocation cost. Supply your own with [`From`]; disable presets via
/// `FromHtmlOptions { presets: None, .. }`.
#[derive(Debug, Clone, Default)]
pub enum StylePresets {
  /// The built-in Chromium element presets.
  #[default]
  Builtin,
  /// Caller-supplied presets, fully replacing the built-in table.
  Custom(HashMap<Box<str>, Style>),
}

impl StylePresets {
  fn get(&self, tag: &str) -> Option<&Style> {
    match self {
      Self::Builtin => DEFAULT_STYLE_PRESETS.get(tag),
      Self::Custom(presets) => presets.get(tag),
    }
  }
}

impl From<HashMap<Box<str>, Style>> for StylePresets {
  fn from(presets: HashMap<Box<str>, Style>) -> Self {
    Self::Custom(presets)
  }
}

impl From<&[(&str, &str)]> for StylePresets {
  /// Build presets from `(tag, css-declarations)` pairs. Declarations that
  /// fail to parse are dropped, matching the JS object path's ignore-unknown
  /// behavior.
  fn from(entries: &[(&str, &str)]) -> Self {
    Self::Custom(
      entries
        .iter()
        .map(|&(tag, css)| (tag.into(), Style::from(parse_declarations(css))))
        .collect(),
    )
  }
}

/// Options for [`Node::from_html`].
#[derive(Debug, Clone)]
pub struct FromHtmlOptions {
  /// Default element styles. `None` disables presets entirely.
  pub presets: Option<StylePresets>,
  /// Attribute name carrying Tailwind classes. `None` means `tw`.
  pub tailwind_property: Option<Box<str>>,
}

impl Default for FromHtmlOptions {
  fn default() -> Self {
    Self {
      presets: Some(StylePresets::default()),
      tailwind_property: None,
    }
  }
}

impl Node {
  /// Parse HTML markup into a node tree.
  ///
  /// `tw`, `style`, and `class` attributes become the corresponding node
  /// styling; `<style>` blocks and other void elements are dropped. A single
  /// root element is returned as-is; multiple roots are wrapped in a
  /// full-size container.
  pub fn from_html(source: &str, options: FromHtmlOptions) -> Result<Node, HtmlError> {
    let tw_property = options.tailwind_property.as_deref().unwrap_or("tw");

    let context = QualName::new(None, ns!(html), local_name!("body"));
    let dom = parse_fragment(
      RcDom::default(),
      ParseOpts::default(),
      context,
      vec![],
      false,
    )
    .one(source);

    // `parse_fragment` wraps the roots in a synthetic context element, the
    // document's only child.
    let mut nodes = Vec::new();
    if let Some(context) = dom.document.children.borrow().first() {
      for child in context.children.borrow().iter() {
        build_nodes(child, &options.presets, tw_property, &mut nodes)?;
      }
    }

    Ok(collapse(nodes))
  }
}

/// Apply the root-collapse rule shared with the JS `fromHtml`.
fn collapse(mut nodes: Vec<Node>) -> Node {
  // Trim whitespace-only roots so a newline-wrapped single element stays one root.
  while nodes.first().is_some_and(Node::is_whitespace_only_text) {
    nodes.remove(0);
  }
  while nodes.last().is_some_and(Node::is_whitespace_only_text) {
    nodes.pop();
  }

  match nodes.len() {
    0 => Node::container([]),
    1 => nodes.pop().unwrap_or_default(),
    _ => {
      Node::container(nodes).with_style(Style::from(parse_declarations("width:100%;height:100%")))
    }
  }
}

fn build_nodes(
  handle: &Handle,
  presets: &Option<StylePresets>,
  tw_property: &str,
  out: &mut Vec<Node>,
) -> Result<(), HtmlError> {
  match &handle.data {
    NodeData::Comment { .. }
    | NodeData::Doctype { .. }
    | NodeData::ProcessingInstruction { .. } => {}
    NodeData::Document => {
      for child in handle.children.borrow().iter() {
        build_nodes(child, presets, tw_property, out)?;
      }
    }
    NodeData::Text { contents } => {
      let value = contents.borrow();
      if !value.is_empty() {
        out.push(Node::text(value.to_string()));
      }
    }
    NodeData::Element { name, .. } => {
      let tag = name.local.as_ref();

      if let Some(node) = build_element(handle, tag, presets, tw_property)? {
        out.push(node);
      }
    }
  }

  Ok(())
}

fn build_element(
  handle: &Handle,
  tag: &str,
  presets: &Option<StylePresets>,
  tw_property: &str,
) -> Result<Option<Node>, HtmlError> {
  if tag == "br" {
    return Ok(Some(apply_metadata(
      Node::text("\n"),
      handle,
      tag,
      presets,
      tw_property,
    )));
  }

  if tag == "img" {
    let src = attribute(handle, "src")
      .filter(|src| !src.is_empty())
      .ok_or(HtmlError::MissingImageSrc)?;
    let image = ImageData {
      src: ImageSourceInput::Url(src.into()),
      width: dimension(handle, "width"),
      height: dimension(handle, "height"),
    };

    return Ok(Some(apply_metadata(
      Node::image(image),
      handle,
      tag,
      presets,
      tw_property,
    )));
  }

  if VOID_TAGS.contains(&tag) {
    return Ok(None);
  }

  if tag == "svg" {
    let image = ImageData {
      src: ImageSourceInput::Url(serialize_outer_html(handle).into()),
      width: dimension(handle, "width"),
      height: dimension(handle, "height"),
    };

    return Ok(Some(apply_metadata(
      Node::image(image),
      handle,
      tag,
      presets,
      tw_property,
    )));
  }

  if let Some(text) = text_only_contents(handle) {
    return Ok(Some(apply_metadata(
      Node::text(text),
      handle,
      tag,
      presets,
      tw_property,
    )));
  }

  let mut children = Vec::new();
  for child in handle.children.borrow().iter() {
    build_nodes(child, presets, tw_property, &mut children)?;
  }

  Ok(Some(apply_metadata(
    Node::container(children),
    handle,
    tag,
    presets,
    tw_property,
  )))
}

/// Concatenated text if every child is a text node, else `None`. Comments are
/// ignored, mirroring the JS markup walker.
fn text_only_contents(handle: &Handle) -> Option<String> {
  let mut text = String::new();

  for child in handle.children.borrow().iter() {
    match &child.data {
      NodeData::Comment { .. } => continue,
      NodeData::Text { contents } => text.push_str(&contents.borrow()),
      _ => return None,
    }
  }

  (!text.is_empty()).then_some(text)
}

fn apply_metadata(
  mut node: Node,
  handle: &Handle,
  tag: &str,
  presets: &Option<StylePresets>,
  tw_property: &str,
) -> Node {
  node = node.with_tag_name(tag);

  if let Some(preset) = presets.as_ref().and_then(|presets| presets.get(tag)) {
    node = node.with_preset(preset.clone());
  }

  let NodeData::Element { attrs, .. } = &handle.data else {
    return node;
  };

  let mut attributes = BTreeMap::new();
  for attr in attrs.borrow().iter() {
    let name = attr.name.local.as_ref();
    let value = attr.value.as_ref();

    // Read Tailwind independently of reserved names so it can alias `class`
    // without dropping the class name.
    if name == tw_property
      && let Ok(tw) = TailwindValues::from_str(value)
    {
      node = node.with_tw(tw);
    }

    match name {
      "class" => node = node.with_class_name(value),
      "id" => node = node.with_id(value),
      "lang" => node = node.with_lang(value),
      "dir" => {
        if let Some(dir) = parse_direction(value) {
          node = node.with_dir(dir);
        }
      }
      "style" => node = node.with_style(Style::from(parse_declarations(value))),
      // Consumed above; keep out of the passthrough attributes.
      _ if name == tw_property => {}
      _ => {
        attributes.insert(name.into(), value.into());
      }
    }
  }

  if !attributes.is_empty() {
    node = node.with_attributes(attributes);
  }

  node
}

fn attribute(handle: &Handle, name: &str) -> Option<String> {
  let NodeData::Element { attrs, .. } = &handle.data else {
    return None;
  };

  attrs
    .borrow()
    .iter()
    .find(|attr| attr.name.local.as_ref() == name)
    .map(|attr| attr.value.to_string())
}

fn dimension(handle: &Handle, name: &str) -> Option<f32> {
  attribute(handle, name).and_then(|value| value.parse().ok())
}

fn parse_direction(value: &str) -> Option<Direction> {
  let value = value.trim();

  if value.eq_ignore_ascii_case("ltr") {
    Some(Direction::Ltr)
  } else if value.eq_ignore_ascii_case("rtl") {
    Some(Direction::Rtl)
  } else {
    None
  }
}

/// Serialize an element including its own tag (outer HTML), used to round-trip
/// `<svg>` subtrees into an image source.
fn serialize_outer_html(handle: &Handle) -> String {
  let mut buffer = Vec::new();
  let serializable: SerializableHandle = handle.clone().into();
  let opts = SerializeOpts {
    traversal_scope: TraversalScope::IncludeNode,
    ..SerializeOpts::default()
  };

  if serialize(&mut buffer, &serializable, opts).is_err() {
    return String::new();
  }

  String::from_utf8_lossy(&buffer).into_owned()
}

/// Parse a CSS declaration block, dropping declarations that fail to parse so
/// one unsupported value does not discard the rest.
fn parse_declarations(css: &str) -> StyleDeclarationBlock {
  let mut block = StyleDeclarationBlock::default();

  for declaration in css.split(';') {
    let declaration = declaration.trim();
    if declaration.is_empty() {
      continue;
    }

    if let Ok(parsed) = StyleDeclarationBlock::from_str(declaration) {
      block.append(parsed);
    }
  }

  block
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::layout::node::NodeKind;

  fn parse(source: &str) -> Node {
    Node::from_html(source, FromHtmlOptions::default()).unwrap()
  }

  #[test]
  fn builtin_presets_parse() {
    let presets = StylePresets::default();
    for (tag, _) in DEFAULT_PRESETS {
      let style = presets.get(tag).expect("preset present");
      assert!(
        !style.declarations.declarations.is_empty(),
        "preset `{tag}` produced no declarations",
      );
    }
  }

  #[test]
  fn whitespace_around_single_root_is_trimmed() {
    let node = parse("\n  <div>x</div>\n");
    assert_eq!(node.metadata.tag_name.as_deref(), Some("div"));
  }

  #[test]
  fn tailwind_property_can_alias_reserved_name() {
    let node = Node::from_html(
      r#"<div class="flex">x</div>"#,
      FromHtmlOptions {
        presets: None,
        tailwind_property: Some("class".into()),
      },
    )
    .unwrap();
    assert!(node.metadata.tw.is_some());
    assert_eq!(node.metadata.class_name.as_deref(), Some("flex"));
  }

  #[test]
  fn dir_is_case_insensitive() {
    let node = parse(r#"<div dir="RTL">x</div>"#);
    assert_eq!(node.metadata.dir, Some(Direction::Rtl));
  }

  #[test]
  fn empty_img_src_rejected() {
    assert!(matches!(
      Node::from_html(r#"<img src="">"#, FromHtmlOptions::default()),
      Err(HtmlError::MissingImageSrc),
    ));
  }

  #[test]
  fn single_root_returned_directly() {
    let node = parse(r#"<div tw="flex"><span>a</span><span>b</span></div>"#);
    assert_eq!(node.metadata.tag_name.as_deref(), Some("div"));
    assert!(node.metadata.tw.is_some());
    assert!(matches!(node.kind, NodeKind::Container { .. }));
  }

  #[test]
  fn text_only_element_becomes_text() {
    let node = parse("<p>hello</p>");
    let NodeKind::Text(data) = &node.kind else {
      panic!("expected text node");
    };
    assert_eq!(data.text, "hello");
    assert_eq!(node.metadata.tag_name.as_deref(), Some("p"));
    assert!(node.metadata.preset.is_some());
  }

  #[test]
  fn multiple_roots_wrapped() {
    let node = parse("<div>a</div><div>b</div>");
    let NodeKind::Container { children } = &node.kind else {
      panic!("expected container");
    };
    assert_eq!(children.len(), 2);
    assert!(node.metadata.style.is_some());
  }

  #[test]
  fn inline_style_and_attributes() {
    let node = parse(r#"<div style="color:red" data-x="1">x</div>"#);
    assert!(node.metadata.style.is_some());
    assert_eq!(
      node
        .metadata
        .attributes
        .as_ref()
        .and_then(|a| a.get("data-x"))
        .map(AsRef::as_ref),
      Some("1"),
    );
  }

  #[test]
  fn img_requires_src() {
    assert!(matches!(
      Node::from_html("<img>", FromHtmlOptions::default()),
      Err(HtmlError::MissingImageSrc),
    ));

    let node = parse(r#"<img src="a.png" width="10" height="20">"#);
    let NodeKind::Image(data) = &node.kind else {
      panic!("expected image");
    };
    assert_eq!(data.width, Some(10.0));
    assert_eq!(data.height, Some(20.0));
  }

  #[test]
  fn presets_disabled() {
    let node = Node::from_html(
      "<p>x</p>",
      FromHtmlOptions {
        presets: None,
        tailwind_property: None,
      },
    )
    .unwrap();
    assert!(node.metadata.preset.is_none());
  }

  #[test]
  fn void_elements_dropped() {
    let node = parse("<style>.a{color:red}</style><div>x</div>");
    assert_eq!(node.metadata.tag_name.as_deref(), Some("div"));
  }
}
