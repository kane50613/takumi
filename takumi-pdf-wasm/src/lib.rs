//! WebAssembly bindings for takumi-pdf.

#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::{
  collections::HashMap,
  str::FromStr,
  sync::{Arc, RwLock},
};

use serde::{Deserialize, Deserializer, de::Error as DeError};
use serde_bytes::ByteBuf;
use serde_wasm_bindgen::{from_value, to_value};
use takumi_bindings_common::{build_font_resource, default_fonts, stylesheet};
use takumi_core::{
  Fonts,
  layout::node::Node,
  resources::{
    font::FontResource,
    image::{ImageCacheMode, ImageSource as DecodedImage, ResourceCache},
  },
  style::{FontFamily, FontStyle as CssFontStyle, FromCssStr, Lang},
  viewport::Viewport,
};
use takumi_pdf::{
  MeasureOptions, PageMargins, PageOptions, PdfDate, PdfMetadata, PdfOptions, PdfStandard, Tagging,
};
use wasm_bindgen::prelude::*;

fn map_error(error: impl core::fmt::Debug) -> js_sys::Error {
  js_sys::Error::new(&format!("{error:?}"))
}

#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT: &'static str = include_str!("./dts-header.d.ts");

#[wasm_bindgen]
extern "C" {
  /// JavaScript object representing a layout node.
  #[wasm_bindgen(typescript_type = "Node")]
  pub type NodeType;

  /// JavaScript type for font input (details object or raw buffer).
  #[wasm_bindgen(typescript_type = "Font")]
  pub type FontType;

  /// JavaScript type for the families produced by `registerFont`.
  #[wasm_bindgen(typescript_type = "RegisteredFamily[]")]
  pub type RegisteredFamiliesType;

  /// JavaScript object representing render options.
  #[wasm_bindgen(typescript_type = "PdfRenderOptions")]
  pub type PdfRenderOptionsType;

  /// JavaScript object representing measure options.
  #[wasm_bindgen(typescript_type = "MeasureOptions")]
  pub type MeasureOptionsType;

  /// JavaScript object representing a measured size.
  #[wasm_bindgen(typescript_type = "MeasuredSize")]
  pub type MeasuredSizeType;
}

/// Details for loading a custom font.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FontDetails {
  name: Option<String>,
  data: ByteBuf,
  weight: Option<f64>,
  style: Option<FontStyle>,
  subset_of: Option<String>,
  generic: Option<String>,
}

/// Font input, either as detailed object or raw buffer.
#[derive(Deserialize)]
#[serde(untagged)]
enum Font {
  Object(FontDetails),
  Buffer(ByteBuf),
}

struct FontStyle(CssFontStyle);

impl<'de> Deserialize<'de> for FontStyle {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let value = String::deserialize(deserializer)?;
    Ok(Self(
      CssFontStyle::from_css_str(&value).map_err(D::Error::custom)?,
    ))
  }
}

/// An image source with its URL and raw data.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageSource {
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

/// Options for rendering a PDF.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PdfRenderOptions {
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
  /// Band repeated at the top of every page. Nodes classed `pageNumber` /
  /// `totalPages` receive the counters, optionally formatted by a
  /// supported `@counter-style` name in the same class list.
  header: Option<Node>,
  /// Band repeated at the bottom of every page; same class hooks as `header`.
  footer: Option<Node>,
  /// Pre-fetched images keyed by URL.
  images: Option<Vec<ImageSource>>,
  /// CSS stylesheets to apply before layout.
  stylesheets: Option<Vec<String>>,
  /// Per-render font stack: ordered family names used as the fallback chain.
  font_families: Option<Vec<String>>,
  /// Default BCP-47 language tag applied to the root.
  lang: Option<String>,
  /// Document metadata written to the PDF's info dictionary.
  metadata: Option<MetadataInput>,
  /// Generates a PDF outline (bookmarks) from `h1`–`h6` headings.
  outline: Option<bool>,
  /// PDF/A conformance level: "2a", "2b", "2u", "3a", "3b", "3u" or "4".
  pdfa: Option<PdfaInput>,
  /// Structure-tree emission: `false`, `true` (default) or `"ua1"` to also
  /// validate against PDF/UA-1.
  tagged: Option<TaggedInput>,
}

/// `tagged` values accepted from JS.
#[derive(Deserialize, Clone, Copy)]
#[serde(untagged)]
enum TaggedInput {
  Enabled(bool),
  #[serde(with = "ua1_literal")]
  Ua1,
}

/// Deserializes the `"ua1"` string literal.
mod ua1_literal {
  use serde::{Deserialize, Deserializer};

  pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<(), D::Error> {
    let value = String::deserialize(deserializer)?;

    if value == "ua1" {
      Ok(())
    } else {
      Err(serde::de::Error::custom("expected \"ua1\""))
    }
  }
}

