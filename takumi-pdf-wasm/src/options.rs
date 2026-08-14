//! Deserialization of the render/measure option object, and the geometry and
//! image resolution it drives.

use std::{collections::HashMap, sync::Arc};

use serde::Deserialize;
use serde_bytes::ByteBuf;
use takumi_core::{
  layout::node::Node,
  resources::image::{ImageCacheMode, ImageSource as DecodedImage, ResourceCache},
  style::{Color, ColorInput, FromCssStr},
  viewport::Viewport,
};
use takumi_pdf::{PageMargins, PageOptions};

use crate::{
  map_error,
  metadata::{AttachmentInput, MetadataInput, PdfaInput, TaggedInput},
};

/// An image source with its URL and raw data.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageSource {
  src: Arc<str>,
  data: ByteBuf,
  cache: Option<ImageCacheMode>,
}

/// Explicit page or viewport dimensions in CSS px.
#[derive(Deserialize, Clone, Copy)]
struct Dimensions {
  width: f32,
  height: f32,
}

/// Viewport for single-page output. A missing height sizes the page to the
/// laid-out content.
#[derive(Deserialize, Clone, Copy)]
struct ViewportInput {
  width: f32,
  height: Option<f32>,
}

/// A page size: a preset name (`"a4"`, `"letter"`) or explicit dimensions.
#[derive(Deserialize)]
#[serde(untagged)]
enum SizeInput {
  Named(String),
  Dimensions(Dimensions),
}

/// A page margin: one number for all sides, or per-side values (missing
/// sides are zero).
#[derive(Deserialize)]
#[serde(untagged)]
enum MarginInput {
  Uniform(f32),
  Sides {
    top: Option<f32>,
    right: Option<f32>,
    bottom: Option<f32>,
    left: Option<f32>,
  },
}

fn resolve_page(
  size: Option<&SizeInput>,
  landscape: bool,
  margin: Option<&MarginInput>,
) -> Result<PageOptions, js_sys::Error> {
  let mut page = match size {
    None => PageOptions::A4,
    Some(SizeInput::Dimensions(dimensions)) => PageOptions {
      width: dimensions.width,
      height: dimensions.height,
      ..PageOptions::A4
    },
    Some(SizeInput::Named(name)) => match name.to_ascii_lowercase().as_str() {
      "a4" => PageOptions::A4,
      "letter" => PageOptions::LETTER,
      other => {
        return Err(js_sys::Error::new(&format!("unknown page size: {other}")));
      }
    },
  };

  if landscape {
    page = page.landscape();
  }
  match margin {
    None => {}
    Some(MarginInput::Uniform(value)) => page.margin = PageMargins::uniform(*value),
    Some(MarginInput::Sides {
      top,
      right,
      bottom,
      left,
    }) => {
      page.margin = PageMargins {
        top: top.unwrap_or(0.0),
        right: right.unwrap_or(0.0),
        bottom: bottom.unwrap_or(0.0),
        left: left.unwrap_or(0.0),
      };
    }
  }
  Ok(page)
}

/// The paper color, parsed from a CSS color.
pub(crate) fn page_background(color: Option<&str>) -> Result<Option<Color>, js_sys::Error> {
  color
    .map(|color| match ColorInput::from_css_str(color) {
      Ok(ColorInput::Value(color)) => Ok(color),
      _ => Err(js_sys::Error::new(&format!(
        "backgroundColor is not a color: {color}"
      ))),
    })
    .transpose()
}

/// Options for rendering a PDF.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PdfRenderOptions {
  /// Fixed viewport for single-page output: percentage heights resolve
  /// against it and overflowing content is clipped. Mutually exclusive with
  /// the paged fields.
  viewport: Option<ViewportInput>,
  /// Page size for paged output. Defaults to A4.
  size: Option<SizeInput>,
  /// Swaps the page's width and height, including explicit sizes.
  landscape: Option<bool>,
  /// Page margin in CSS px: one number or per-side values.
  margin: Option<MarginInput>,
  /// Paper color as a CSS color, painted under everything on every page.
  pub(crate) background_color: Option<String>,
  /// Band repeated at the top of every page. Nodes classed `pageNumber` /
  /// `totalPages` receive the counters, optionally formatted by a
  /// supported `@counter-style` name in the same class list.
  pub(crate) header: Option<Node>,
  /// Band repeated at the bottom of every page; same class hooks as `header`.
  pub(crate) footer: Option<Node>,
  /// Pre-fetched images keyed by URL.
  pub(crate) images: Option<Vec<ImageSource>>,
  /// CSS stylesheets to apply before layout.
  pub(crate) stylesheets: Option<Vec<String>>,
  /// Per-render font stack: ordered family names used as the fallback chain.
  pub(crate) font_families: Option<Vec<String>>,
  /// Default BCP-47 language tag applied to the root.
  pub(crate) lang: Option<String>,
  /// Document metadata written to the PDF's info dictionary.
  pub(crate) metadata: Option<MetadataInput>,
  /// Generates a PDF outline (bookmarks) from `h1`–`h6` headings.
  pub(crate) outline: Option<bool>,
  /// PDF/A conformance level: "2a", "2b", "2u", "3a", "3b", "3u", "4" or
  /// "4f".
  pub(crate) pdfa: Option<PdfaInput>,
  /// Structure-tree emission: `false`, `true` (default), or `"ua1"` / `"ua2"`
  /// to also validate against that accessibility standard.
  pub(crate) tagged: Option<TaggedInput>,
  /// Files attached to the document.
  pub(crate) attachments: Option<Vec<AttachmentInput>>,
}

pub(crate) fn decode_images(
  cache: &ResourceCache,
  sources: Option<Vec<ImageSource>>,
) -> Result<HashMap<Arc<str>, DecodedImage>, js_sys::Error> {
  let mut images = HashMap::new();

  for source in sources.unwrap_or_default() {
    let image = cache
      .get_or_decode(&source.data, source.cache.unwrap_or_default())
      .map_err(map_error)?;

    images.insert(source.src, image);
  }
  Ok(images)
}

/// Splits the options into single-page viewport or paged geometry, rejecting
/// a mix of both.
pub(crate) fn resolve_geometry(
  options: &PdfRenderOptions,
) -> Result<(Option<Viewport>, Option<PageOptions>), js_sys::Error> {
  let paged_field_set = options.size.is_some()
    || options.landscape.is_some()
    || options.margin.is_some()
    || options.header.is_some()
    || options.footer.is_some();

  match options.viewport {
    Some(_) if paged_field_set => Err(js_sys::Error::new(
      "viewport is mutually exclusive with the paged options (size, landscape, margin, header, footer)",
    )),
    Some(input) => Ok((
      Some(Viewport::new((
        input.width as u32,
        input.height.map(|height| height as u32),
      ))),
      None,
    )),
    None => Ok((
      None,
      Some(resolve_page(
        options.size.as_ref(),
        options.landscape.unwrap_or(false),
        options.margin.as_ref(),
      )?),
    )),
  }
}
