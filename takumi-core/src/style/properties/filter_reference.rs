//! SVG `<filter>` references in `filter: url(data:image/svg+xml,...)`.

use std::{str::from_utf8, sync::Arc};

use crate::{
  html::{
    dom::{AttributeNamespace, NodeData, NodeRef},
    markup::parse,
  },
  resources::image::{DataUriError, decode_data_uri},
};

/// Error produced while parsing a filter reference.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum FilterReferenceError {
  /// The URL is not a `data:image/svg+xml` URI.
  #[error("filter url() must be a data:image/svg+xml URI")]
  UnsupportedUrl,
  /// The data URI could not be decoded.
  #[error("invalid data URI: {0}")]
  InvalidDataUri(&'static str),
  /// The document contains no `<filter>` element.
  #[error("no <filter> element found")]
  MissingFilterElement,
}

/// A validated SVG filter reference.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterReference {
  /// The `<filter>` markup, its `id` replaced with [`FilterReference::ID`].
  pub markup: Arc<str>,
}

impl FilterReference {
  /// The `id` attribute value guaranteed on [`FilterReference::markup`].
  pub const ID: &str = "takumi-filter";
  /// Parses a `filter: url(...)` value, `data:image/svg+xml` URIs only.
  pub(crate) fn from_url(url: &str) -> Result<Self, FilterReferenceError> {
    let decoded = decode_data_uri(url).map_err(|error| match error {
      DataUriError::Malformed => FilterReferenceError::UnsupportedUrl,
      DataUriError::Undecodable => FilterReferenceError::InvalidDataUri("undecodable body"),
    })?;

    if decoded.mime != "image/svg+xml" {
      return Err(FilterReferenceError::UnsupportedUrl);
    }

    let text = from_utf8(&decoded.bytes)
      .map_err(|_| FilterReferenceError::InvalidDataUri("body is not UTF-8"))?;

    Self::from_markup(text)
  }

  /// The first `<filter>` element in `markup`, a filter or an `<svg>` document holding one, as
  /// [`filter_markup`] writes it.
  pub(crate) fn from_markup(markup: &str) -> Result<Self, FilterReferenceError> {
    // Inside `<svg>`, the parser keeps a lone `<filter>` and its primitives in the SVG namespace.
    let dom = parse(&format!("<svg>{markup}</svg>"), false);
    let filter = first_filter(dom.document()).ok_or(FilterReferenceError::MissingFilterElement)?;

    Ok(Self {
      markup: filter_markup(filter).into(),
    })
  }
}

