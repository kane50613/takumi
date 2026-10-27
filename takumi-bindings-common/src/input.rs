//! The font and image inputs a binding deserializes from JS before handing them to takumi.

use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Deserializer, de::Error as DeError};
use serde_bytes::ByteBuf;
use takumi_core::{
  Fonts,
  resources::{
    font::{FontError, FontOverride, FontResource, RegisteredFamily},
    image::{ImageCacheMode, ImageError, ImageSource as DecodedImage, ResourceCache},
    unicode_range::UnicodeRange,
  },
  style::{FontStyle as CssFontStyle, FromCssStr},
};
use xxhash_rust::xxh3::Xxh3;

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
  /// A hash of `bytes` registered with these options, equal for the same file registered the
  /// same way.
  pub fn content_hash(&self, bytes: &[u8]) -> u64 {
    let mut hasher = Xxh3::new();

    hasher.update(bytes);

    for field in [
      self.name.as_deref(),
      self.subset_of.as_deref(),
      self.style.as_ref().map(|style| &*style.css),
      self.generic.as_deref(),
    ] {
      hasher.update(&[u8::from(field.is_some())]);
      hasher.update(field.unwrap_or_default().as_bytes());
      hasher.update(&[0xff]);
    }

    hasher.update(&self.weight.map_or(u64::MAX, f64::to_bits).to_le_bytes());

    for &(from, to) in self.unicode_range.iter().flatten() {
      hasher.update(&from.to_le_bytes());
      hasher.update(&to.to_le_bytes());
    }

    hasher.digest()
  }

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
        style: self.style.as_ref().map(|style| style.parsed),
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

impl Font {
  /// See [`FontOptions::content_hash`].
  pub fn content_hash(&self) -> u64 {
    match self {
      Self::Object(details) => details.options.content_hash(&details.data),
      Self::Buffer(buffer) => FontOptions::default().content_hash(buffer),
    }
  }
}

/// A `font-style` value parsed from its CSS text, which it keeps for [`FontOptions::content_hash`].
#[derive(Clone)]
struct FontStyle {
  css: Box<str>,
  parsed: CssFontStyle,
}

impl<'de> Deserialize<'de> for FontStyle {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let css = String::deserialize(deserializer)?;
    let parsed = CssFontStyle::from_css_str(&css).map_err(D::Error::custom)?;

    Ok(Self {
      css: css.into(),
      parsed,
    })
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn content_hash_tells_registrations_apart() {
    let bytes = b"font";
    let named = |name: &str| FontOptions {
      name: Some(name.into()),
      ..FontOptions::default()
    };

    assert_eq!(
      named("A").content_hash(bytes),
      named("A").content_hash(bytes)
    );
    assert_ne!(
      named("A").content_hash(bytes),
      named("B").content_hash(bytes)
    );
    assert_ne!(
      named("A").content_hash(bytes),
      named("A").content_hash(b"other")
    );
    assert_ne!(
      FontOptions::default().content_hash(bytes),
      FontOptions {
        weight: Some(700.0),
        ..FontOptions::default()
      }
      .content_hash(bytes)
    );
  }
}
