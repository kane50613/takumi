//! A parser for authored markup, which fills a [`Dom`].
//!
//! Approximate: it follows the HTML tokenizer for tags, attributes, comments, character references,
//! and raw text, but only a small part of the tree builder's error recovery
//! (<https://html.spec.whatwg.org/multipage/parsing.html#tree-construction>). Implied end tags
//! cover `p`, `li`, `dt`, `dd`, `option`, and table rows and cells, checked against the innermost
//! open element only. Misnested formatting elements are not reconstructed, table content is not
//! foster-parented, and HTML elements inside `<svg>` do not break out of it.

use std::borrow::Cow;

use super::{
  dom::{
    Attribute, AttributeNamespace, Dom, ElementName, Namespace, NodeData, NodeId, VOID_ELEMENTS,
  },
  entities,
};

/// Parses `source`, wrapping its content in `<html>`, `<head>`, and `<body>` as a browser does
/// when `document` is set.
pub(crate) fn parse(source: &str, document: bool) -> Dom {
  let source = normalize_newlines(source);
  let mut parser = Parser {
    rest: &source,
    dom: Dom::default(),
    open: Vec::new(),
    templates: Vec::new(),
    document,
    quirks: document,
    html: None,
    head: None,
    body: None,
  };

  while !parser.rest.is_empty() {
    parser.step();
  }

  if document {
    parser.body(Vec::new());
  }

  parser.dom
}

/// <https://infra.spec.whatwg.org/#normalize-newlines>
fn normalize_newlines(source: &str) -> Cow<'_, str> {
  if source.contains('\r') {
    Cow::Owned(source.replace("\r\n", "\n").replace('\r', "\n"))
  } else {
    Cow::Borrowed(source)
  }
}

/// A start or end tag as written, its names lowercased.
struct Tag {
  name: String,
  attrs: Vec<(String, String)>,
  self_closing: bool,
}

impl Tag {
  fn empty(name: &str) -> Self {
    Self {
      name: name.into(),
      attrs: Vec::new(),
      self_closing: false,
    }
  }
}

struct Parser<'a> {
  rest: &'a str,
  dom: Dom,
  /// Open elements, innermost last. `<html>`, `<head>`, and `<body>` are here only when written.
  open: Vec<NodeId>,
  /// Each `<template>` and the detached fragment its contents go into, which leaves the element
  /// itself empty as the tree builder does.
  templates: Vec<(NodeId, NodeId)>,
  document: bool,
  /// Whether the tree builder is in quirks mode, as a document without an `html` doctype puts
  /// it. Approximate: legacy public identifiers that also select quirks mode are not recognized.
  /// <https://html.spec.whatwg.org/multipage/parsing.html#the-initial-insertion-mode>
  quirks: bool,
  html: Option<NodeId>,
  head: Option<NodeId>,
  body: Option<NodeId>,
}

impl<'a> Parser<'a> {
  fn step(&mut self) {
    let rest = self.rest;
    let markup = rest.as_bytes().windows(2).position(|pair| match *pair {
      [b'<', next] => next.is_ascii_alphabetic() || matches!(next, b'/' | b'!' | b'?'),
      _ => false,
    });

    match markup {
      Some(0) => self.markup(),
      end => {
        let (text, rest) = split_or_whole(rest, end.unwrap_or(rest.len()));

        self.rest = rest;
        self.insert_text(&entities::decode(text));
      }
    }
  }

  fn markup(&mut self) {
    let rest = self.rest;

    if let Some(rest) = rest.strip_prefix("<!--") {
      // `<!-->` and `<!--->` end an empty comment.
      let (contents, rest) = match rest.strip_prefix('>').or_else(|| rest.strip_prefix("->")) {
        Some(rest) => ("", rest),
        None => rest.split_once("-->").unwrap_or((rest, "")),
      };

      self.rest = rest;
      self.insert_comment(contents);
    } else if let Some(rest) = rest.strip_prefix("<![CDATA[")
      && self.child_namespace() != Namespace::Html
    {
      let (text, rest) = rest.split_once("]]>").unwrap_or((rest, ""));

      self.rest = rest;
      self.insert_text(text);
    } else if let Some(rest) = rest.strip_prefix("<!").or_else(|| rest.strip_prefix("<?")) {
      let (declaration, rest) = rest.split_once('>').unwrap_or((rest, ""));

      self.rest = rest;
      self.declaration(declaration);
    } else if let Some(rest) = rest.strip_prefix("</") {
      if rest.starts_with(|character: char| character.is_ascii_alphabetic()) {
        self.rest = rest;

        let tag = self.tag();

        self.end_tag(&tag.name);
      } else {
        // `</` without a letter after it opens a bogus comment, or nothing when it is `</>`.
        let (comment, rest) = rest.split_once('>').unwrap_or((rest, ""));

        self.rest = rest;

        if !comment.is_empty() {
          self.insert_comment(comment);
        }
      }
    } else {
      self.rest = rest.strip_prefix('<').unwrap_or(rest);

      let tag = self.tag();

      self.start_tag(tag);
    }
  }

