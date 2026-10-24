//! An SVG's size from its root element alone. Follows usvg's `resolve_svg_size`, except that a
//! percentage size without a `viewBox` stays at the 100 px default instead of the content bounds.

use cssparser::Parser;
use roxmltree::{Document, Node, ParsingOptions};
use thiserror::Error;

use crate::{
  geometry::Size,
  style::{FontUnits, FromCssStr, IntrinsicSizing, Length, SizingContext},
  viewport::Viewport,
};

const FONT_SIZE: f32 = 12.0;
const DEFAULT_SIZE: f32 = 100.0;
#[cfg(any(test, not(feature = "svg")))]
const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";

/// The size usvg gives an SVG tree, plus its CSS intrinsic sizing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SvgSize {
  pub(crate) width: f32,
  pub(crate) height: f32,
  /// Per <https://www.w3.org/TR/SVG/coords.html#IntrinsicSizing>: a non-percentage
  /// `width`/`height` is an intrinsic dimension, the `viewBox` gives the ratio.
  pub(crate) intrinsic: IntrinsicSizing,
}

/// The `viewBox` attribute: the user-space rectangle a viewport maps onto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ViewBox {
  pub(crate) x: f32,
  pub(crate) y: f32,
  pub(crate) width: f32,
  pub(crate) height: f32,
}

impl ViewBox {
  /// Four numbers, spaced or comma-separated, with a positive size.
  pub(crate) fn parse(text: &str) -> Option<Self> {
    let mut parser = Parser::new(text);
    let mut values = [0.0; 4];

    for value in &mut values {
      *value = parser.expect_number().ok()?;

      let _ = parser.try_parse(Parser::expect_comma);
    }

    let [x, y, width, height] = values;

    (parser.is_exhausted() && width > 0.0 && height > 0.0).then_some(Self {
      x,
      y,
      width,
      height,
    })
  }
}

#[derive(Debug, Error)]
pub(crate) enum SvgSizeError {
  #[error(transparent)]
  Xml(#[from] roxmltree::Error),
  #[error("SVG has an invalid size")]
  InvalidSize,
  #[cfg(any(test, not(feature = "svg")))]
  #[error("SVG root element is not <svg>")]
  InvalidRoot,
}

impl SvgSize {
  #[cfg(any(test, not(feature = "svg")))]
  pub(crate) fn parse(markup: &str) -> Result<Self, SvgSizeError> {
    let document = parse_svg_document(markup)?;
    let root = document.root_element();
    let tag = root.tag_name();
    if tag.name() != "svg" || !matches!(tag.namespace(), None | Some(SVG_NAMESPACE)) {
      return Err(SvgSizeError::InvalidRoot);
    }

    Self::from_root(root)
  }

  pub(crate) fn from_root(root: Node) -> Result<Self, SvgSizeError> {
    let length = |name| {
      root
        .attribute(name)
        .and_then(|text| Length::from_css_str(text).ok())
        .filter(|length| !matches!(length, Length::Auto))
    };
    let sizing = SizingContext::builder()
      .viewport(Viewport::new((0, 0)).with_font_size(FONT_SIZE))
      .build();
    let sizing = length("font-size").map_or(sizing.clone(), |size| {
      let font_size = size.to_px(&sizing, FONT_SIZE);

      sizing.with_font_metrics(
        font_size,
        None,
        font_size,
        None,
        FontUnits {
          ch: font_size / 2.0,
          ex: font_size / 2.0,
        },
      )
    });
    let view_box = root
      .attribute("viewBox")
      .and_then(ViewBox::parse)
      .map(|view_box| (view_box.width, view_box.height));

    let width = length("width");
    let height = length("height");
    let default = Length::Percentage(100.0);

    let (width_px, height_px) = match view_box {
      Some((box_width, box_height)) => {
        let width_px = width.unwrap_or(default).to_px(&sizing, box_width);
        let height_px = height.unwrap_or(default).to_px(&sizing, box_height);
        match (width, height) {
          (Some(_), None) => (width_px, box_height * width_px / box_width),
          (None, Some(_)) => (box_width * height_px / box_height, height_px),
          _ => (width_px, height_px),
        }
      }
      None => (
        width.unwrap_or(default).to_px(&sizing, DEFAULT_SIZE),
        height.unwrap_or(default).to_px(&sizing, DEFAULT_SIZE),
      ),
    };

    if !positive(width_px) || !positive(height_px) {
      return Err(SvgSizeError::InvalidSize);
    }

    let absolute = |length: Option<Length>, px| {
      length
        .filter(|length| !matches!(length, Length::Percentage(_)))
        .map(|_| px)
    };
    let intrinsic_width = absolute(width, width_px);
    let intrinsic_height = absolute(height, height_px);
    let aspect_ratio = match (intrinsic_width, intrinsic_height) {
      (Some(width), Some(height)) => Some(Size { width, height }),
      _ => view_box.map(|(width, height)| Size { width, height }),
    };

    Ok(Self {
      width: width_px,
      height: height_px,
      intrinsic: IntrinsicSizing {
        width: intrinsic_width,
        height: intrinsic_height,
        aspect_ratio,
      },
    })
  }
}

/// Parses SVG markup as XML, with the DTD entities usvg also accepts.
pub(crate) fn parse_svg_document(markup: &str) -> Result<Document<'_>, roxmltree::Error> {
  Document::parse_with_options(
    markup,
    ParsingOptions {
      allow_dtd: true,
      ..Default::default()
    },
  )
}

