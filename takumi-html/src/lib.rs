#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![deny(missing_docs)]
//! Parse HTML markup into a takumi [`Node`] tree.
//!
//! The `style`, `tw` (Tailwind classes), `class`, `id`, `dir`, and `lang` attributes carry
//! over. Elements get Chromium's default styles unless [`FromHtmlOptions`] sets other presets.
//! `<style>` and `<script>` elements are dropped, so pass CSS to the renderer as a stylesheet.
//!
//! In the `takumi` crate, enable the `from-html` feature and call `takumi::from_html`.
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

mod dom;
mod entities;
mod markup;
mod named_references;

use std::{
  borrow::Cow,
  cell::RefCell,
  collections::{BTreeMap, HashMap},
  str::FromStr,
  sync::{Arc, LazyLock},
};

#[cfg(feature = "svg")]
use takumi_core::resources::image::{ImageSource, SvgSource};
use takumi_core::{
  layout::node::{ImageData, ImageSourceInput, MAXIMUM_DOM_TREE_DEPTH, Node, NodeKind},
  style::{
    Direction, Display, FromCssStr, Lang, Length, Style, StyleDeclaration, StyleDeclarationBlock,
    TailwindValues,
  },
};
use typed_builder::TypedBuilder;

#[cfg(feature = "svg")]
use crate::dom::AttributeNamespace;
use crate::dom::{NodeData, NodeRef};

/// Tags whose entire subtree is dropped, matching the JS markup walker.
/// Deliberately distinct from the `display:none` presets (`title`,
/// `noscript`, `template`, ...): these never reach the tree, those are merely
/// laid out hidden.
const UNRENDERED_TAGS: [&str; 5] = ["head", "meta", "link", "style", "script"];

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
  (
    "table",
    "display:table;box-sizing:border-box;border-spacing:2px",
  ),
  (
    "thead",
    "display:table-header-group;vertical-align:middle;break-inside:avoid",
  ),
  ("tbody", "display:table-row-group;vertical-align:middle"),
  (
    "tfoot",
    "display:table-footer-group;vertical-align:middle;break-inside:avoid",
  ),
  ("tr", "display:table-row;vertical-align:inherit"),
  (
    "td",
    "display:table-cell;padding:1px;vertical-align:inherit",
  ),
  (
    "th",
    "display:table-cell;padding:1px;vertical-align:inherit;font-weight:bold;text-align:center",
  ),
  ("caption", "display:table-caption;text-align:center"),
  ("colgroup", "display:table-column-group"),
  ("col", "display:table-column"),
];

static DEFAULT_STYLE_PRESETS: LazyLock<HashMap<Box<str>, Arc<Style>>> = LazyLock::new(|| {
  DEFAULT_PRESETS
    .iter()
    .map(|&(tag, css)| {
      (
        tag.into(),
        Arc::new(Style::from(StyleDeclarationBlock::parse_loosy(css))),
      )
    })
    .collect()
});