  /// A doctype, or a bogus comment holding `declaration`.
  fn declaration(&mut self, declaration: &str) {
    let Some(doctype) = declaration
      .split_at_checked(7)
      .filter(|(prefix, _)| prefix.eq_ignore_ascii_case("doctype"))
      .map(|(_, doctype)| doctype)
    else {
      self.insert_comment(declaration);
      return;
    };
    let name = doctype
      .split_ascii_whitespace()
      .next()
      .unwrap_or_default()
      .to_ascii_lowercase();
    if self.html.is_none() {
      self.quirks = name != "html";
    }

    let doctype = self.dom.push(NodeData::Doctype { name: name.into() });

    self.dom.append_node(Dom::DOCUMENT, doctype);
  }

  /// Reads a tag from its name to its closing `>`.
  /// <https://html.spec.whatwg.org/multipage/parsing.html#tag-name-state>
  fn tag(&mut self) -> Tag {
    let (name, mut rest) = split_before(self.rest, is_tag_delimiter);
    let name = name.to_ascii_lowercase();
    let mut attrs: Vec<(String, String)> = Vec::new();
    let mut self_closing = false;

    loop {
      rest = rest.trim_start_matches(|character: char| character.is_ascii_whitespace());

      if let Some(after) = rest.strip_prefix("/>") {
        rest = after;
        self_closing = true;
        break;
      }

      if let Some(after) = rest.strip_prefix('>') {
        rest = after;
        break;
      }

      if let Some(after) = rest.strip_prefix('/') {
        rest = after;
        continue;
      }

      if rest.is_empty() {
        break;
      }

      // An attribute name may start with `=`.
      let name_end = rest
        .char_indices()
        .skip(1)
        .find(|&(_, character)| is_tag_delimiter(character) || character == '=')
        .map_or(rest.len(), |(end, _)| end);
      let (name, after) = split_or_whole(rest, name_end);
      let name = name.to_ascii_lowercase();
      let after_name = after.trim_start_matches(|character: char| character.is_ascii_whitespace());
      let value = match after_name.strip_prefix('=') {
        Some(value) => {
          let (value, after) = split_attribute_value(value);

          rest = after;
          value
        }
        None => {
          rest = after;
          ""
        }
      };

      if !attrs.iter().any(|(existing, _)| *existing == name) {
        attrs.push((name, entities::decode_attribute(value).into_owned()));
      }
    }

    self.rest = rest;

    Tag {
      name,
      attrs,
      self_closing,
    }
  }

  fn start_tag(&mut self, tag: Tag) {
    let ns = match (self.child_namespace(), tag.name.as_str()) {
      (Namespace::Html, "svg") => Namespace::Svg,
      (Namespace::Html, "math") => Namespace::MathMl,
      (ns, _) => ns,
    };

    if ns == Namespace::Html {
      if matches!(tag.name.as_str(), "html" | "head" | "body") {
        if self.document {
          self.document_element(tag);
        }

        return;
      }

      self.close_implied_by(&tag.name);
      self.open_implied_table_parents(&tag.name);
    }

    let raw_text = ns == Namespace::Html && RAW_TEXT_ELEMENTS.contains(&tag.name.as_str());
    let escapable_raw_text =
      ns == Namespace::Html && matches!(tag.name.as_str(), "textarea" | "title");
    let opens = match ns {
      Namespace::Html => {
        !VOID_ELEMENTS.contains(&tag.name.as_str()) && !raw_text && !escapable_raw_text
      }
      _ => !tag.self_closing,
    };
    let leading_newline =
      ns == Namespace::Html && matches!(tag.name.as_str(), "pre" | "listing" | "textarea");
    let is_template = tag.name == "template";
    let parent = self.parent(HEAD_ELEMENTS.contains(&tag.name.as_str()));
    let element = self.element(ns, tag);

    self.dom.append_node(parent, element);

    if leading_newline {
      self.rest = self.rest.strip_prefix('\n').unwrap_or(self.rest);
    }

    if raw_text || escapable_raw_text {
      let text = self.raw_text(element);
      let text = if escapable_raw_text {
        entities::decode(text)
      } else {
        Cow::Borrowed(text)
      };

      if !text.is_empty() {
        self.dom.append_text(element, &text);
      }
    } else if opens {
      if ns == Namespace::Html && is_template {
        let contents = self.dom.push(NodeData::Document);

        self.templates.push((element, contents));
      }

      self.open.push(element);
    }
  }

