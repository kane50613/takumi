//! WebAssembly bindings for takumi-pdf.

#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::{
  collections::HashMap,
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
    image::{ImageCacheMode, ResourceCache},
  },
  style::{FontFamily, FontStyle as CssFontStyle, FromCssStr, Lang},
  viewport::Viewport,
};
use takumi_pdf::{PageOptions, PdfOptions};
use wasm_bindgen::prelude::*;

fn map_error(error: impl core::fmt::Debug) -> js_sys::Error {
  js_sys::Error::new(&format!("{error:?}"))
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

/// A page size, like the CSS `@page` `size` descriptor: a keyword string
/// (`"a4"`, `"letter"`, optionally followed by `landscape`) or `[width, height]`
/// in CSS px.
#[derive(Deserialize)]
#[serde(untagged)]
enum SizeInput {
  Named(String),
  Dimensions([f32; 2]),
}

fn resolve_page(
  size: Option<&SizeInput>,
  margin: Option<f32>,
) -> Result<PageOptions, js_sys::Error> {
  let mut page = match size {
    None => PageOptions::A4,
    Some(SizeInput::Dimensions([width, height])) => PageOptions {
      width: *width,
      height: *height,
      ..PageOptions::A4
    },
    Some(SizeInput::Named(keywords)) => {
      let mut page = None;
      let mut landscape = false;

      for keyword in keywords.split_whitespace() {
        match keyword {
          "a4" => page = Some(PageOptions::A4),
          "letter" => page = Some(PageOptions::LETTER),
          "landscape" => landscape = true,
          "portrait" => {}
          other => {
            return Err(js_sys::Error::new(&format!("unknown page size: {other}")));
          }
        }
      }
      let page = page.unwrap_or(PageOptions::A4);

      if landscape { page.landscape() } else { page }
    }
  };

  if let Some(margin) = margin {
    page.margin = margin;
  }
  Ok(page)
}

/// Options for rendering a PDF.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PdfRenderOptions {
  /// Viewport width for single-page output.
  width: Option<u32>,
  /// Viewport height for single-page output.
  height: Option<u32>,
  /// Page size for paged output. Without `width`/`height` this defaults to A4.
  size: Option<SizeInput>,
  /// Uniform page margin in CSS px for paged output.
  margin: Option<f32>,
  /// Band repeated at the top of every page (`{page}`/`{pages}` in text).
  header: Option<Node>,
  /// Band repeated at the bottom of every page.
  footer: Option<Node>,
  /// Pre-fetched images keyed by URL.
  images: Option<Vec<ImageSource>>,
  /// CSS stylesheets to apply before layout.
  stylesheets: Option<Vec<String>>,
  /// Per-render font stack: ordered family names used as the fallback chain.
  font_families: Option<Vec<String>>,
  /// Default BCP-47 language tag applied to the root.
  lang: Option<String>,
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
  pub fn register_font(&self, font: JsValue) -> Result<JsValue, js_sys::Error> {
    let font: Font = from_value(font).map_err(map_error)?;
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

    to_value(&registered).map_err(map_error)
  }

  /// Renders a node tree to PDF bytes. Without options the output is paged A4;
  /// `width` + `height` without `size` renders a single fixed page.
  #[wasm_bindgen]
  pub fn render(
    &self,
    node: JsValue,
    options: Option<js_sys::Object>,
  ) -> Result<Vec<u8>, js_sys::Error> {
    let node: Node = from_value(node).map_err(map_error)?;
    let options: PdfRenderOptions = options
      .map(|options| from_value(options.into()).map_err(map_error))
      .transpose()?
      .unwrap_or_default();

    let mut images = HashMap::new();

    for source in options.images.unwrap_or_default() {
      let image = self
        .resource_cache
        .get_or_decode(&source.data, source.cache.unwrap_or_default())
        .map_err(map_error)?;

      images.insert(source.src, image);
    }

    let (viewport, page) = match (&options.size, options.width, options.height) {
      (None, Some(width), Some(height)) => (Some(Viewport::new((width, height))), None),
      (size, _, _) => (None, Some(resolve_page(size.as_ref(), options.margin)?)),
    };
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
    })
    .map_err(map_error)
  }
}