/// Errors raised while parsing HTML into a node tree.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum HtmlError {
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
pub struct StylePresets(Cow<'static, HashMap<Box<str>, Arc<Style>>>);

impl StylePresets {
  /// Built-in Chromium element presets.
  pub fn chromium() -> Self {
    Self(Cow::Borrowed(&DEFAULT_STYLE_PRESETS))
  }

  /// Empty presets — every element keeps its bare style.
  pub fn empty() -> Self {
    Self(Cow::Owned(HashMap::new()))
  }

  /// The preset of `tag`, shared until an element changes its copy.
  fn get(&self, tag: &str) -> Option<&Arc<Style>> {
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
    Self(Cow::Owned(
      presets
        .into_iter()
        .map(|(tag, style)| (tag, Arc::new(style)))
        .collect(),
    ))
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
  /// Defaults to [`MAXIMUM_DOM_TREE_DEPTH`].
  #[builder(default = MAXIMUM_DOM_TREE_DEPTH)]
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
/// corresponding node styling and metadata; `<style>` blocks and other
/// unrendered elements are dropped. A single root element is returned as-is; multiple
/// roots are wrapped in a full-size container.
///
/// A source that starts with a doctype or an `<html>` element is parsed as a
/// document, so the tree has an `<html>` root along with `<head>` and `<body>`,
/// as a browser builds them. Anything else is parsed as a fragment and gains no
/// wrappers of its own.
///
/// The parser covers the tokenizer but only part of the tree builder's error recovery: it
/// closes `p`, `li`, `dt`, `dd`, `option`, and table rows and cells implicitly, but does not
/// reconstruct misnested formatting elements or foster-parent table content.
pub fn from_html(source: &str, options: FromHtmlOptions) -> Result<Node, HtmlError> {
  // Decoding drops a leading byte order mark: https://encoding.spec.whatwg.org/#decode
  let source = source.strip_prefix('\u{feff}').unwrap_or(source);
  let builder = NodeBuilder {
    presets: &options.presets,
    tw_property: options.tailwind_property.as_deref().unwrap_or("tw"),
    max_depth: options.max_depth,
    styles: RefCell::default(),
  };
  let dom = markup::parse(source, is_document(source));
  let mut nodes = Vec::new();

  builder.build_nodes(dom.document(), 0, &mut nodes)?;

  Ok(collapse(nodes))
}

/// Whether the source is a document: it opens with a doctype or an `<html>` tag, ignoring
/// leading whitespace and comments.
///
/// Document parsing invents `<html>`, `<head>` and `<body>` for a source that
/// has none, which would wrap every fragment in boxes its author never wrote.
fn is_document(source: &str) -> bool {
  let mut rest = source.trim_start();

  loop {
    if rest
      .get(..9)
      .is_some_and(|prefix| prefix.eq_ignore_ascii_case("<!doctype"))
    {
      return true;
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
    _ => Node::container(nodes).with_style(Style::from(StyleDeclarationBlock::parse_loosy(
      "display:block;width:100%;height:100%",
    ))),
  }
}

/// Turns parsed DOM handles into nodes under one set of options.
struct NodeBuilder<'a> {
  presets: &'a StylePresets,
  tw_property: &'a str,
  max_depth: usize,
  /// Parsed `style` attributes, by source text.
  styles: RefCell<HashMap<Box<str>, Style>>,
}

impl NodeBuilder<'_> {
  fn build_nodes(
    &self,
    handle: NodeRef<'_>,
    depth: usize,
    out: &mut Vec<Node>,
  ) -> Result<(), HtmlError> {
    match handle.data() {
      NodeData::Comment(_) | NodeData::Doctype { .. } => {}
      NodeData::Document => {
        for child in handle.children() {
          self.build_nodes(child, depth, out)?;
        }
      }
      NodeData::Text(contents) => {
        if !contents.is_empty() {
          out.push(Node::text(contents.clone()));
        }
      }
      NodeData::Element { name, .. } => {
        if let Some(node) = self.build_element(handle, &name.local, depth)? {
          out.push(node);
        }
      }
    }

    Ok(())
  }

  fn build_element(
    &self,
    handle: NodeRef<'_>,
    tag: &str,
    depth: usize,
  ) -> Result<Option<Node>, HtmlError> {
    if depth >= self.max_depth {
      return Err(HtmlError::MaxDepthExceeded(self.max_depth));
    }

    let node = match tag {
      "br" => Node::text("\n"),
      "img" => match handle.attribute("src").filter(|src| !src.trim().is_empty()) {
        Some(src) => Node::image(ImageData {
          src: ImageSourceInput::Url(src.into()),
          width: pixels(handle, tag, "width"),
          height: pixels(handle, tag, "height"),
        }),
        // Chrome draws an `<img>` without a source as an empty inline box its `width` and
        // `height` size.
        None => Node::container([]).with_preset(empty_image_style(handle)),
      },
      _ if UNRENDERED_TAGS.contains(&tag) => return Ok(None),
      "svg" => Node::image(ImageData {
        src: self.inline_svg(handle, depth)?,
        width: pixels(handle, tag, "width"),
        height: pixels(handle, tag, "height"),
      }),
      _ => match text_only_contents(handle) {
        Some(text) => Node::text(text),
        None => {
          let mut children = Vec::new();
          for child in handle.children() {
            self.build_nodes(child, depth + 1, &mut children)?;
          }

          Node::container(children)
        }
      },
    };

    Ok(Some(self.apply_metadata(node, handle, tag)))
  }

  fn apply_metadata(&self, mut node: Node, handle: NodeRef<'_>, tag: &str) -> Node {
    node = node.with_tag_name(tag);

    let mut preset = self.presets.get(tag).cloned();

    // A size in other units than pixels, which `ImageData` cannot hold, styles the image.
    if matches!(node.kind, NodeKind::Image(_)) {
      for (name, declaration) in [
        (
          "width",
          StyleDeclaration::width as fn(Length) -> StyleDeclaration,
        ),
        ("height", StyleDeclaration::height),
      ] {
        if let Some(length) =
          dimension(handle, tag, name).filter(|length| !matches!(length, Length::Px(_)))
        {
          Arc::make_mut(preset.get_or_insert_default()).push(declaration(length), false);
        }
      }
    }

    if let Some(preset) = preset {
      node = node.with_preset(preset);
    }

    let NodeData::Element { attrs, .. } = handle.data() else {
      return node;
    };

    let mut attributes = BTreeMap::new();
    for attr in attrs {
      let name = &*attr.local;
      let value = &*attr.value;

      // Read Tailwind independently of reserved names so it can alias `class`
      // without dropping the class name.
      if name == self.tw_property
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
        "style" => node = node.with_style(self.style(value)),
        // Consumed into `ImageData`; re-emitted by serialization, so keep them out
        // of the passthrough attributes to avoid duplicating on round-trip.
        "src" if tag == "img" => {}
        "width" | "height" if matches!(tag, "img" | "svg") => {}
        // Consumed above; keep out of the passthrough attributes.
        _ if name == self.tw_property => {}
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

  /// The source of an inline `<svg>`: its subtree as nodes, which the page's stylesheet reaches,
  /// or its markup when the nodes do not form a document.
  #[cfg(feature = "svg")]
  fn inline_svg(&self, handle: NodeRef<'_>, depth: usize) -> Result<ImageSourceInput, HtmlError> {
    let markup = handle.outer_html();
    let root = self.svg_node(handle, depth)?;
    let source = str::from_utf8(&markup)
      .ok()
      .zip(root)
      .and_then(|(markup, root)| SvgSource::from_inline_element(&root, markup).ok());

    Ok(match source {
      Some(source) => ImageSourceInput::Loaded(ImageSource::from(source)),
      None => ImageSourceInput::Buffer(markup),
    })
  }

  #[cfg(not(feature = "svg"))]
  fn inline_svg(&self, handle: NodeRef<'_>, _depth: usize) -> Result<ImageSourceInput, HtmlError> {
    Ok(ImageSourceInput::Buffer(handle.outer_html()))
  }

  /// An element of an inline `<svg>` with every attribute in its attribute map under its
  /// qualified name, or a text, none for other nodes. A subtree past the depth limit fails as the
  /// rest of the document does.
  #[cfg(feature = "svg")]
  fn svg_node(&self, handle: NodeRef<'_>, depth: usize) -> Result<Option<Node>, HtmlError> {
    if depth >= self.max_depth {
      return Err(HtmlError::MaxDepthExceeded(self.max_depth));
    }

    Ok(match handle.data() {
      NodeData::Text(contents) => Some(Node::text(contents.clone())),
      NodeData::Element { name, attrs, .. } => {
        let attributes: BTreeMap<Box<str>, Box<str>> = attrs
          .iter()
          .map(|attr| {
            let local = &*attr.local;
            let name = match attr.ns {
              AttributeNamespace::Xlink => format!("xlink:{local}").into(),
              AttributeNamespace::Xml => format!("xml:{local}").into(),
              AttributeNamespace::None | AttributeNamespace::Xmlns => local.into(),
            };

            (name, attr.value.clone())
          })
          .collect();
        let mut children = Vec::new();

        for child in handle.children() {
          children.extend(self.svg_node(child, depth + 1)?);
        }

        Some(
          Node::container(children)
            .with_tag_name(&*name.local)
            .with_attributes(attributes),
        )
      }
      _ => None,
    })
  }

  /// The style a `style` attribute of `css` sets.
  fn style(&self, css: &str) -> Style {
    if let Some(style) = self.styles.borrow().get(css) {
      return style.clone();
    }

    let style = Style::from(StyleDeclarationBlock::parse_loosy(css));

    self.styles.borrow_mut().insert(css.into(), style.clone());
    style
  }
}

/// Concatenated text if every child is a text node and not all of it is white space, else `None`.
/// Comments are ignored, mirroring the JS markup walker.
fn text_only_contents(handle: NodeRef<'_>) -> Option<String> {
  let mut text = String::new();

  for child in handle.children() {
    match child.data() {
      NodeData::Comment(_) => continue,
      NodeData::Text(contents) => text.push_str(contents),
      _ => return None,
    }
  }

  text
    .chars()
    .any(|ch| !ch.is_ascii_whitespace())
    .then_some(text)
}

/// The `width` or `height` attribute of an `<img>` or `<svg>` as Blink maps it to style: an
/// `<img>` takes HTML's rules for parsing dimension values, a number of pixels or a percentage,
/// and an `<svg>` a CSS length, a number counting as pixels.
/// <https://html.spec.whatwg.org/multipage/common-microsyntaxes.html#rules-for-parsing-dimension-values>
fn dimension(handle: NodeRef<'_>, tag: &str, name: &str) -> Option<Length> {
  let value = handle.attribute(name)?.trim();

  if tag == "svg" {
    return Length::from_css_str(value)
      .ok()
      .or_else(|| value.parse().ok().map(Length::Px));
  }

  let digits_end = |start: usize| {
    value[start..]
      .find(|character: char| !character.is_ascii_digit())
      .map_or(value.len(), |end| start + end)
  };
  let integer_end = digits_end(0);

  if integer_end == 0 {
    return None;
  }
  // A fraction needs a digit after its point; anything past the number is ignored.
  let number_end = match value[integer_end..].strip_prefix('.') {
    Some(fraction) if fraction.starts_with(|character: char| character.is_ascii_digit()) => {
      digits_end(integer_end + 1)
    }
    _ => integer_end,
  };
  let number: f32 = value[..number_end].parse().ok()?;

  Some(if value[number_end..].starts_with('%') {
    Length::Percentage(number)
  } else {
    Length::Px(number)
  })
}

/// [`dimension`] when it is a number of pixels, which `ImageData` holds.
fn pixels(handle: NodeRef<'_>, tag: &str, name: &str) -> Option<f32> {
  match dimension(handle, tag, name)? {
    Length::Px(pixels) => Some(pixels),
    _ => None,
  }
}

/// The style of an `<img>` without a source: an inline-block, sized by its `width` and `height`.
fn empty_image_style(handle: NodeRef<'_>) -> Style {
  let mut style = Style::default().display(Display::InlineBlock);

  if let Some(width) = dimension(handle, "img", "width") {
    style = style.width(width);
  }

  if let Some(height) = dimension(handle, "img", "height") {
    style = style.height(height);
  }

  style
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse(source: &str) -> Node {
    from_html(source, FromHtmlOptions::default()).unwrap()
  }

  /// An `<img>` takes a percentage `width` the way HTML parses a dimension, and an `<svg>` any
  /// CSS length, as Chrome maps them to style; a number of pixels stays in `ImageData`.
  #[test]
  fn image_size_attributes_in_other_units_style_the_image() {
    let image = parse(r#"<img src="a.png" width="50%" height="20..5">"#);
    let fraction_only = parse(r#"<img src="a.png" width=".5">"#);
    let svg = parse(r#"<svg width="100%" height="2em" viewBox="0 0 1 1"></svg>"#);

    assert!(matches!(
      image.kind,
      NodeKind::Image(ImageData {
        width: None,
        height: Some(20.0),
        ..
      })
    ));
    assert!(matches!(
      fraction_only.kind,
      NodeKind::Image(ImageData { width: None, .. })
    ));
    assert!(image.to_html().contains("width: 50%"));
    assert!(svg.to_html().contains("width: 100%; height: 2em"));
  }

  /// A subtree of an inline `<svg>` past the depth limit fails as the rest of the document does,
  /// rather than drawing without its deepest elements.
  #[cfg(feature = "svg")]
  #[test]
  fn an_inline_svg_past_the_depth_limit_fails() {
    let options = FromHtmlOptions::builder().max_depth(2).build();
    let result = from_html(r#"<svg><g><rect width="1" height="1"/></g></svg>"#, options);

    assert!(matches!(result, Err(HtmlError::MaxDepthExceeded(2))));
  }

  /// A `style` attribute is a declaration list, and CSS drops only the declaration it
  /// cannot read. An unreadable width used to take the whole attribute down with it.
  #[test]
  fn an_unreadable_declaration_leaves_its_neighbours_alone() {
    let block = StyleDeclarationBlock::parse_loosy("font-size:64px;width:wider;color:red");

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

  /// A doctype makes a document even without an `<html>` tag, so the body and its margin are
  /// there as in a browser.
  #[test]
  fn a_doctype_makes_a_document() {
    let node = parse("<!DOCTYPE html><title>t</title><div>hi</div>");

    assert_eq!(node.tag_name(), Some("html"));
    assert!(node.to_html().contains("<body"));
  }

  #[test]
  fn a_leading_byte_order_mark_is_dropped() {
    assert_eq!(
      parse("\u{feff}<!doctype html><div>hi</div>").tag_name(),
      Some("html")
    );
    assert_eq!(parse("\u{feff}<div>hi</div>").tag_name(), Some("div"));
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
  fn an_img_without_a_source_is_an_empty_box() {
    let node = parse(r#"<img src="" width="10" height="20">"#);

    assert!(matches!(node.kind, NodeKind::Container { .. }));
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
  fn a_repeated_inline_style_parses_like_the_first() {
    let node =
      parse(r#"<p style="color:red">a</p><p style="color:red">b</p><p style="color:blue">c</p>"#);
    let NodeKind::Container { children } = &node.kind else {
      panic!("expected container");
    };
    let alone = |source: &str| format!("{:?}", parse(source));

    assert_eq!(
      format!("{:?}", children[1]),
      alone(r#"<p style="color:red">b</p>"#)
    );
    assert_eq!(
      format!("{:?}", children[2]),
      alone(r#"<p style="color:blue">c</p>"#)
    );
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