  /// Opens the `<tbody>` and `<tr>` a row or cell implies directly inside `<table>`.
  /// <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intable>
  fn open_implied_table_parents(&mut self, name: &str) {
    let implied = match (self.innermost_html_element(), name) {
      (Some("table"), "tr" | "td" | "th") => "tbody",
      (Some("tbody" | "thead" | "tfoot"), "td" | "th") => "tr",
      _ => return,
    };

    self.start_tag(Tag::empty(implied));
    self.open_implied_table_parents(name);
  }

  fn innermost_html_element(&self) -> Option<&str> {
    self
      .open
      .last()
      .and_then(|&top| self.dom.element_name(top))
      .filter(|name| name.ns == Namespace::Html)
      .map(|name| &*name.local)
  }

  fn end_tag(&mut self, name: &str) {
    // An end `</br>` is a `<br>`.
    if name == "br" {
      self.start_tag(Tag::empty(name));
      return;
    }

    let dom = &self.dom;

    if let Some(index) = self.open.iter().rposition(|&id| {
      dom
        .element_name(id)
        .is_some_and(|open| open.local.eq_ignore_ascii_case(name))
    }) {
      self.open.truncate(index);
    }
  }

  /// The text of a raw text element up to its end tag, which it consumes.
  fn raw_text(&mut self, element: NodeId) -> &'a str {
    let rest = self.rest;
    let Some(name) = self.dom.element_name(element) else {
      return "";
    };
    let close = format!("</{}", name.local);
    let end = rest
      .char_indices()
      .filter(|&(_, character)| character == '<')
      .map(|(index, _)| index)
      .find(|&index| {
        rest
          .get(index..index + close.len())
          .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&close))
          && rest
            .get(index + close.len()..)
            .is_some_and(|after| after.chars().next().is_none_or(is_tag_delimiter))
      });

    let Some(end) = end else {
      self.rest = "";
      return rest;
    };
    let (text, closing) = split_or_whole(rest, end);

    self.rest = closing.split_once('>').map_or("", |(_, after)| after);
    text
  }

  /// `<html>`, `<head>`, or `<body>` in a document: the first adds the element, a later one
  /// adds the attributes it lacks.
  fn document_element(&mut self, tag: Tag) {
    match tag.name.as_str() {
      "html" => {
        self.html(tag.attrs);
      }
      "head" => {
        if self.head.is_none() && self.body.is_none() {
          let head = self.head(tag.attrs);

          self.open.push(head);
        }
      }
      _ => {
        let written = self.body.is_none();
        let body = self.body(tag.attrs);

        if written {
          self.open.push(body);
        }
      }
    }
  }

  fn add_missing_attributes(&mut self, element: NodeId, attrs: Vec<(String, String)>) {
    if let NodeData::Element {
      attrs: existing, ..
    } = self.dom.data_mut(element)
    {
      for (name, value) in attrs {
        if !existing.iter().any(|attr| *attr.local == *name) {
          existing.push(Attribute {
            ns: AttributeNamespace::None,
            local: name.into(),
            value: value.into(),
          });
        }
      }
    }
  }

  fn html(&mut self, attrs: Vec<(String, String)>) -> NodeId {
    if let Some(html) = self.html {
      self.add_missing_attributes(html, attrs);
      return html;
    }

    let html = self.html_element("html", attrs);

    self.dom.append_node(Dom::DOCUMENT, html);
    self.html = Some(html);
    html
  }

  fn head(&mut self, attrs: Vec<(String, String)>) -> NodeId {
    if let Some(head) = self.head {
      return head;
    }

    let html = self.html(Vec::new());
    let head = self.html_element("head", attrs);

    self.dom.append_node(html, head);
    self.head = Some(head);
    head
  }

  fn body(&mut self, attrs: Vec<(String, String)>) -> NodeId {
    if let Some(body) = self.body {
      self.add_missing_attributes(body, attrs);
      return body;
    }

    self.close_head();
    self.head(Vec::new());

    let html = self.html(Vec::new());
    let body = self.html_element("body", attrs);

    self.dom.append_node(html, body);
    self.body = Some(body);
    body
  }

  fn html_element(&mut self, name: &str, attrs: Vec<(String, String)>) -> NodeId {
    self.element(
      Namespace::Html,
      Tag {
        attrs,
        ..Tag::empty(name)
      },
    )
  }

  /// Pops a written `<head>` still open, as content that belongs in `<body>` does.
  fn close_head(&mut self) {
    if let Some(index) = self.open.iter().position(|&id| Some(id) == self.head) {
      self.open.truncate(index);
    }
  }

  /// Where the next node goes. In a document, content outside every written element goes into
  /// `<head>` while it is metadata before any body content, and into `<body>` otherwise.
  fn parent(&mut self, head_content: bool) -> NodeId {
    if let Some(&top) = self.open.last()
      && let Some(&(_, contents)) = self.templates.iter().find(|(template, _)| *template == top)
    {
      return contents;
    }

    match self.open.last().copied() {
      Some(top) if !self.document || (Some(top) != self.html && Some(top) != self.head) => top,
      Some(top) if Some(top) == self.head && head_content => top,
      _ if !self.document => Dom::DOCUMENT,
      _ if head_content && self.body.is_none() => self.head(Vec::new()),
      _ => self.body(Vec::new()),
    }
  }

  fn insert_text(&mut self, text: &str) {
    if text.is_empty() {
      return;
    }

    // White space before body content is dropped, as the tree builder ignores it there.
    let outside_body = self.document
      && self.body.is_none()
      && self
        .open
        .last()
        .is_none_or(|&top| Some(top) == self.html || Some(top) == self.head);

    if outside_body
      && text
        .chars()
        .all(|character| character.is_ascii_whitespace())
    {
      return;
    }

    let parent = self.parent(false);

    self.dom.append_text(parent, text);
  }

  fn insert_comment(&mut self, contents: &str) {
    let parent = self
      .open
      .last()
      .copied()
      .or(self.body)
      .unwrap_or(Dom::DOCUMENT);
    let comment = self.dom.push(NodeData::Comment(contents.into()));

    self.dom.append_node(parent, comment);
  }

  /// Pops open elements that a start tag `name` ends implicitly.
  /// <https://html.spec.whatwg.org/multipage/syntax.html#optional-tags>
  fn close_implied_by(&mut self, name: &str) {
    while let Some(&top) = self.open.last()
      && let Some(open) = self
        .dom
        .element_name(top)
        .filter(|open| open.ns == Namespace::Html)
    {
      let closes = match &*open.local {
        "p" => ENDS_P.contains(&name) || (name == "table" && !self.quirks),
        "li" => name == "li",
        "dt" | "dd" => matches!(name, "dt" | "dd"),
        "option" => matches!(name, "option" | "optgroup"),
        "td" | "th" => matches!(name, "td" | "th" | "tr"),
        "tr" => name == "tr",
        _ => false,
      };

      if !closes {
        break;
      }

      self.open.pop();
    }
  }

  /// The namespace of an element opened at the current position, before the `svg` and `math`
  /// tags switch it.
  fn child_namespace(&self) -> Namespace {
    let Some(parent) = self.open.last().and_then(|&top| self.dom.element_name(top)) else {
      return Namespace::Html;
    };
    let integration_point = match parent.ns {
      Namespace::Html => true,
      Namespace::Svg => matches!(&*parent.local, "foreignObject" | "desc" | "title"),
      Namespace::MathMl => matches!(&*parent.local, "mi" | "mo" | "mn" | "ms" | "mtext"),
    };

    if integration_point {
      Namespace::Html
    } else {
      parent.ns
    }
  }

  /// A new element in `ns`, its names cased as the tree builder adjusts them in foreign content.
  /// <https://html.spec.whatwg.org/multipage/parsing.html#creating-and-inserting-nodes>
  fn element(&mut self, ns: Namespace, tag: Tag) -> NodeId {
    let local = match ns {
      Namespace::Svg => adjust_case(&SVG_ELEMENTS, tag.name),
      _ => tag.name,
    };
    let attrs = tag
      .attrs
      .into_iter()
      .map(|(name, value)| foreign_attribute(ns, name, value))
      .collect();

    self.dom.push(NodeData::Element {
      name: ElementName {
        ns,
        local: local.into(),
      },
      attrs,
    })
  }
}

