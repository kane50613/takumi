// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod clippath;
mod converter;
mod filter;
mod image;
mod marker;
mod mask;
mod options;
mod paint_server;
mod shapes;
mod style;
mod svgtree;
mod switch;
mod units;
mod use_node;

use std::sync::Arc;

use crate::resvg::usvg::NonZeroRect;
use crate::resvg::usvg::filter::Filter;
use svgtree::AId;

pub use image::ImageHrefResolver;
pub use options::Options;

/// List of all errors.
#[derive(Debug)]
pub enum Error {
  /// Only UTF-8 content are supported.
  NotAnUtf8Str,

  /// `svgz` feature is required to parse SVGZ data.
  SvgzFeatureNotEnabled,

  /// Compressed SVG must use the GZip algorithm.
  MalformedGZip,

  /// We do not allow SVG with more than 1_000_000 elements for security reasons.
  ElementsLimitReached,

  /// SVG doesn't have a valid size.
  ///
  /// Occurs when width and/or height are <= 0.
  ///
  /// Also occurs if width, height and viewBox are not set.
  InvalidSize,

  /// Failed to parse an SVG data.
  ParsingFailed(roxmltree::Error),
}

impl From<roxmltree::Error> for Error {
  fn from(e: roxmltree::Error) -> Self {
    Error::ParsingFailed(e)
  }
}

impl std::fmt::Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match *self {
      Error::NotAnUtf8Str => {
        write!(f, "provided data has not an UTF-8 encoding")
      }
      Self::SvgzFeatureNotEnabled => {
        write!(f, "enable svgz cargo feature to decode SVGZ data")
      }
      Error::MalformedGZip => {
        write!(f, "provided data has a malformed GZip content")
      }
      Error::ElementsLimitReached => {
        write!(f, "the maximum number of SVG elements has been reached")
      }
      Error::InvalidSize => {
        write!(f, "SVG has an invalid size")
      }
      Error::ParsingFailed(ref e) => {
        write!(f, "SVG data parsing failed cause {}", e)
      }
    }
  }
}

impl std::error::Error for Error {}

pub(crate) trait OptionLog {
  fn log_none<F: FnOnce()>(self, f: F) -> Self;
}

impl<T> OptionLog for Option<T> {
  #[inline]
  fn log_none<F: FnOnce()>(self, f: F) -> Self {
    self.or_else(|| {
      f();
      None
    })
  }
}

impl crate::resvg::usvg::Tree {
  /// Parses `Tree` from an SVG data.
  ///
  /// Can contain an SVG string or a gzip compressed data.
  pub fn from_data(data: &[u8], opt: &Options) -> Result<Self, Error> {
    if data.starts_with(&[0x1f, 0x8b]) {
      Err(Error::SvgzFeatureNotEnabled)
    } else {
      let text = std::str::from_utf8(data).map_err(|_| Error::NotAnUtf8Str)?;
      Self::from_str(text, opt)
    }
  }

  /// Similar to the `from_data` method, except that it ignores all `image` elements linking to
  /// external files, as required by the SVG specification when SVG files are loaded
  /// for `<image href="..." />` tags.
  pub fn from_data_nested(data: &[u8], opt: &Options) -> Result<Self, Error> {
    let nested_opt = Options {
      resources_dir: None,
      dpi: opt.dpi,
      font_size: opt.font_size,
      languages: opt.languages.clone(),
      shape_rendering: opt.shape_rendering,
      text_rendering: opt.text_rendering,
      image_rendering: opt.image_rendering,
      default_size: opt.default_size,
      current_color: opt.current_color,
      image_href_resolver: ImageHrefResolver {
        resolve_data: Box::new(|a, b, c| (opt.image_href_resolver.resolve_data)(a, b, c)),
        // External images should be ignored.
        resolve_string: Box::new(|_, _| None),
      },
      ..Options::default()
    };

    Self::from_data(data, &nested_opt)
  }

  /// Parses `Tree` from an SVG string.
  pub fn from_str(text: &str, opt: &Options) -> Result<Self, Error> {
    let xml_opt = roxmltree::ParsingOptions {
      allow_dtd: true,
      ..Default::default()
    };

    let doc =
      roxmltree::Document::parse_with_options(text, xml_opt).map_err(Error::ParsingFailed)?;

    Self::from_xmltree(&doc, opt)
  }

  /// Parses `Tree` from `roxmltree::Document`.
  pub fn from_xmltree(doc: &roxmltree::Document, opt: &Options) -> Result<Self, Error> {
    let doc = svgtree::Document::parse_tree(doc, opt.style_sheet.as_deref())?;
    self::converter::convert_doc(&doc, opt)
  }
}

/// Parses standalone `<filter>` markup (carrying `id="{filter_id}"`) and
/// resolves it against a `width` x `height` element box, skipping the render
/// tree entirely.
///
/// Returns `Ok(None)` when the filter reference is invalid, which per the
/// converter's behaviour hides the filtered element.
pub(crate) fn filters_from_markup(
  markup: &str,
  filter_id: &str,
  width: f32,
  height: f32,
  opt: &Options,
) -> Result<Option<Vec<Arc<Filter>>>, Error> {
  let document = format!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}">{markup}<rect width="{width}" height="{height}" filter="url(#{filter_id})"/></svg>"#
  );
  let xml_opt = roxmltree::ParsingOptions {
    allow_dtd: true,
    ..Default::default()
  };
  let xml =
    roxmltree::Document::parse_with_options(&document, xml_opt).map_err(Error::ParsingFailed)?;
  let doc = svgtree::Document::parse_tree(&xml, None)?;

  let bbox = NonZeroRect::from_xywh(0.0, 0.0, width, height).ok_or(Error::InvalidSize)?;
  let state = converter::State {
    parent_clip_path: None,
    parent_markers: Vec::new(),
    context_element: None,
    fe_image_link: false,
    view_box: bbox,
    use_size: (None, None),
    opt,
  };
  let mut cache = converter::Cache::new();
  let node = doc
    .descendants()
    .find(|n| n.has_attribute(AId::Filter))
    .ok_or(Error::InvalidSize)?;

  Ok(filter::convert(node, &state, Some(bbox), &mut cache).ok())
}

#[inline]
pub(crate) fn f32_bound(min: f32, val: f32, max: f32) -> f32 {
  debug_assert!(min.is_finite());
  debug_assert!(max.is_finite());

  if val > max {
    max
  } else if val >= min {
    val
  } else {
    // Catches `val < min` as well as a NaN `val`.
    min
  }
}