/// The first `filter` element at or under `root`, in document order.
fn first_filter(root: NodeRef<'_>) -> Option<NodeRef<'_>> {
  let mut pending = vec![root];

  while let Some(node) = pending.pop() {
    if node.svg_element_name() == Some("filter") {
      return Some(node);
    }

    let start = pending.len();

    pending.extend(node.children());
    pending[start..].reverse();
  }

  None
}

/// `filter` as XML under the id [`FilterReference::ID`], with only the filter primitives under it,
/// the elements whose names start with `fe`. The SVG backend writes this markup into its output
/// as is, so names keep to XML name characters and text and values are escaped: nothing in it can
/// close the filter or add an element such as `<script>`.
fn filter_markup(filter: NodeRef<'_>) -> String {
  enum Step<'a> {
    Open(NodeRef<'a>),
    Close(&'a str),
  }

  let mut out = format!(r#"<filter id="{}""#, FilterReference::ID);

  write_attributes(&mut out, filter, true);
  out.push('>');

  let mut steps = vec![Step::Close("filter")];

  steps.extend(filter.children().rev().map(Step::Open));

  while let Some(step) = steps.pop() {
    match step {
      Step::Open(node) => match node.data() {
        NodeData::Text(text) => escape_into(&mut out, text),
        NodeData::Element { .. } => {
          let Some(name) = node
            .svg_element_name()
            .filter(|name| name.starts_with("fe") && is_xml_name(name))
          else {
            continue;
          };

          out.push('<');
          out.push_str(name);
          write_attributes(&mut out, node, false);
          out.push('>');
          steps.push(Step::Close(name));
          steps.extend(node.children().rev().map(Step::Open));
        }
        _ => {}
      },
      Step::Close(name) => {
        out.push_str("</");
        out.push_str(name);
        out.push('>');
      }
    }
  }

  out
}

/// Writes the attributes of `element` with an XML name, its `id` left out when `skip_id` is set.
fn write_attributes(out: &mut String, element: NodeRef<'_>, skip_id: bool) {
  let NodeData::Element { attrs, .. } = element.data() else {
    return;
  };

  for attribute in attrs {
    let prefix = match attribute.ns {
      AttributeNamespace::None if skip_id && &*attribute.local == "id" => continue,
      AttributeNamespace::None => "",
      AttributeNamespace::Xlink => "xlink:",
      AttributeNamespace::Xml => "xml:",
      AttributeNamespace::Xmlns => continue,
    };

    if !is_xml_name(&attribute.local) {
      continue;
    }

    out.push(' ');
    out.push_str(prefix);
    out.push_str(&attribute.local);
    out.push_str("=\"");
    escape_into(out, &attribute.value);
    out.push('"');
  }
}

/// Whether `name` is a name XML reads back as one: ASCII letters, digits, `-`, `_`, and `.`,
/// starting with a letter or `_`.
fn is_xml_name(name: &str) -> bool {
  name.starts_with(|character: char| character.is_ascii_alphabetic() || character == '_')
    && name
      .chars()
      .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
}

/// Appends `text` to `out` with the characters XML gives meaning escaped.
fn escape_into(out: &mut String, text: &str) {
  for character in text.chars() {
    match character {
      '&' => out.push_str("&amp;"),
      '<' => out.push_str("&lt;"),
      '>' => out.push_str("&gt;"),
      '"' => out.push_str("&quot;"),
      _ => out.push(character),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extracts_filter_and_injects_id() {
    let reference = FilterReference::from_markup(
      r#"<filter color-interpolation-filters="sRGB"><feGaussianBlur stdDeviation="2"/></filter>"#,
    )
    .unwrap();
    assert!(
      reference
        .markup
        .starts_with(r#"<filter id="takumi-filter""#)
    );
    assert!(reference.markup.ends_with("</filter>"));
  }

  #[test]
  fn replaces_existing_id() {
    let reference =
      FilterReference::from_markup(r#"<filter id="dither"><feFlood flood-color="red"/></filter>"#)
        .unwrap();
    assert_eq!(
      &*reference.markup,
      r#"<filter id="takumi-filter"><feFlood flood-color="red"></feFlood></filter>"#
    );
  }

  #[test]
  fn replaces_whitespace_and_entity_id() {
    let reference =
      FilterReference::from_markup("<filter id = 'a&#98;c'><feFlood/></filter>").unwrap();
    assert_eq!(
      &*reference.markup,
      r#"<filter id="takumi-filter"><feFlood></feFlood></filter>"#
    );
  }

  #[test]
  fn finds_filter_inside_svg_document() {
    let reference = FilterReference::from_markup(
      r#"<svg xmlns="http://www.w3.org/2000/svg"><defs><filter id="f"><feFlood/></filter></defs><rect/></svg>"#,
    )
    .unwrap();
    assert_eq!(
      &*reference.markup,
      r#"<filter id="takumi-filter"><feFlood></feFlood></filter>"#
    );
  }

  #[test]
  fn rejects_missing_filter() {
    assert_eq!(
      FilterReference::from_markup(r"<svg><rect/></svg>",),
      Err(FilterReferenceError::MissingFilterElement)
    );
  }

  #[test]
  fn keeps_only_filter_primitives() {
    let reference = FilterReference::from_markup(
      "<filter><feFlood/><script>alert(1)</script><foreignObject><style></filter><script>x</script></style></foreignObject></filter>",
    )
    .unwrap();

    assert_eq!(
      &*reference.markup,
      r#"<filter id="takumi-filter"><feFlood></feFlood></filter>"#
    );
  }

  #[test]
  fn escapes_attribute_values_and_drops_unreadable_names() {
    let reference = FilterReference::from_markup(
      r#"<filter><feFlood flood-color='"><script>' a"b="c"/></filter>"#,
    )
    .unwrap();

    assert_eq!(
      &*reference.markup,
      r#"<filter id="takumi-filter"><feFlood flood-color="&quot;&gt;&lt;script&gt;"></feFlood></filter>"#
    );
  }

  #[test]
  fn from_url_decodes_percent_encoding() {
    let reference = FilterReference::from_url(
      "data:image/svg+xml,%3Cfilter%3E%3CfeGaussianBlur stdDeviation='2'/%3E%3C/filter%3E",
    )
    .unwrap();
    assert!(reference.markup.contains("feGaussianBlur"));
  }

  #[test]
  fn from_url_rejects_non_data() {
    assert_eq!(
      FilterReference::from_url("https://example.com/f.svg"),
      Err(FilterReferenceError::UnsupportedUrl)
    );
    assert_eq!(
      FilterReference::from_url("#local"),
      Err(FilterReferenceError::UnsupportedUrl)
    );
  }
}