fn is_tag_delimiter(character: char) -> bool {
  character.is_ascii_whitespace() || matches!(character, '/' | '>')
}

/// A quoted or unquoted attribute value at the start of `source`, and the source after it.
/// <https://html.spec.whatwg.org/multipage/parsing.html#before-attribute-value-state>
fn split_attribute_value(source: &str) -> (&str, &str) {
  let source = source.trim_start_matches(|character: char| character.is_ascii_whitespace());

  match source.chars().next() {
    Some(quote @ ('"' | '\'')) => {
      let quoted = source.strip_prefix(quote).unwrap_or(source);

      quoted.split_once(quote).unwrap_or((quoted, ""))
    }
    _ => split_before(source, |character| {
      character.is_ascii_whitespace() || character == '>'
    }),
  }
}

/// `text` split before the first character `delimiter` matches, or whole when none does.
fn split_before(text: &str, delimiter: impl Fn(char) -> bool) -> (&str, &str) {
  split_or_whole(text, text.find(delimiter).unwrap_or(text.len()))
}

/// `text` split at byte `index`, or whole when `index` is past its end or inside a character.
fn split_or_whole(text: &str, index: usize) -> (&str, &str) {
  text.split_at_checked(index).unwrap_or((text, ""))
}

/// An attribute of an element in `ns`, with the namespace and casing the tree builder gives it.
/// <https://html.spec.whatwg.org/multipage/parsing.html#adjust-foreign-attributes>
fn foreign_attribute(ns: Namespace, name: String, value: String) -> Attribute {
  let (attribute_ns, local) = if ns == Namespace::Html {
    (AttributeNamespace::None, name)
  } else if let Some(local) = name
    .strip_prefix("xlink:")
    .filter(|local| XLINK_ATTRIBUTES.contains(local))
  {
    (AttributeNamespace::Xlink, local.into())
  } else if let Some(local) = name
    .strip_prefix("xml:")
    .filter(|local| matches!(*local, "lang" | "space"))
  {
    (AttributeNamespace::Xml, local.into())
  } else if name == "xmlns:xlink" {
    (AttributeNamespace::Xmlns, "xlink".into())
  } else if name == "xmlns" {
    (AttributeNamespace::Xmlns, name)
  } else if ns == Namespace::Svg {
    (AttributeNamespace::None, adjust_case(&SVG_ATTRIBUTES, name))
  } else if name == "definitionurl" {
    (AttributeNamespace::None, "definitionURL".into())
  } else {
    (AttributeNamespace::None, name)
  };

  Attribute {
    ns: attribute_ns,
    local: local.into(),
    value: value.into(),
  }
}

