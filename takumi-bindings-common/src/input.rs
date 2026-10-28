//! The node, font, and image inputs a binding deserializes from JS before handing them to takumi.

#[cfg(feature = "wasm")]
use std::rc::Rc;
use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Deserializer, de::Error as DeError};
use serde_bytes::ByteBuf;
#[cfg(feature = "wasm")]
use serde_wasm_bindgen::from_value;
use takumi_core::{
  Fonts,
  layout::node::Node,
  resources::{
    font::{FontError, FontOverride, FontResource, PendingFontFace, RegisteredFamily},
    image::{ImageCacheMode, ImageError, ImageSource as DecodedImage, ResourceCache},
    unicode_range::UnicodeRange,
  },
  style::{FontStyle as CssFontStyle, FromCssStr},
};
#[cfg(feature = "wasm")]
use wasm_bindgen::JsValue;

/// The node tree in `json`, failing with the message reading the tree from an object reports,
/// which carries no line and column.
pub fn node_from_json(json: &str) -> Result<Node, String> {
  serde_json::from_str(json).map_err(|error| {
    let message = error.to_string();
    let position = format!(" at line {} column {}", error.line(), error.column());

    message
      .strip_suffix(&position)
      .map_or(message.clone(), str::to_owned)
  })
}

/// A font a render may load, described as `fonts` describes it before its bytes arrive, which a
/// needs pass reports by `id` when the render would load it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingFont {
  id: usize,
  name: String,
  weight: Option<f64>,
  style: Option<FontStyle>,
  unicode_range: Option<Vec<(u32, u32)>>,
}

impl PendingFont {
  /// The face font matching takes this font for while it loads.
  fn face(self) -> PendingFontFace {
    let mut face = PendingFontFace::new(self.id, self.name);

    if let Some(weight) = self.weight {
      face = face.weight(weight as f32, weight as f32);
    }

    if let Some(FontStyle(style)) = self.style {
      face = face.style(style);
    }

    if let Some(ranges) = self.unicode_range {
      face = face.unicode_range(
        ranges
          .into_iter()
          .map(|(from, to)| UnicodeRange::new(from, to))
          .collect(),
      );
    }

    face
  }
}

/// `fonts` with `pending` registered as faces still loading.
pub fn with_pending(fonts: &Fonts, pending: Vec<PendingFont>) -> Fonts {
  let mut fonts = fonts.clone();

  for font in pending {
    fonts.register_pending(font.face());
  }

  fonts
}

/// `fonts` with the pending fonts a wasm binding's `pending` list describes, or `fonts` itself
/// without one.
#[cfg(feature = "wasm")]
pub fn with_pending_value(
  fonts: Rc<Fonts>,
  pending: Option<JsValue>,
) -> Result<Rc<Fonts>, js_sys::Error> {
  let Some(pending) = pending else {
    return Ok(fonts);
  };
  let pending: Vec<PendingFont> =
    from_value(pending).map_err(|error| js_sys::Error::new(&error.to_string()))?;

  Ok(Rc::new(with_pending(&fonts, pending)))
}

/// How to register a font's bytes, each field overriding what the file says.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FontOptions {
  /// The family name to register the font under.
  name: Option<String>,
  /// The font weight, e.g. 400 or 700.
  weight: Option<f64>,
  /// The font style.
  style: Option<FontStyle>,
  /// Inclusive codepoint ranges the font serves, as `@font-face` `unicode-range`.
  unicode_range: Option<Vec<(u32, u32)>>,
  /// Deprecated: the family to register under in place of `name`.
  subset_of: Option<String>,
  /// CSS generic family keyword this font resolves for.
  generic: Option<String>,
}

impl FontOptions {
  /// A font resource over `bytes` with these overrides applied.
  pub fn resource<'a>(&self, bytes: &'a [u8]) -> Result<FontResource<'a>, FontError> {
    let resource = FontResource::new(bytes)
      .override_info(FontOverride {
        family_name: self
          .subset_of
          .as_deref()
          .or(self.name.as_deref())
          .map(Into::into),
        weight: self.weight.map(|weight| weight as f32),
        style: self.style.map(|style| style.0),
        ..Default::default()
      })
      .unicode_range(
        self
          .unicode_range
          .iter()
          .flatten()
          .map(|&(from, to)| UnicodeRange::new(from, to))
          .collect(),
      );

    match &self.generic {
      Some(generic) => Ok(resource.generic_family(generic.parse()?)),
      None => Ok(resource),
    }
  }
}

/// Details for loading a custom font.
#[derive(Deserialize)]
pub struct FontDetails {
  /// The raw font bytes.
  data: ByteBuf,
  /// How to register the bytes.
  #[serde(flatten)]
  options: FontOptions,
}

/// Font input, either a details object or raw bytes.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Font {
  /// Font loaded with detailed configuration.
  Object(FontDetails),
  /// Raw font bytes.
  Buffer(ByteBuf),
}

/// A `font-style` value parsed from its CSS text.
#[derive(Clone, Copy)]
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

/// An image keyed by the `src` nodes reference it with.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSource {
  /// The source URL of the image.
  src: Arc<str>,
  /// The raw image bytes.
  data: ByteBuf,
  /// Cache policy for the decoded image. Defaults to `"auto"`.
  cache: Option<ImageCacheMode>,
}

/// Registers a font input, returning the families it produced and the bytes of font data it holds.
pub fn register_font(
  fonts: &mut Fonts,
  font: Font,
) -> Result<(Vec<RegisteredFamily>, u64), FontError> {
  match font {
    Font::Buffer(buffer) => register_resource(fonts, FontResource::new(buffer.into_vec())),
    Font::Object(details) => register_resource(fonts, details.options.resource(&details.data)?),
  }
}

/// Registers `resource` decoded, returning the families it produced and its decoded bytes.
pub fn register_resource(
  fonts: &mut Fonts,
  resource: FontResource<'_>,
) -> Result<(Vec<RegisteredFamily>, u64), FontError> {
  let resource = resource.into_resolved()?;
  let bytes = resource.byte_len() as u64;

  Ok((fonts.register(resource)?, bytes))
}

/// Decodes the per-render image inputs into a `src`-keyed map through `cache`.
pub fn decode_images(
  cache: &ResourceCache,
  sources: &[ImageSource],
) -> Result<HashMap<Arc<str>, DecodedImage>, ImageError> {
  sources
    .iter()
    .map(|source| {
      let image = cache.get_or_decode(&source.data, source.cache.unwrap_or_default())?;
      Ok((source.src.clone(), image))
    })
    .collect()
}
