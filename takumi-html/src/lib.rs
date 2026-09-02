#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![deny(missing_docs)]
//! Parse HTML markup into a takumi [`Node`] tree.
//!
//! ```rust
//! use takumi_core::layout::node::Node;
//! use takumi_html::{FromHtml, FromHtmlOptions};
//!
//! # fn main() -> Result<(), takumi_html::HtmlError> {
//! let node = Node::from_html("<div style=\"color:red\">Hi</div>", FromHtmlOptions::default())?;
//! # Ok(())
//! # }
//! ```

use std::{
  borrow::Cow,
  collections::{BTreeMap, HashMap},
  str::FromStr,
  sync::LazyLock,
};

use html5ever::{
  ParseOpts, QualName, local_name, ns, parse_document, parse_fragment,
  serialize::{SerializeOpts, TraversalScope, serialize},
  tendril::TendrilSink,
};
use markup5ever_rcdom::{Handle, NodeData, RcDom, SerializableHandle};
use takumi_core::{
  layout::node::{ImageData, ImageSourceInput, Node, NodeKind},
  style::{Direction, FromCssStr, Lang, Style, StyleDeclarationBlock, TailwindValues},
};
use typed_builder::TypedBuilder;

/// Tags whose entire subtree is dropped, matching the JS `isHtmlVoidElement`
/// set. Deliberately distinct from the `display:none` presets (`title`,
/// `noscript`, `template`, ...): these never reach the tree, those are merely
/// laid out hidden.
const VOID_TAGS: [&str; 5] = ["head", "meta", "link", "style", "script"];

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
  (
    "blockquote",
    "margin-top:1em;margin-bottom:1em;margin-left:40px;margin-right:40px;display:block",
  ),
  (
    "figure",
    "margin-top:1em;margin-bottom:1em;margin-left:40px;margin-right:40px;display:block",
  ),
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
  (
    "hr",
    "margin-top:0.5em;margin-bottom:0.5em;margin-left:auto;margin-right:auto;border-width:1px;display:block",
  ),
  (
    "ul",
    "margin-top:1em;margin-bottom:1em;padding-inline-start:40px;display:block;list-style-type:disc",
  ),
  (
    "ol",
    "margin-top:1em;margin-bottom:1em;padding-inline-start:40px;display:block;list-style-type:decimal",
  ),
  (
    "menu",
    "margin-top:1em;margin-bottom:1em;padding-inline-start:40px;display:block;list-style-type:disc",
  ),
  ("li", "display:list-item"),
  ("dl", "margin-top:1em;margin-bottom:1em;display:block"),
  ("dt", "display:block"),
  ("dd", "margin-left:40px;display:block"),
  ("form", "display:block"),
  // https://html.spec.whatwg.org/multipage/rendering.html#form-controls makes
  // these `inline-block`, and hides a closed `<select>`'s options through the
  // shadow tree rather than CSS. An inline-level box here has no rectangle of
  // its own for a widget annotation to cover.
  ("input", "display:block"),
  ("textarea", "display:block"),
  ("select", "display:block"),
  ("option", "display:none"),
  (
    "fieldset",
    "margin-left:2px;margin-right:2px;padding-top:0.35em;padding-right:0.75em;padding-bottom:0.625em;padding-left:0.75em;border-width:2px;display:block",
  ),
  ("legend", "padding-left:2px;padding-right:2px;display:block"),
  ("details", "display:block"),
  ("summary", "display:block"),
  ("search", "display:block"),
  (
    "h1",
    "font-size:2em;margin-top:0.67em;margin-bottom:0.67em;margin-left:0;margin-right:0;font-weight:bold;display:block",
  ),
  (
    "h2",
    "font-size:1.5em;margin-top:0.83em;margin-bottom:0.83em;margin-left:0;margin-right:0;font-weight:bold;display:block",
  ),
  (
    "h3",
    "font-size:1.17em;margin-top:1em;margin-bottom:1em;margin-left:0;margin-right:0;font-weight:bold;display:block",
  ),
  (
    "h4",
    "margin-top:1.33em;margin-bottom:1.33em;margin-left:0;margin-right:0;font-weight:bold;display:block",
  ),
  (
    "h5",
    "font-size:0.83em;margin-top:1.67em;margin-bottom:1.67em;margin-left:0;margin-right:0;font-weight:bold;display:block",
  ),
  (
    "h6",
    "font-size:0.67em;margin-top:2.33em;margin-bottom:2.33em;margin-left:0;margin-right:0;font-weight:bold;display:block",
  ),
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
  (
    "pre",
    "font-family:monospace;white-space:pre;margin:1em 0;display:block",
  ),
  ("mark", "background-color:yellow;color:black"),
  ("big", "font-size:larger"),
  ("small", "font-size:smaller"),
  ("s", "text-decoration:line-through"),
  ("del", "text-decoration:line-through"),
  ("sub", "font-size:smaller;vertical-align:sub"),
  ("sup", "font-size:smaller;vertical-align:super"),
  ("div", "display:block"),
  ("br", "white-space:pre"),
  (
    "table",
    "display:table;box-sizing:border-box;border-spacing:2px",
  ),
  ("thead", "display:table-header-group"),
  ("tbody", "display:table-row-group"),
  ("tfoot", "display:table-footer-group"),
  ("tr", "display:table-row"),
  ("td", "display:table-cell;padding:1px"),
  (
    "th",
    "display:table-cell;padding:1px;font-weight:bold;text-align:center",
  ),
  ("caption", "display:table-caption;text-align:center"),
];