/// `name` in the casing `table` lists it in, or as is.
fn adjust_case(table: &[&str], name: String) -> String {
  table
    .iter()
    .find(|cased| cased.eq_ignore_ascii_case(&name))
    .map_or(name, |cased| (*cased).into())
}

/// Elements whose text runs to their end tag without markup or character references.
const RAW_TEXT_ELEMENTS: [&str; 7] = [
  "script", "style", "xmp", "iframe", "noembed", "noframes", "noscript",
];

/// Metadata the tree builder keeps in `<head>` until body content starts.
const HEAD_ELEMENTS: [&str; 7] = [
  "base", "link", "meta", "noscript", "script", "style", "title",
];

/// Start tags that end an open `<p>`, except `table`, which ends it only outside quirks mode.
/// <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody>
const ENDS_P: [&str; 40] = [
  "address",
  "article",
  "aside",
  "blockquote",
  "center",
  "dd",
  "details",
  "dialog",
  "dir",
  "div",
  "dl",
  "dt",
  "fieldset",
  "figcaption",
  "figure",
  "footer",
  "form",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "header",
  "hgroup",
  "hr",
  "li",
  "listing",
  "main",
  "menu",
  "nav",
  "ol",
  "p",
  "plaintext",
  "pre",
  "search",
  "section",
  "summary",
  "ul",
  "xmp",
];

