//! The node, font, and image inputs a binding deserializes from JS before handing them to takumi.

use std::{
  collections::{BTreeSet, HashMap, HashSet},
  sync::Arc,
};

use serde::{Deserialize, Deserializer, de::Error as DeError};
use serde_bytes::ByteBuf;
use takumi_core::{
  Fonts,
  layout::node::Node,
  resources::{
    font::{FontError, FontOverride, FontResource, FontSource, PendingFontFace, RegisteredFamily},
    image::{ImageCacheMode, ImageError, ImageSource as DecodedImage, ResourceCache, is_svg_bytes},
    unicode_range::UnicodeRange,
  },
  style::{FontStyle as CssFontStyle, FromCssStr},
};

use crate::font_cache::FontCache;

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

/// A font a render's `fonts` list names, described before its bytes arrive.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListedFont {
  /// The key it registers under.
  key: String,
  name: Option<String>,
  weight: Option<f64>,
  style: Option<FontStyle>,
  unicode_range: Option<Vec<(u32, u32)>>,
}

/// A render's `fonts` list as a needs pass reads it.
pub struct ListedFonts {
  pending: Vec<PendingFontFace>,
  /// The families a render falls back through, in order.
  /// Approximate: an unnamed font not held yet, such as a URL, is left out.
  pub families: Option<Vec<String>>,
  len: usize,
  /// The listed `unicodeRange` subsets still loading.
  subsets: Vec<usize>,
}

impl ListedFonts {
  /// `listed` read against `cache`, which knows the fonts a renderer still holds.
  pub fn read(cache: &FontCache, listed: Vec<ListedFont>) -> Self {
    let len = listed.len();
    let mut pending = Vec::new();
    let mut families: Vec<String> = Vec::new();
    let mut subsets = Vec::new();

    for (id, font) in listed.into_iter().enumerate() {
      if let Some(held) = cache.lookup(&font.key) {
        families.extend(held.into_iter().map(|family| family.name));
        continue;
      }

      let Some(name) = font.name.filter(|name| !name.is_empty()) else {
        continue;
      };
      let mut face = PendingFontFace::new(id, name.as_str());

      families.push(name);

      if let Some(weight) = font.weight {
        face = face.weight(weight as f32, weight as f32);
      }

      if let Some(FontStyle(style)) = font.style {
        face = face.style(style);
      }

      if let Some(ranges) = font.unicode_range.filter(|ranges| !ranges.is_empty()) {
        subsets.push(id);
        face = face.unicode_range(
          ranges
            .into_iter()
            .map(|(from, to)| UnicodeRange::new(from, to))
            .collect(),
        );
      }

      pending.push(face);
    }

    let mut seen = HashSet::new();

    families.retain(|family| seen.insert(family.clone()));

    Self {
      pending,
      families: (!families.is_empty()).then_some(families),
      len,
      subsets,
    }
  }

  /// `fonts` with every listed font still loading registered as a pending face.
  pub fn fonts(&self, fonts: &Fonts) -> Fonts {
    let mut fonts = fonts.clone();

    for face in &self.pending {
      fonts.register_pending(face.clone());
    }

    fonts
  }

  /// The ids of the listed fonts a render loads, without the `unicodeRange` subsets it does not reach.
  /// <https://drafts.csswg.org/css-fonts-4/#unicode-range-desc>
  pub fn to_load(&self, faces: &BTreeSet<usize>) -> Vec<usize> {
    (0..self.len)
      .filter(|id| !self.subsets.contains(id) || faces.contains(id))
      .collect()
  }
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
  /// Deprecated: the codepoint ranges in place of `unicodeRange`.
  ranges: Option<Vec<(u32, u32)>>,
  /// Deprecated: the family to register under in place of `name`.
  subset_of: Option<String>,
  /// CSS generic family keyword this font resolves for.
  generic: Option<String>,
}

impl FontOptions {
  /// A font resource over `bytes` with these overrides applied.
  pub fn resource<'a>(
    &self,
    bytes: impl Into<FontSource<'a>>,
  ) -> Result<FontResource<'a>, FontError> {
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
      .unicode_range(self.served_ranges().collect());

    match &self.generic {
      Some(generic) => Ok(resource.generic_family(generic.parse()?)),
      None => Ok(resource),
    }
  }

  /// The codepoints the font serves, from `unicodeRange` or else the deprecated `ranges`.
  fn served_ranges(&self) -> impl Iterator<Item = UnicodeRange> + '_ {
    self
      .unicode_range
      .as_ref()
      .or(self.ranges.as_ref())
      .into_iter()
      .flatten()
      .map(|&(from, to)| UnicodeRange::new(from, to))
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

/// Keeps the SVG images a needs pass reads, whose `text` elements the fonts draw.
pub fn retain_svg(images: &mut Option<Vec<ImageSource>>) {
  if let Some(images) = images {
    images.retain(|image| is_svg_bytes(&image.data));
  }
}

/// Registers a font input, returning the families it produced and the bytes of font data it holds.
pub fn register_font(
  fonts: &mut Fonts,
  font: Font,
) -> Result<(Vec<RegisteredFamily>, u64), FontError> {
  match font {
    Font::Buffer(buffer) => register_resource(fonts, FontResource::new(buffer.into_vec())),
    Font::Object(details) => {
      register_resource(fonts, details.options.resource(details.data.as_slice())?)
    }
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

  fn served_ranges(json: &str) -> Vec<UnicodeRange> {
    let options: FontOptions = serde_json::from_str(json).unwrap();

    options.served_ranges().collect()
  }

  #[test]
  fn reads_the_deprecated_ranges_when_unicode_range_is_absent() {
    assert_eq!(
      served_ranges(r#"{ "ranges": [[65, 90]] }"#),
      [UnicodeRange::new(65, 90)]
    );
    assert_eq!(
      served_ranges(r#"{ "unicodeRange": [[48, 57]], "ranges": [[65, 90]] }"#),
      [UnicodeRange::new(48, 57)]
    );
  }
}
