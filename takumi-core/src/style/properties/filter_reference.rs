//! Parsed SVG `<filter>` references for `filter: url(data:image/svg+xml,...)`.
//!
//! The markup is validated and extracted at CSS parse time; execution is
//! delegated to `resvg` in the raster backend and emitted verbatim by the SVG
//! backend, so both sides share one spec implementation.

use std::{str::from_utf8, sync::Arc};

use roxmltree::Document;

use crate::resources::image::{DataUriError, decode_data_uri};

/// Error produced while parsing a filter reference.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum FilterReferenceError {
  /// The URL is not a `data:image/svg+xml` URI.
  #[error("filter url() must be a data:image/svg+xml URI")]
  UnsupportedUrl,
  /// The data URI could not be decoded.
  #[error("invalid data URI: {0}")]
  InvalidDataUri(&'static str),
  /// The XML markup could not be parsed.
  #[error("invalid filter markup: {0}")]
  InvalidMarkup(String),
  /// The document contains no `<filter>` element.
  #[error("no <filter> element found")]
  MissingFilterElement,
}

/// A validated SVG filter reference.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterReference {
  /// The `<filter>` element markup. Any author `id` is stripped and replaced
  /// with [`FilterReference::ID`], so consumers can rewrite it textually.
  /// Serialization rebuilds a minimal `data:` URI from it.
  pub markup: Arc<str>,
}

impl FilterReference {
  /// The `id` attribute value guaranteed on [`FilterReference::markup`].
  pub const ID: &str = "takumi-filter";
  /// Parses a `filter: url(...)` value. Only `data:image/svg+xml` URIs are
  /// supported; there is no document to resolve fragments or external URLs
  /// against.
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

  /// Validates `<filter>` markup (optionally wrapped in an `<svg>` document)
  /// and extracts the filter element.
  pub(crate) fn from_markup(xml: &str) -> Result<Self, FilterReferenceError> {
    let document = Document::parse(xml)
      .map_err(|error| FilterReferenceError::InvalidMarkup(error.to_string()))?;
    let root = document.root_element();

    let filter = if root.has_tag_name("filter") {
      root
    } else {
      root
        .descendants()
        .find(|node| node.has_tag_name("filter"))
        .ok_or(FilterReferenceError::MissingFilterElement)?
    };

    let filter_range = filter.range();
    let mut markup = xml[filter_range.clone()].to_string();

    // Strip any author id (via its byte range, so whitespace and entity
    // encoding don't matter) along with its leading whitespace, and inject the
    // canonical one, which the SVG backend rewrites textually.
    if let Some(id) = filter
      .attributes()
      .find(|attribute| attribute.namespace().is_none() && attribute.name() == "id")
    {
      let range = id.range();
      let mut start = range.start - filter_range.start;

      while start > 0 && markup.as_bytes()[start - 1].is_ascii_whitespace() {
        start -= 1;
      }
      markup.replace_range(start..range.end - filter_range.start, "");
    }
    let rest = markup
      .strip_prefix("<filter")
      .ok_or(FilterReferenceError::MissingFilterElement)?;
    let markup = format!(r#"<filter id="{id}"{rest}"#, id = Self::ID);

    Ok(Self {
      markup: markup.into(),
    })
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
      r#"<filter id="takumi-filter"><feFlood flood-color="red"/></filter>"#
    );
  }

  #[test]
  fn replaces_whitespace_and_entity_id() {
    let reference =
      FilterReference::from_markup("<filter id = 'a&#98;c'><feFlood/></filter>").unwrap();
    assert_eq!(
      &*reference.markup,
      r#"<filter id="takumi-filter"><feFlood/></filter>"#
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
      r#"<filter id="takumi-filter"><feFlood/></filter>"#
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