const XLINK_ATTRIBUTES: [&str; 7] = [
  "actuate", "arcrole", "href", "role", "show", "title", "type",
];

/// <https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inforeign>
const SVG_ELEMENTS: [&str; 37] = [
  "altGlyph",
  "altGlyphDef",
  "altGlyphItem",
  "animateColor",
  "animateMotion",
  "animateTransform",
  "clipPath",
  "feBlend",
  "feColorMatrix",
  "feComponentTransfer",
  "feComposite",
  "feConvolveMatrix",
  "feDiffuseLighting",
  "feDisplacementMap",
  "feDistantLight",
  "feDropShadow",
  "feFlood",
  "feFuncA",
  "feFuncB",
  "feFuncG",
  "feFuncR",
  "feGaussianBlur",
  "feImage",
  "feMerge",
  "feMergeNode",
  "feMorphology",
  "feOffset",
  "fePointLight",
  "feSpecularLighting",
  "feSpotLight",
  "feTile",
  "feTurbulence",
  "foreignObject",
  "glyphRef",
  "linearGradient",
  "radialGradient",
  "textPath",
];

/// <https://html.spec.whatwg.org/multipage/parsing.html#adjust-svg-attributes>
const SVG_ATTRIBUTES: [&str; 58] = [
  "attributeName",
  "attributeType",
  "baseFrequency",
  "baseProfile",
  "calcMode",
  "clipPathUnits",
  "diffuseConstant",
  "edgeMode",
  "filterUnits",
  "glyphRef",
  "gradientTransform",
  "gradientUnits",
  "kernelMatrix",
  "kernelUnitLength",
  "keyPoints",
  "keySplines",
  "keyTimes",
  "lengthAdjust",
  "limitingConeAngle",
  "markerHeight",
  "markerUnits",
  "markerWidth",
  "maskContentUnits",
  "maskUnits",
  "numOctaves",
  "pathLength",
  "patternContentUnits",
  "patternTransform",
  "patternUnits",
  "pointsAtX",
  "pointsAtY",
  "pointsAtZ",
  "preserveAlpha",
  "preserveAspectRatio",
  "primitiveUnits",
  "refX",
  "refY",
  "repeatCount",
  "repeatDur",
  "requiredExtensions",
  "requiredFeatures",
  "specularConstant",
  "specularExponent",
  "spreadMethod",
  "startOffset",
  "stdDeviation",
  "stitchTiles",
  "surfaceScale",
  "systemLanguage",
  "tableValues",
  "targetX",
  "targetY",
  "textLength",
  "viewBox",
  "viewTarget",
  "xChannelSelector",
  "yChannelSelector",
  "zoomAndPan",
];

#[cfg(test)]
mod tests {
  use super::parse;

  fn fragment(source: &str) -> String {
    String::from_utf8(parse(source, false).document().outer_html()).unwrap()
  }

  fn document(source: &str) -> String {
    String::from_utf8(parse(source, true).document().outer_html()).unwrap()
  }