impl From<TaggedInput> for Tagging {
  fn from(tagged: TaggedInput) -> Self {
    match tagged {
      TaggedInput::Enabled(false) => Tagging::Off,
      TaggedInput::Enabled(true) => Tagging::On,
      TaggedInput::Ua1 => Tagging::Ua1,
    }
  }
}

/// PDF/A conformance level names accepted from JS.
#[derive(Deserialize, Clone, Copy)]
enum PdfaInput {
  #[serde(rename = "2a")]
  A2a,
  #[serde(rename = "3a")]
  A3a,
  #[serde(rename = "2b")]
  A2b,
  #[serde(rename = "2u")]
  A2u,
  #[serde(rename = "3b")]
  A3b,
  #[serde(rename = "3u")]
  A3u,
  #[serde(rename = "4")]
  A4,
}

impl From<PdfaInput> for PdfStandard {
  fn from(pdfa: PdfaInput) -> Self {
    match pdfa {
      PdfaInput::A2a => PdfStandard::A2a,
      PdfaInput::A3a => PdfStandard::A3a,
      PdfaInput::A2b => PdfStandard::A2b,
      PdfaInput::A2u => PdfStandard::A2u,
      PdfaInput::A3b => PdfStandard::A3b,
      PdfaInput::A3u => PdfStandard::A3u,
      PdfaInput::A4 => PdfStandard::A4,
    }
  }
}

/// Document metadata fields.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataInput {
  title: Option<String>,
  description: Option<String>,
  authors: Option<Vec<String>>,
  keywords: Option<Vec<String>>,
  creator: Option<String>,
  /// UTC creation date as `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS`.
  creation_date: Option<String>,
}

fn parse_field<T: FromStr>(part: Option<&str>, width: usize) -> Option<T> {
  let part = part?;

  if part.len() != width || !part.bytes().all(|byte| byte.is_ascii_digit()) {
    return None;
  }
  part.parse().ok()
}

fn parse_date(value: &str) -> Option<PdfDate> {
  let (date, time) = match value.split_once('T') {
    Some((date, time)) => (date, Some(time.strip_suffix('Z').unwrap_or(time))),
    None => (value, None),
  };
  let mut parts = date.splitn(3, '-');
  let year: u16 = parse_field(parts.next(), 4)?;
  let month: u8 = parse_field(parts.next(), 2)?;
  let day: u8 = parse_field(parts.next(), 2)?;
  let (hour, minute, second) = match time {
    Some(time) => {
      let mut parts = time.splitn(3, ':');
      (
        parse_field(parts.next(), 2)?,
        parse_field(parts.next(), 2)?,
        parse_field(parts.next(), 2)?,
      )
    }
    None => (0u8, 0u8, 0u8),
  };
  let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
  let days_in_month = match month {
    1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
    4 | 6 | 9 | 11 => 30,
    2 if leap => 29,
    2 => 28,
    _ => return None,
  };

  if day < 1 || day > days_in_month || hour > 23 || minute > 59 || second > 59 {
    return None;
  }
  Some(PdfDate {
    year,
    month,
    day,
    hour,
    minute,
    second,
  })
}

impl TryFrom<MetadataInput> for PdfMetadata {
  type Error = js_sys::Error;

  fn try_from(input: MetadataInput) -> Result<Self, Self::Error> {
    let creation_date = input
      .creation_date
      .as_deref()
      .map(|value| {
        parse_date(value).ok_or_else(|| {
          js_sys::Error::new("invalid creationDate: expected YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS")
        })
      })
      .transpose()?;

    Ok(Self {
      title: input.title,
      description: input.description,
      authors: input.authors.unwrap_or_default(),
      keywords: input.keywords.unwrap_or_default(),
      creator: input.creator,
      creation_date,
    })
  }
}

