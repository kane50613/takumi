//! The font and image inputs a binding deserializes from JS before handing them to takumi.

use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Deserializer, de::Error as DeError};
use serde_bytes::ByteBuf;
use takumi_core::{
  Fonts,
  resources::{
    font::{FontError, FontResource, RegisteredFamily},
    image::{ImageCacheMode, ImageError, ImageSource as DecodedImage, ResourceCache},
  },
  style::{FontStyle as CssFontStyle, FromCssStr},
};

use crate::build_font_resource;

/// Details for loading a custom font.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontDetails {
  /// The family name to register the font under.
  pub name: Option<String>,
  /// The raw font bytes.
  pub data: ByteBuf,
  /// The font weight, e.g. 400 or 700.
  pub weight: Option<f64>,
  /// The font style.
  pub style: Option<FontStyle>,
  /// Logical family this font is a coverage subset of.
  pub subset_of: Option<String>,
  /// Where this subset sits in its group's fallback order.
  pub subset_rank: Option<u32>,
  /// CSS generic family keyword this font resolves for.
  pub generic: Option<String>,
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
pub struct FontStyle(pub CssFontStyle);

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

impl From<FontStyle> for CssFontStyle {
  fn from(style: FontStyle) -> Self {
    style.0
  }
}

/// An image keyed by the `src` nodes reference it with.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSource {
  /// The source URL of the image.
  pub src: Arc<str>,
  /// The raw image bytes.
  pub data: ByteBuf,
  /// Cache policy for the decoded image. Defaults to `"auto"`.
  pub cache: Option<ImageCacheMode>,
}

/// Registers a font input, returning the families it produced.
pub fn register_font(fonts: &mut Fonts, font: Font) -> Result<Vec<RegisteredFamily>, FontError> {
  match font {
    Font::Buffer(buffer) => fonts.register(FontResource::new(buffer.into_vec())),
    Font::Object(details) => {
      let data = details.data.into_vec();
      let resource = build_font_resource(
        &data,
        details.name,
        details.weight.map(|weight| weight as f32),
        details.style.map(Into::into),
        details.subset_of,
        details.subset_rank,
        details.generic,
      )?;
      fonts.register(resource)
    }
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