static DEFAULT_STYLE_PRESETS: LazyLock<HashMap<Box<str>, Style>> = LazyLock::new(|| {
  DEFAULT_PRESETS
    .iter()
    .map(|&(tag, css)| (tag.into(), Style::from(parse_declarations(css))))
    .collect()
});

/// Default cap on element nesting depth, guarding the recursive walk against
/// stack overflow on hostile input. Matches Blink's limit.
pub const DEFAULT_MAX_DEPTH: usize = 512;

/// Errors raised while parsing HTML into a node tree.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum HtmlError {
  /// An `<img>` element lacked the required `src` attribute.
  #[error("image element must have a `src` attribute")]
  MissingImageSrc,
  /// Element nesting exceeded [`FromHtmlOptions::max_depth`].
  #[error("element nesting exceeded the maximum depth of {0}")]
  MaxDepthExceeded(usize),
}

/// Per-tag default styles applied at the lowest cascade layer.
///
/// [`StylePresets::chromium`] is the built-in table, borrowed from a shared
/// static at no allocation cost. Supply your own with [`From`]; disable presets
/// with [`StylePresets::empty`].
#[derive(Debug, Clone)]
pub struct StylePresets(Cow<'static, HashMap<Box<str>, Style>>);

impl StylePresets {
  /// Built-in Chromium element presets.
  pub fn chromium() -> Self {
    Self(Cow::Borrowed(&DEFAULT_STYLE_PRESETS))
  }

  /// Empty presets — every element keeps its bare style.
  pub fn empty() -> Self {
    Self(Cow::Owned(HashMap::new()))
  }

  fn get(&self, tag: &str) -> Option<&Style> {
    self.0.get(tag)
  }
}

impl Default for StylePresets {
  fn default() -> Self {
    Self::chromium()
  }
}

impl From<HashMap<Box<str>, Style>> for StylePresets {
  fn from(presets: HashMap<Box<str>, Style>) -> Self {
    Self(Cow::Owned(presets))
  }
}

/// Options for [`from_html`]. Construct via [`FromHtmlOptions::builder`], or
/// [`default`](Self::default) for the defaults.
#[derive(Debug, Clone, TypedBuilder)]
#[non_exhaustive]
pub struct FromHtmlOptions {
  /// Default element styles. Use [`StylePresets::empty`] to disable.
  #[builder(default)]
  pub(crate) presets: StylePresets,
  /// Attribute name carrying Tailwind classes. `None` means `tw`.
  #[builder(default, setter(into, strip_option))]
  pub(crate) tailwind_property: Option<Box<str>>,
  /// Maximum element nesting depth before [`HtmlError::MaxDepthExceeded`].
  /// Defaults to [`DEFAULT_MAX_DEPTH`].
  #[builder(default = DEFAULT_MAX_DEPTH)]
  pub(crate) max_depth: usize,
}

impl Default for FromHtmlOptions {
  fn default() -> Self {
    Self::builder().build()
  }
}

/// Parse HTML markup into a node tree.
///
/// `tw`, `style`, `class`, `id`, `dir`, and `lang` attributes become the
/// corresponding node styling and metadata; `<style>` blocks and other void
/// elements are dropped. A single root element is returned as-is; multiple
/// roots are wrapped in a full-size container.
///
/// A source that starts with an `<html>` element is parsed as a document, so
/// the tree keeps that element as its root along with `<head>` and `<body>`.
/// Anything else is parsed as a fragment and gains no wrappers of its own.
pub fn from_html(source: &str, options: FromHtmlOptions) -> Result<Node, HtmlError> {
  let tw_property = options.tailwind_property.as_deref().unwrap_or("tw");

  let mut nodes = Vec::new();
  let build = |handle: &Handle, nodes: &mut Vec<Node>| {
    build_nodes(
      handle,
      &options.presets,
      tw_property,
      options.max_depth,
      0,
      nodes,
    )
  };

  if starts_with_html_element(source) {
    let dom = parse_document(RcDom::default(), ParseOpts::default()).one(source);

    build(&dom.document, &mut nodes)?;
  } else {
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
    if let Some(context) = dom.document.children.borrow().first() {
      for child in context.children.borrow().iter() {
        build(child, &mut nodes)?;
      }
    }
  }

  Ok(collapse(nodes))
}

/// Whether the source opens with an `<html>` tag, ignoring leading whitespace,
/// a doctype, and comments.
///
/// Document parsing invents `<html>`, `<head>` and `<body>` for a source that
/// has none, which would wrap every fragment in boxes its author never wrote.
fn starts_with_html_element(source: &str) -> bool {
  let mut rest = source.trim_start();

  loop {
    let lowered = rest.get(..9).map(str::to_ascii_lowercase);

    if lowered
      .as_deref()
      .is_some_and(|s| s.starts_with("<!doctype"))
    {
      let Some((_, after)) = rest.split_once('>') else {
        return false;
      };
      rest = after.trim_start();
      continue;
    }

    if rest.starts_with("<!--") {
      let Some((_, after)) = rest.split_once("-->") else {
        return false;
      };
      rest = after.trim_start();
      continue;
    }

    return rest
      .split_at_checked("<html".len())
      .filter(|(prefix, _)| prefix.eq_ignore_ascii_case("<html"))
      .is_some_and(|(_, after)| {
        after.starts_with(['>', '/']) || after.starts_with(char::is_whitespace)
      });
  }
}

/// Adds [`Node::from_html`](FromHtml::from_html) when this crate is in scope.
pub trait FromHtml: Sized {
  /// Parse HTML markup into a node tree. See [`from_html`].
  fn from_html(source: &str, options: FromHtmlOptions) -> Result<Self, HtmlError>;
}

impl FromHtml for Node {
  fn from_html(source: &str, options: FromHtmlOptions) -> Result<Self, HtmlError> {
    from_html(source, options)
  }
}

fn is_whitespace_only_text(node: &Node) -> bool {
  matches!(&node.kind, NodeKind::Text(data) if data.text.trim().is_empty())
}

/// Apply the root-collapse rule shared with the JS `fromHtml`.
fn collapse(mut nodes: Vec<Node>) -> Node {
  // Trim whitespace-only roots so a newline-wrapped single element stays one root.
  while nodes.first().is_some_and(is_whitespace_only_text) {
    nodes.remove(0);
  }
  while nodes.last().is_some_and(is_whitespace_only_text) {
    nodes.pop();
  }

  match nodes.len() {
    0 => Node::container([]),
    1 => nodes.pop().unwrap_or_default(),
    _ => Node::container(nodes).with_style(Style::from(parse_declarations(
      "display:block;width:100%;height:100%",
    ))),
  }
}

fn build_nodes(
  handle: &Handle,
  presets: &StylePresets,
  tw_property: &str,
  max_depth: usize,
  depth: usize,
  out: &mut Vec<Node>,
) -> Result<(), HtmlError> {
  match &handle.data {
    NodeData::Comment { .. }
    | NodeData::Doctype { .. }
    | NodeData::ProcessingInstruction { .. } => {}
    NodeData::Document => {
      for child in handle.children.borrow().iter() {
        build_nodes(child, presets, tw_property, max_depth, depth, out)?;
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

      if let Some(node) = build_element(handle, tag, presets, tw_property, max_depth, depth)? {
        out.push(node);
      }
    }
  }

  Ok(())
}

fn build_element(
  handle: &Handle,
  tag: &str,
  presets: &StylePresets,
  tw_property: &str,
  max_depth: usize,
  depth: usize,
) -> Result<Option<Node>, HtmlError> {
  if depth >= max_depth {
    return Err(HtmlError::MaxDepthExceeded(max_depth));
  }

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
      .filter(|src| !src.trim().is_empty())
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
      src: ImageSourceInput::Buffer(serialize_outer_html(handle)),
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
    build_nodes(
      child,
      presets,
      tw_property,
      max_depth,
      depth + 1,
      &mut children,
    )?;
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
  presets: &StylePresets,
  tw_property: &str,
) -> Node {
  node = node.with_tag_name(tag);

  if let Some(preset) = presets.get(tag) {
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
      "lang" => {
        if let Ok(lang) = Lang::parse(value) {
          node = node.with_lang(lang);
        }
      }
      "dir" => {
        if let Ok(dir) = Direction::from_css_str(value) {
          node = node.with_dir(dir);
        }
      }
      "style" => node = node.with_style(Style::from(parse_declarations(value))),
      // Consumed into `ImageData`; re-emitted by serialization, so keep them out
      // of the passthrough attributes to avoid duplicating on round-trip.
      "src" if tag == "img" => {}
      "width" | "height" if matches!(tag, "img" | "svg") => {}
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

/// Serialize an element including its own tag (outer HTML), used to round-trip
/// `<svg>` subtrees into an image source.
fn serialize_outer_html(handle: &Handle) -> Vec<u8> {
  let mut buffer = Vec::new();
  let serializable: SerializableHandle = handle.clone().into();
  let opts = SerializeOpts {
    traversal_scope: TraversalScope::IncludeNode,
    ..SerializeOpts::default()
  };

  if serialize(&mut buffer, &serializable, opts).is_err() {
    return Vec::new();
  }

  buffer
}

/// Parse a CSS declaration block, ignoring it if it fails to parse. Parses the
/// whole block so values containing `;` (e.g. `data:` URIs) survive.
fn parse_declarations(css: &str) -> StyleDeclarationBlock {
  StyleDeclarationBlock::parse_loosy(css)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse(source: &str) -> Node {
    from_html(source, FromHtmlOptions::default()).unwrap()
  }

  /// A `style` attribute is a declaration list, and CSS drops only the declaration it
  /// cannot read. `fit-content` is not a width this crate understands, and it used to take
  /// the whole attribute down with it.
  #[test]
  fn an_unreadable_declaration_leaves_its_neighbours_alone() {
    let block = parse_declarations("font-size:64px;width:fit-content;color:red");

    assert_eq!(block.len(), 2);
  }

  #[test]
  fn builtin_presets_parse() {
    let presets = StylePresets::default();
    for (tag, _) in DEFAULT_PRESETS {
      let style = presets.get(tag).expect("preset present");
      assert!(
        !style.declarations.is_empty(),
        "preset `{tag}` produced no declarations",
      );
    }
  }

  /// A list needs the counter style on the list and `display: list-item` on the
  /// item for the renderer to draw a marker.
  #[test]
  fn list_presets_carry_the_marker_styles() {
    for (tag, expected) in [
      ("ol", "list-style-type:decimal"),
      ("ul", "list-style-type:disc"),
      ("li", "display:list-item"),
    ] {
      let (_, css) = DEFAULT_PRESETS
        .iter()
        .find(|(name, _)| *name == tag)
        .expect("preset present");

      assert!(
        css.contains(expected),
        "preset `{tag}` is missing `{expected}`"
      );
    }
  }

  #[test]
  fn a_document_keeps_its_html_root() {
    let node = parse(
      r#"<!doctype html><html style="font-size:62.5%"><head><title>t</title></head><body><div>hi</div></body></html>"#,
    );

    assert_eq!(node.tag_name(), Some("html"));
    assert!(node.to_html().contains("font-size: 62.5%"));

    assert!(node.to_html().contains("<body"));
  }

  #[test]
  fn a_fragment_gains_no_wrappers() {
    assert_eq!(parse("<div>hi</div>").tag_name(), Some("div"));
    assert_eq!(parse("<body><div>hi</div></body>").tag_name(), Some("div"));
    assert_eq!(parse("text <html> in content").tag_name(), None);
  }

  /// Tag names are ASCII case-insensitive, so a document is a document however
  /// its author spelled the root.
  #[test]
  fn a_document_root_is_matched_case_insensitively() {
    assert_eq!(
      parse("<Html><body>hi</body></Html>").tag_name(),
      Some("html")
    );
    assert_eq!(
      parse("<HTML><body>hi</body></HTML>").tag_name(),
      Some("html")
    );
  }

  #[test]
  fn whitespace_around_single_root_is_trimmed() {
    let node = parse("\n  <div>x</div>\n");
    assert!(matches!(node.kind, NodeKind::Text(_)));
    assert!(node.to_html().starts_with("<div"));
  }

  #[test]
  fn tailwind_property_can_alias_reserved_name() {
    let node = from_html(
      r#"<div class="flex">x</div>"#,
      FromHtmlOptions::builder()
        .presets(StylePresets::empty())
        .tailwind_property("class")
        .build(),
    )
    .unwrap();
    assert!(node.to_html().contains(r#"class="flex""#));
  }

  #[test]
  fn tailwind_consumed_not_passthrough() {
    let node = parse(r#"<div tw="flex">x</div>"#);
    assert!(!node.to_html().contains("tw="));
  }

  #[test]
  fn dir_is_case_insensitive() {
    let node = parse(r#"<div dir="RTL">x</div>"#);
    assert!(node.to_html().contains(r#"dir="rtl""#));
  }

  #[test]
  fn empty_img_src_rejected() {
    assert!(matches!(
      from_html(r#"<img src="">"#, FromHtmlOptions::default()),
      Err(HtmlError::MissingImageSrc),
    ));
  }

  #[test]
  fn img_dimensions_parsed() {
    let node = parse(r#"<img src="a.png" width="10" height="20">"#);
    let NodeKind::Image(data) = &node.kind else {
      panic!("expected image");
    };
    assert_eq!(data.width, Some(10.0));
    assert_eq!(data.height, Some(20.0));
  }

  #[test]
  fn text_only_element_becomes_text() {
    let node = parse("<p>hello</p>");
    let NodeKind::Text(data) = &node.kind else {
      panic!("expected text node");
    };
    assert_eq!(data.text, "hello");
    assert!(node.to_html().starts_with("<p"));
  }

  #[test]
  fn multiple_roots_wrapped() {
    let node = parse("<div>a</div><div>b</div>");
    let NodeKind::Container { children } = &node.kind else {
      panic!("expected container");
    };
    assert_eq!(children.len(), 2);
    assert!(node.to_html().contains("100%"));
  }

  #[test]
  fn inline_style_and_attributes() {
    let node = parse(r#"<div style="color:red" data-x="1">x</div>"#);
    let html = node.to_html();
    assert!(html.contains(r#"data-x="1""#));
    assert!(html.contains("color"));
  }

  #[test]
  fn presets_disabled() {
    let node = from_html(
      "<p>x</p>",
      FromHtmlOptions::builder()
        .presets(StylePresets::empty())
        .build(),
    )
    .unwrap();
    assert!(!node.to_html().contains("style="));
  }

  #[test]
  fn nesting_past_max_depth_is_rejected() {
    let src = format!("{}{}", "<div>".repeat(4), "</div>".repeat(4));
    let opts = FromHtmlOptions::builder().max_depth(2).build();
    assert!(matches!(
      from_html(&src, opts),
      Err(HtmlError::MaxDepthExceeded(2)),
    ));
  }

  #[test]
  fn void_elements_dropped() {
    let node = parse("<style>.a{color:red}</style><div>x</div>");
    assert!(node.to_html().starts_with("<div"));
  }
}
