//! SVG `<filter>` references in `filter: url(data:image/svg+xml,...)`.

use std::{str::from_utf8, sync::Arc};

use crate::{
  html::{
    dom::{Attribute, AttributeNamespace, NodeData, NodeRef},
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

  /// The first `<filter>` element in `markup`, a filter or an `<svg>` document holding one, with
  /// its `id` replaced by [`FilterReference::ID`].
  pub(crate) fn from_markup(markup: &str) -> Result<Self, FilterReferenceError> {
    // Inside `<svg>`, the parser keeps a lone `<filter>` and its primitives in the SVG namespace.
    let mut dom = parse(&format!("<svg>{markup}</svg>"), false);
    let filter = first_filter(dom.document())
      .ok_or(FilterReferenceError::MissingFilterElement)?
      .id();

    if let NodeData::Element { attrs, .. } = dom.data_mut(filter) {
      attrs
        .retain(|attribute| attribute.ns != AttributeNamespace::None || &*attribute.local != "id");
      attrs.insert(
        0,
        Attribute {
          ns: AttributeNamespace::None,
          local: "id".into(),
          value: Self::ID.into(),
        },
      );
    }

    Ok(Self {
      markup: String::from_utf8_lossy(&dom.node(filter).outer_html()).into(),
    })
  }
}

/// The first `filter` element at or under `node`, in document order.
fn first_filter(node: NodeRef<'_>) -> Option<NodeRef<'_>> {
  if let NodeData::Element { name, .. } = node.data()
    && &*name.local == "filter"
  {
    return Some(node);
  }

  node.children().find_map(first_filter)
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