  #[test]
  fn decodes_references_as_the_tokenizer_does() {
    assert_eq!(
      fragment("<p>&lt;a&gt; &amp; &#169;&#xA9; &unknown; &amp</p>"),
      "<p>&lt;a&gt; &amp; ©© &amp;unknown; &amp;</p>"
    );
    assert_eq!(
      fragment("<p>a&Tab;b&NewLine;c&nbspd&notit; &#65&#x42 &#0; &#x110000; &#128;</p>"),
      "<p>a\tb\nc&nbsp;d¬it; AB \u{fffd} \u{fffd} €</p>"
    );
    assert_eq!(
      fragment(r#"<a title="&amp=&ampx&amp;x&amp">"#),
      r#"<a title="&amp;amp=&amp;ampx&amp;x&amp;"></a>"#
    );
  }

  #[test]
  fn a_table_ends_a_paragraph_outside_quirks_mode() {
    assert_eq!(fragment("<p>a<table></table>"), "<p>a</p><table></table>");
    assert_eq!(
      document("<!DOCTYPE html><p>a<table></table>"),
      "<!DOCTYPE html><html><head></head><body><p>a</p><table></table></body></html>"
    );
    assert_eq!(
      document("<html><p>a<table></table>"),
      "<html><head></head><body><p>a<table></table></p></body></html>"
    );
  }

  #[test]
  fn a_document_without_content_has_a_body() {
    assert_eq!(
      document("<!DOCTYPE html><title>t</title>"),
      "<!DOCTYPE html><html><head><title>t</title></head><body></body></html>"
    );
  }

  #[test]
  fn reads_every_attribute_syntax() {
    assert_eq!(
      fragment(r#"<div a="1 &quot;2&quot;" b='3' c=4 d data-é=5 a="dup"></div>"#),
      r#"<div a="1 &quot;2&quot;" b="3" c="4" d="" data-é="5"></div>"#
    );
  }

  #[test]
  fn restores_svg_casing_and_xlink() {
    assert_eq!(
      fragment(r##"<svg viewbox="0 0 1 1"><lineargradient id="g"/><use xlink:href="#g"/></svg>"##),
      r##"<svg viewBox="0 0 1 1"><linearGradient id="g"></linearGradient><use xlink:href="#g"></use></svg>"##
    );
  }

  #[test]
  fn foreign_object_returns_to_html() {
    assert_eq!(
      fragment("<svg><foreignObject><div/>x</foreignObject><g/></svg>"),
      "<svg><foreignObject><div>x</div></foreignObject><g></g></svg>"
    );
  }

  #[test]
  fn raw_text_runs_to_its_end_tag() {
    assert_eq!(
      fragment("<script>if (a < b) '</div>'</SCRIPT ><textarea>\n&lt;b&gt;</textarea>"),
      "<script>if (a < b) '</div>'</script><textarea>&lt;b&gt;</textarea>"
    );
  }

  #[test]
  fn start_tags_end_paragraphs_and_list_items() {
    assert_eq!(
      fragment("<p>a<div>b</div><ul><li>c<li>d</ul>"),
      "<p>a</p><div>b</div><ul><li>c</li><li>d</li></ul>"
    );
    assert_eq!(
      fragment("<ul><li><p>a<li>b</ul><dl><dt><p>c<dd>d</dl><details><p>e<summary>f</details>"),
      "<ul><li><p>a</p></li><li>b</li></ul><dl><dt><p>c</p></dt><dd>d</dd></dl><details><p>e</p><summary>f</summary></details>"
    );
  }

  #[test]
  fn rows_imply_a_table_body() {
    assert_eq!(
      fragment("<table><tr><td>a<td>b<tr><td>c</table>"),
      "<table><tbody><tr><td>a</td><td>b</td></tr><tr><td>c</td></tr></tbody></table>"
    );
  }

  #[test]
  fn template_contents_stay_out_of_the_tree() {
    assert_eq!(
      fragment("<template><img src=a></template><p>b</p>"),
      "<template></template><p>b</p>"
    );
  }

  #[test]
  fn a_document_gains_head_and_body() {
    assert_eq!(
      document("<!doctype html>\n<title>t</title>\n<div>a</div>"),
      "<!DOCTYPE html><html><head><title>t</title></head><body><div>a</div></body></html>"
    );
    assert_eq!(
      document("<html lang=en><head><meta charset=utf-8><div>a</div>"),
      r#"<html lang="en"><head><meta charset="utf-8"></head><body><div>a</div></body></html>"#
    );
  }

  #[test]
  fn reads_comments_cdata_and_newlines() {
    assert_eq!(
      fragment("<!---->a<!-->b<svg><![CDATA[<c>]]></svg>\r\nd</br>"),
      "<!---->a<!---->b<svg>&lt;c&gt;</svg>\nd<br>"
    );
  }

  #[test]
  fn closes_what_is_still_open_at_the_end() {
    assert_eq!(fragment("<div><span>a"), "<div><span>a</span></div>");
    assert_eq!(fragment("a < b <3"), "a &lt; b &lt;3");
  }
}