fn decode_images(
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
fn resolve_geometry(
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

/// The size returned by [`PdfRenderer::measure`].
#[derive(serde::Serialize)]
struct MeasuredSizeOutput {
  width: f32,
  height: f32,
}

/// A PDF renderer holding registered fonts and a decoded-resource cache.
///
/// State lives behind a lock and every method takes `&self`, mirroring the
/// other wasm bindings: a panic mid-call can't leave the wasm-bindgen borrow
/// flag permanently set.
#[wasm_bindgen]
pub struct PdfRenderer {
  state: RwLock<Fonts>,
  resource_cache: ResourceCache,
}

#[wasm_bindgen]
impl PdfRenderer {
  /// Creates a renderer with the bundled last-resort fonts.
  #[wasm_bindgen(constructor)]
  pub fn new() -> Result<PdfRenderer, js_sys::Error> {
    Ok(PdfRenderer {
      state: RwLock::new(default_fonts().map_err(map_error)?),
      resource_cache: ResourceCache::default(),
    })
  }

  /// Registers a font (raw bytes or a details object), returning the families
  /// it produced.
  #[wasm_bindgen(js_name = registerFont)]
  pub fn register_font(&self, font: FontType) -> Result<RegisteredFamiliesType, js_sys::Error> {
    let font: Font = from_value(font.into()).map_err(map_error)?;
    let mut state = self
      .state
      .try_write()
      .map_err(|error| js_sys::Error::new(&format!("Renderer state is locked: {error}")))?;

    let registered = match font {
      Font::Buffer(buffer) => state
        .register(FontResource::new(buffer.into_vec()))
        .map_err(map_error)?,
      Font::Object(details) => {
        let data = details.data.into_vec();
        let resource = build_font_resource(
          &data,
          details.name,
          details.weight.map(|weight| weight as f32),
          details.style.map(|style| style.0),
          details.subset_of,
          details.generic,
        )
        .map_err(map_error)?;

        state.register(resource).map_err(map_error)?
      }
    };

    Ok(to_value(&registered).map_err(map_error)?.unchecked_into())
  }

  /// Renders a node tree to PDF bytes. Without options the output is paged A4;
  /// `viewport` renders a single fixed page instead.
  #[wasm_bindgen(unchecked_return_type = "Uint8Array<ArrayBuffer>")]
  pub fn render(
    &self,
    node: NodeType,
    options: Option<PdfRenderOptionsType>,
  ) -> Result<Vec<u8>, js_sys::Error> {
    let node: Node = from_value(node.into()).map_err(map_error)?;
    let mut options: PdfRenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    let images = decode_images(&self.resource_cache, options.images.take())?;
    let (viewport, page) = resolve_geometry(&options)?;
    let lang = options
      .lang
      .as_deref()
      .map(Lang::parse)
      .transpose()
      .map_err(map_error)?;
    let state = self
      .state
      .try_read()
      .map_err(|error| js_sys::Error::new(&format!("Renderer state is locked: {error}")))?;

    takumi_pdf::render(PdfOptions {
      viewport,
      fonts: &state,
      node,
      stylesheet: stylesheet(&self.resource_cache, options.stylesheets, Vec::new()),
      images,
      page,
      header: options.header,
      footer: options.footer,
      font_families: options.font_families.map(FontFamily::from_names),
      lang,
      metadata: options.metadata.map(PdfMetadata::try_from).transpose()?,
      outline: options.outline.unwrap_or(false),
      standard: options.pdfa.map(PdfStandard::from).unwrap_or_default(),
      tagged: options.tagged.map(Tagging::from).unwrap_or_default(),
    })
    .map_err(map_error)
  }

  /// Lays out a node tree without rendering and returns its size in CSS px.
  /// Page options lay out at the full page width, like a header/footer band;
  /// `pageNumber` / `totalPages` hooks are filled with three-digit counters.
  #[wasm_bindgen]
  pub fn measure(
    &self,
    node: NodeType,
    options: Option<MeasureOptionsType>,
  ) -> Result<MeasuredSizeType, js_sys::Error> {
    let node: Node = from_value(node.into()).map_err(map_error)?;
    let mut options: PdfRenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();
    let images = decode_images(&self.resource_cache, options.images.take())?;
    let (viewport, page) = resolve_geometry(&options)?;
    let lang = options
      .lang
      .as_deref()
      .map(Lang::parse)
      .transpose()
      .map_err(map_error)?;
    let state = self
      .state
      .try_read()
      .map_err(|error| js_sys::Error::new(&format!("Renderer state is locked: {error}")))?;
    let measured = takumi_pdf::measure(MeasureOptions {
      viewport,
      fonts: &state,
      node,
      stylesheet: stylesheet(&self.resource_cache, options.stylesheets, Vec::new()),
      images,
      page,
      font_families: options.font_families.map(FontFamily::from_names),
      lang,
    })
    .map_err(map_error)?;

    Ok(
      to_value(&MeasuredSizeOutput {
        width: measured.width,
        height: measured.height,
      })
      .map_err(map_error)?
      .unchecked_into(),
    )
  }
}

#[cfg(test)]
mod tests {
  use super::parse_date;

  #[test]
  fn parse_date_accepts_documented_formats() {
    assert!(parse_date("2026-08-06").is_some());
    assert!(parse_date("2026-08-06T01:02:03").is_some());
    assert!(parse_date("2026-08-06T01:02:03Z").is_some());
    assert!(parse_date("2028-02-29").is_some());
  }

  #[test]
  fn parse_date_rejects_invalid_input() {
    assert!(parse_date("2026-08-06T01:02:03ZZ").is_none());
    assert!(parse_date("2026-02-30").is_none());
    assert!(parse_date("2026-13-01").is_none());
    assert!(parse_date("26-08-06").is_none());
  }
}
