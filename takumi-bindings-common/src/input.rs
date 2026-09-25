//! The font and image inputs a binding deserializes from JS before handing them to takumi.

use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Deserializer, de::Error as DeError};
use serde_bytes::ByteBuf;
use takumi_core::{
  Fonts,
  resources::{
    font::{FontError, FontOverride, FontResource, RegisteredFamily},
    image::{ImageCacheMode, ImageError, ImageSource as DecodedImage, ResourceCache},
  },
  style::{FontStyle as CssFontStyle, FromCssStr},
};

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
  /// Logical family this font is a coverage subset of.
  subset_of: Option<String>,
  /// Where this subset sits in its group's fallback order.
  subset_rank: Option<u32>,
  /// CSS generic family keyword this font resolves for.
  generic: Option<String>,
}

impl FontOptions {
  /// A font resource over `bytes` with these overrides applied.
  pub fn resource<'a>(&self, bytes: &'a [u8]) -> Result<FontResource<'a>, FontError> {
    let resource = FontResource::new(bytes).override_info(FontOverride {
      family_name: self.name.clone().map(Into::into),
      weight: self.weight.map(|weight| weight as f32),
      style: self.style.map(|style| style.0),
      ..Default::default()
    });

    let resource = match &self.subset_of {
      Some(logical) => resource
        .subset_of(logical.clone())
        .subset_rank(self.subset_rank.unwrap_or_default()),
      None => resource,
    };

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

/// Registers a font input, returning the families it produced.
pub fn register_font(fonts: &mut Fonts, font: Font) -> Result<Vec<RegisteredFamily>, FontError> {
  match font {
    Font::Buffer(buffer) => fonts.register(FontResource::new(buffer.into_vec())),
    Font::Object(details) => fonts.register(details.options.resource(&details.data)?),
  }
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
