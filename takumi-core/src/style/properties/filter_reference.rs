//! Parsed SVG `<filter>` references for `filter: url(data:image/svg+xml,...)`.
//!
//! The markup is validated and extracted at CSS parse time; execution is
//! delegated to `resvg` in the raster backend and emitted verbatim by the SVG
//! backend, so both sides share one spec implementation.

use std::sync::Arc;

use data_url::DataUrl;
use roxmltree::{Document, Node};

/// Error produced while parsing a filter reference.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FilterReferenceError {
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
  /// The filter contains no primitives.
  #[error("<filter> has no primitives")]
  EmptyFilter,
  /// An element inside `<filter>` is not a filter primitive.
  #[error("unsupported element <{0}> inside <filter>")]
  UnsupportedPrimitive(String),
  /// An attribute value is outside the supported subset.
  #[error("unsupported {attribute}: {value}")]
  UnsupportedValue {
    /// What was rejected.
    attribute: &'static str,
    /// The rejected value.
    value: String,
  },
}

/// A validated SVG filter reference.
#[derive(Debug, Clone)]
pub struct FilterReference {
  /// The original `url()` value, kept for serialization and equality.
  pub uri: Arc<str>,
  /// The `<filter>` element markup. Any author `id` is stripped and replaced
  /// with [`FilterReference::ID`], so consumers can rewrite it textually.
  pub markup: Arc<str>,
}

impl PartialEq for FilterReference {
  fn eq(&self, other: &Self) -> bool {
    self.uri == other.uri
  }
}

const SVG_XML_MIME: (&str, &str) = ("image", "svg+xml");

/// Filter primitives and their light-source children. Anything else inside
/// `<filter>` (scripts, `foreignObject`, animation elements) is rejected, since
/// the markup is later emitted verbatim into generated SVG.
const ALLOWED_ELEMENTS: &[&str] = &[
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
];

impl FilterReference {
  /// The `id` attribute value guaranteed on [`FilterReference::markup`].
  pub const ID: &str = "takumi-filter";
  /// Parses a `filter: url(...)` value. Only `data:image/svg+xml` URIs are
  /// supported; there is no document to resolve fragments or external URLs
  /// against.
  pub fn from_url(url: &str) -> Result<Self, FilterReferenceError> {
    if !url
      .get(..5)
      .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
    {
      return Err(FilterReferenceError::UnsupportedUrl);
    }

    // An unescaped `#` (`url(#id)` inside the markup, hex colors) is a URL
    // fragment delimiter and would truncate the body.
    let escaped = url.split_once(',').and_then(|(header, body)| {
      body
        .contains('#')
        .then(|| format!("{header},{}", body.replace('#', "%23")))
    });
    let processed = DataUrl::process(escaped.as_deref().unwrap_or(url))
      .map_err(|_| FilterReferenceError::InvalidDataUri("malformed data URI"))?;

    let mime = processed.mime_type();
    if (mime.type_.as_str(), mime.subtype.as_str()) != SVG_XML_MIME {
      return Err(FilterReferenceError::UnsupportedUrl);
    }

    let (bytes, _) = processed
      .decode_to_vec()
      .map_err(|_| FilterReferenceError::InvalidDataUri("undecodable body"))?;
    let text = std::str::from_utf8(&bytes)
      .map_err(|_| FilterReferenceError::InvalidDataUri("body is not UTF-8"))?;

    Self::from_markup(text, url)
  }

  /// Validates `<filter>` markup (optionally wrapped in an `<svg>` document)
  /// and extracts the filter element.
  pub fn from_markup(xml: &str, uri: &str) -> Result<Self, FilterReferenceError> {
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

    if !filter.children().any(|child| child.is_element()) {
      return Err(FilterReferenceError::EmptyFilter);
    }

    for node in filter.descendants().filter(Node::is_element) {
      if node != filter && !ALLOWED_ELEMENTS.contains(&node.tag_name().name()) {
        return Err(FilterReferenceError::UnsupportedPrimitive(
          node.tag_name().name().into(),
        ));
      }
      for attribute in node.attributes() {
        let name = attribute.name();
        if name.len() >= 3
          && name
            .get(..2)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("on"))
        {
          return Err(FilterReferenceError::UnsupportedValue {
            attribute: "event handler",
            value: name.into(),
          });
        }
        if name == "href"
          && !attribute
            .value()
            .trim_start()
            .get(..5)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
        {
          return Err(FilterReferenceError::UnsupportedValue {
            attribute: "href",
            value: "only data: URIs are supported".into(),
          });
        }
      }
    }

    let filter_range = filter.range();
    let mut markup = xml[filter_range.clone()].to_string();

    // Strip any author id (via its byte range, so whitespace and entity
    // encoding don't matter) and inject the canonical one, which the SVG
    // backend rewrites textually.
    if let Some(id) = filter
      .attributes()
      .find(|attribute| attribute.namespace().is_none() && attribute.name() == "id")
    {
      let range = id.range();
      markup.replace_range(
        range.start - filter_range.start..range.end - filter_range.start,
        "",
      );
    }
    let rest = markup
      .strip_prefix("<filter")
      .ok_or(FilterReferenceError::MissingFilterElement)?;
    let markup = format!(r#"<filter id="{id}"{rest}"#, id = Self::ID);

    Ok(Self {
      uri: uri.into(),
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
      "test",
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
    let reference = FilterReference::from_markup(
      r#"<filter id="dither"><feFlood flood-color="red"/></filter>"#,
      "test",
    )
    .unwrap();
    assert_eq!(
      &*reference.markup,
      r#"<filter id="takumi-filter" ><feFlood flood-color="red"/></filter>"#
    );
  }

  #[test]
  fn replaces_whitespace_and_entity_id() {
    let reference =
      FilterReference::from_markup("<filter id = 'a&#98;c'><feFlood/></filter>", "test").unwrap();
    assert_eq!(
      &*reference.markup,
      r#"<filter id="takumi-filter" ><feFlood/></filter>"#
    );
  }

  #[test]
  fn finds_filter_inside_svg_document() {
    let reference = FilterReference::from_markup(
      r#"<svg xmlns="http://www.w3.org/2000/svg"><defs><filter id="f"><feFlood/></filter></defs><rect/></svg>"#,
      "test",
    )
    .unwrap();
    assert_eq!(
      &*reference.markup,
      r#"<filter id="takumi-filter" ><feFlood/></filter>"#
    );
  }

  #[test]
  fn rejects_active_content() {
    assert!(matches!(
      FilterReference::from_markup(r"<filter><script>alert(1)</script></filter>", "test"),
      Err(FilterReferenceError::UnsupportedPrimitive(name)) if name == "script"
    ));
    assert!(matches!(
      FilterReference::from_markup(r#"<filter><feFlood onload="alert(1)"/></filter>"#, "test",),
      Err(FilterReferenceError::UnsupportedValue { .. })
    ));
    assert!(matches!(
      FilterReference::from_markup(
        r#"<filter><feImage href="https://example.com/x.png"/></filter>"#,
        "test",
      ),
      Err(FilterReferenceError::UnsupportedValue { .. })
    ));
  }

  #[test]
  fn rejects_empty_filter() {
    assert_eq!(
      FilterReference::from_markup(r"<filter></filter>", "test"),
      Err(FilterReferenceError::EmptyFilter)
    );
  }

  #[test]
  fn rejects_missing_filter() {
    assert_eq!(
      FilterReference::from_markup(r"<svg><rect/></svg>", "test"),
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