fn positive(value: f32) -> bool {
  value.is_finite() && value > 0.0
}

#[cfg(test)]
mod tests {
  use super::*;

  fn size(root: &str) -> SvgSize {
    SvgSize::parse(&format!(
      r#"<!-- <svg width="1" height="1"/> --><svg xmlns="http://www.w3.org/2000/svg" {root}><rect width="5" height="5"/></svg>"#
    ))
    .unwrap()
  }

  #[test]
  fn absolute_lengths_convert_to_px() {
    let svg = size(r#"width="2in" height="72pt""#);
    assert_eq!((svg.width, svg.height), (192.0, 96.0));
    assert_eq!(svg.intrinsic.width, Some(192.0));
    assert_eq!(svg.intrinsic.height, Some(96.0));
    assert_eq!(svg.intrinsic.ratio(), Some(2.0));
  }

  #[test]
  fn auto_is_not_a_root_length() {
    let svg = size(r#"width="auto" height="auto""#);

    assert_eq!((svg.width, svg.height), (DEFAULT_SIZE, DEFAULT_SIZE));
  }

  #[test]
  fn em_follows_the_root_font_size() {
    assert_eq!(size(r#"width="2em" height="1em""#).width, 24.0);
    assert_eq!(
      size(r#"font-size="20" width="2em" height="1em""#).width,
      40.0
    );
  }

  #[test]
  fn view_box_supplies_the_ratio_and_missing_dimension() {
    let svg = size(r#"viewBox="0 0 200 100""#);
    assert_eq!((svg.width, svg.height), (200.0, 100.0));
    assert_eq!(
      svg.intrinsic,
      IntrinsicSizing {
        width: None,
        height: None,
        aspect_ratio: Some(Size {
          width: 200.0,
          height: 100.0
        })
      }
    );

    let svg = size(r#"width="50" viewBox="0 0 200 100""#);
    assert_eq!((svg.width, svg.height), (50.0, 25.0));
    assert_eq!(svg.intrinsic.width, Some(50.0));
    assert_eq!(svg.intrinsic.height, None);
  }

  #[test]
  fn percentages_scale_the_view_box_but_are_not_intrinsic() {
    let svg = size(r#"width="50%" height="100%" viewBox="0 0 200 100""#);
    assert_eq!((svg.width, svg.height), (100.0, 100.0));
    assert_eq!(svg.intrinsic.width, None);
    assert_eq!(svg.intrinsic.ratio(), Some(2.0));
  }

  #[test]
  fn no_size_falls_back_to_the_default() {
    let svg = size("");
    assert_eq!((svg.width, svg.height), (100.0, 100.0));
    assert_eq!(svg.intrinsic, IntrinsicSizing::default());
  }

  #[test]
  fn unparsable_lengths_fall_back_like_missing_ones() {
    assert_eq!(size(r#"width="wide" height="10""#).width, 100.0);
  }

  #[test]
  fn the_root_must_be_an_svg_element() {
    let nested = r#"<html><svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/></html>"#;
    let foreign = r#"<svg xmlns="http://example.com/ns" width="10" height="10"/>"#;
    for markup in [nested, foreign] {
      assert!(matches!(
        SvgSize::parse(markup),
        Err(SvgSizeError::InvalidRoot)
      ));
    }
    assert_eq!(
      SvgSize::parse(r#"<svg width="10" height="10"/>"#)
        .unwrap()
        .width,
      10.0
    );
  }

  #[test]
  fn zero_size_is_an_error() {
    let markup = r#"<svg xmlns="http://www.w3.org/2000/svg" width="0" height="10"/>"#;
    assert!(matches!(
      SvgSize::parse(markup),
      Err(SvgSizeError::InvalidSize)
    ));
  }
}
