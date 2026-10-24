// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::Options;
use crate::svg::layout::ImageHrefResolver;
use crate::svg::{Document, ParseError, ParseOptions, StyleContext, layout::convert_doc};

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

impl From<ParseError> for Error {
  fn from(error: ParseError) -> Self {
    match error {
      ParseError::Xml(error) => Error::ParsingFailed(error),
      ParseError::NotSvg => Error::ParsingFailed(roxmltree::Error::NoRootNode),
      ParseError::TooLarge => Error::ElementsLimitReached,
    }
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

impl crate::svg::tree::Tree {
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
      current_color: opt.current_color,
      image_href_resolver: ImageHrefResolver {
        resolve_data: Box::new(|a, b, c| (opt.image_href_resolver.resolve_data)(a, b, c)),
        // External images should be ignored.
        resolve_string: Box::new(|_, _| None),
      },
      fonts: opt.fonts.clone(),
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
    let doc = Document::from_xml(
      doc,
      &ParseOptions {
        style: StyleContext {
          font_size: opt.font_size,
          dpi: opt.dpi,
        },
        style_sheet: opt.style_sheet.as_deref(),
      },
    )?;

    Self::from_document(&doc, opt)
  }

  /// Lays out a parsed `Document` as a `Tree`.
  pub fn from_document(document: &Document, opt: &Options) -> Result<Self, Error> {
    convert_doc(document, opt)
  }
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
