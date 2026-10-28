//! A face whose glyphs are images, one per grapheme cluster, such as the emoji SVGs a caller
//! fetched from a CDN.
//!
//! Not Blink's: a browser draws emoji from a color font. Matching takes the face as a color face of
//! its family, which system fallback reaches before every other family. It holds a synthesized
//! sfnt, so metrics, and the backends that read a run's font file, take it as any registered
//! face, each glyph an empty outline a font size wide. Shaping maps each grapheme cluster of an
//! emoji presentation run that has an image to its glyph instead of running HarfBuzz, and leaves
//! every other cluster `.notdef` for the next font. Painting draws the cluster's image over the
//! glyph's em box.
//!
//! Approximate: every face takes Twemoji Mozilla's vertical metrics, whatever its images draw.

use std::{
  collections::hash_map::Entry,
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
};

use rustc_hash::{FxHashMap, FxHashSet};
use xxhash_rust::xxh3::xxh3_64_with_seed;

use crate::{
  context::RenderContext,
  geometry::Placement,
  resources::{
    glyph::{ResolvedBitmapGlyph, ResolvedGlyph, ResolvedGlyphs},
    image::{ImageSource, RenderedImage},
  },
  style::ImageScalingAlgorithm,
};

/// The design units of an em.
const UNITS_PER_EM: u16 = 512;

/// Twemoji Mozilla's `hhea` ascender, descender, and line gap, in [`UNITS_PER_EM`].
const HHEA: (i16, i16, i16) = (477, -91, 46);

/// Twemoji Mozilla's `OS/2` typographic ascender and descender, the top and bottom of the em box
/// its glyphs draw in, in [`UNITS_PER_EM`].
const TYPO_ASCENDER: i16 = 448;
const TYPO_DESCENDER: i16 = -64;

/// Twemoji Mozilla's `post` underline position and thickness, in [`UNITS_PER_EM`].
const UNDERLINE: (i16, i16) = (-37, 25);

/// The images of a face, by glyph: glyph `i + 1` draws the image at `i`, and glyph 0 is
/// `.notdef`.
pub(crate) struct ImageGlyphs {
  family: Arc<str>,
  glyphs: Vec<(Box<str>, ImageSource)>,
  glyph_ids: FxHashMap<Box<str>, u32>,
  /// The first character of every cluster, which matching takes as the characters the face has.
  first_characters: FxHashSet<char>,
}

impl ImageGlyphs {
  /// `family`'s images for the clusters of `glyphs`, the first image of a cluster winning.
  pub(crate) fn new(family: Arc<str>, glyphs: Vec<(Box<str>, ImageSource)>) -> Self {
    let mut unique: Vec<(Box<str>, ImageSource)> = Vec::with_capacity(glyphs.len());
    let mut glyph_ids = FxHashMap::default();

    for (cluster, image) in glyphs {
      if cluster.is_empty() || glyph_ids.contains_key(&cluster) {
        continue;
      }

      glyph_ids.insert(cluster.clone(), unique.len() as u32 + 1);
      unique.push((cluster, image));
    }

    let first_characters = unique
      .iter()
      .filter_map(|(cluster, _)| cluster.chars().next())
      .collect();

    Self {
      family,
      glyphs: unique,
      glyph_ids,
      first_characters,
    }
  }

  /// The family the face registered under.
  pub(crate) fn family(&self) -> &Arc<str> {
    &self.family
  }

  /// The glyph that draws `cluster`, none when the face has no image for it.
  pub(crate) fn glyph(&self, cluster: &str) -> Option<u32> {
    self.glyph_ids.get(cluster).copied()
  }

  /// The image glyph `glyph_id` draws.
  pub(crate) fn image(&self, glyph_id: u32) -> Option<&ImageSource> {
    let index = usize::try_from(glyph_id).ok()?.checked_sub(1)?;

    self.glyphs.get(index).map(|(_, image)| image)
  }

  /// Whether a cluster the face has an image for starts with `character`.
  pub(crate) fn has_glyph(&self, character: char) -> bool {
    self.first_characters.contains(&character)
  }

  /// Adds the glyphs among `glyph_ids` that `resolved` lacks, each its image over the em box of
  /// a `font_size` font, drawn at `host`'s time and color.
  pub(crate) fn resolve_glyphs_into(
    &self,
    resolved: &mut ResolvedGlyphs,
    font_size: f32,
    glyph_ids: impl Iterator<Item = u32>,
    host: &RenderContext,
  ) {
    for glyph_id in glyph_ids {
      if let Entry::Vacant(slot) = resolved.entry(glyph_id)
        && let Some(glyph) = self
          .image(glyph_id)
          .and_then(|image| resolve_image_glyph(image, font_size, host))
      {
        slot.insert(Arc::new(glyph));
      }
    }
  }

  /// The face's sfnt and the id glyph caches key it by, unique to this face.
  pub(crate) fn sfnt_with_id(&self) -> (Vec<u8>, u64) {
    static REGISTRATION: AtomicU64 = AtomicU64::new(1);

    let sfnt = sfnt(self.glyphs.len() + 1);
    let id = xxh3_64_with_seed(&sfnt, REGISTRATION.fetch_add(1, Ordering::Relaxed));

    (sfnt, id)
  }
}

/// `image` as a bitmap glyph filling the em box of a `font_size` font: an em wide, its top
/// [`TYPO_ASCENDER`] above the baseline. An SVG rasterizes at the em size, and a bitmap scales as
/// it is.
fn resolve_image_glyph(
  image: &ImageSource,
  font_size: f32,
  host: &RenderContext,
) -> Option<ResolvedGlyph> {
  let size = font_size.ceil().max(1.0) as u32;

  if !image.is_drawable_at(size, size) {
    return None;
  }

  let bitmap = match image
    .render_for_layout(
      size,
      size,
      ImageScalingAlgorithm::Auto,
      host.time_ms(),
      host,
    )
    .ok()?
  {
    RenderedImage::Rasterized(bitmap) => bitmap,
    RenderedImage::Sampled { source, .. } => source,
  };
  let (width, height) = (bitmap.width(), bitmap.height());

  if width == 0 || height == 0 {
    return None;
  }

  let em_top = font_size * f32::from(TYPO_ASCENDER) / f32::from(UNITS_PER_EM);

  Some(ResolvedGlyph::Bitmap(ResolvedBitmapGlyph {
    image: (*bitmap).clone(),
    scale_x: font_size / width as f32,
    scale_y: font_size / height as f32,
    placement: Placement {
      left: 0,
      top: em_top.round() as i32,
      width: size,
      height: size,
    },
  }))
}

/// A TrueType font of `glyph_count` empty glyphs, each an em wide, with Twemoji Mozilla's
/// metrics and no character map entries.
fn sfnt(glyph_count: usize) -> Vec<u8> {
  let glyph_count = u16::try_from(glyph_count).unwrap_or(u16::MAX);
  let em = UNITS_PER_EM as i16;
  let (ascender, descender, line_gap) = HHEA;
  let mut hmtx = Writer::default();
  let mut loca = Writer::default().u16(0);

  for _ in 0..glyph_count {
    hmtx = hmtx.u16(UNITS_PER_EM).i16(0);
    loca = loca.u16(0);
  }

  font_file(vec![
    // https://learn.microsoft.com/en-us/typography/opentype/spec/cmap: a format 12 subtable
    // without groups.
    (
      *b"cmap",
      Writer::default()
        .u16(0)
        .u16(1)
        .u16(3)
        .u16(10)
        .u32(12)
        .u16(12)
        .u16(0)
        .u32(16)
        .u32(0)
        .u32(0)
        .finish(),
    ),
    // https://learn.microsoft.com/en-us/typography/opentype/spec/glyf: every glyph is empty.
    (*b"glyf", Vec::new()),
    // https://learn.microsoft.com/en-us/typography/opentype/spec/head
    (
      *b"head",
      Writer::default()
        .u32(0x0001_0000)
        .u32(0x0001_0000)
        .u32(0)
        .u32(0x5F0F_3CF5)
        .u16(0x000B)
        .u16(UNITS_PER_EM)
        .u64(0)
        .u64(0)
        .i16(0)
        .i16(TYPO_DESCENDER)
        .i16(em)
        .i16(TYPO_ASCENDER)
        .u16(0)
        .u16(8)
        .i16(2)
        .i16(0)
        .i16(0)
        .finish(),
    ),
    // https://learn.microsoft.com/en-us/typography/opentype/spec/hhea
    (
      *b"hhea",
      Writer::default()
        .u32(0x0001_0000)
        .i16(ascender)
        .i16(descender)
        .i16(line_gap)
        .u16(UNITS_PER_EM)
        .i16(0)
        .i16(0)
        .i16(em)
        .i16(1)
        .i16(0)
        .i16(0)
        .u64(0)
        .i16(0)
        .u16(glyph_count)
        .finish(),
    ),
    (*b"hmtx", hmtx.finish()),
    (*b"loca", loca.finish()),
    // https://learn.microsoft.com/en-us/typography/opentype/spec/maxp, version 1.0, with one
    // zone.
    (
      *b"maxp",
      Writer::default()
        .u32(0x0001_0000)
        .u16(glyph_count)
        .bytes(&[0; 8])
        .u16(1)
        .bytes(&[0; 16])
        .finish(),
    ),
    // https://learn.microsoft.com/en-us/typography/opentype/spec/os2, version 4, regular, with
    // `USE_TYPO_METRICS` as Twemoji Mozilla sets it.
    (
      *b"OS/2",
      Writer::default()
        .u16(4)
        .i16(em)
        .u16(400)
        .u16(5)
        .u16(0)
        .bytes(&[0; 16])
        .i16(UNDERLINE.1)
        .i16(em / 4)
        .i16(0)
        .bytes(&[0; 26])
        .bytes(b"    ")
        .u16(0x00C0)
        .u16(0)
        .u16(0xFFFF)
        .i16(TYPO_ASCENDER)
        .i16(TYPO_DESCENDER)
        .i16(line_gap)
        .u16(ascender.unsigned_abs())
        .u16(descender.unsigned_abs())
        .u64(0)
        .i16(0)
        .i16(0)
        .u16(0)
        .u16(0x20)
        .u16(1)
        .finish(),
    ),
    // https://learn.microsoft.com/en-us/typography/opentype/spec/post, version 3.0.
    (
      *b"post",
      Writer::default()
        .u32(0x0003_0000)
        .u32(0)
        .i16(UNDERLINE.0)
        .i16(UNDERLINE.1)
        .u32(0)
        .u64(0)
        .u64(0)
        .finish(),
    ),
  ])
}

/// The sfnt holding `tables`, sorted by tag, each starting on a four-byte boundary.
/// <https://learn.microsoft.com/en-us/typography/opentype/spec/otff#organization-of-an-opentype-font>
fn font_file(mut tables: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
  tables.sort_by_key(|(tag, _)| *tag);

  let count = tables.len() as u16;
  let entry_selector = 15 - count.leading_zeros() as u16;
  let search_range = (1 << entry_selector) * 16;
  let mut header = Writer::default()
    .u32(0x0001_0000)
    .u16(count)
    .u16(search_range)
    .u16(entry_selector)
    .u16(count * 16 - search_range);
  let mut offset = 12 + 16 * tables.len();
  let mut body = Vec::new();

  for (tag, data) in &tables {
    header = header
      .bytes(tag)
      .u32(checksum(data))
      .u32(offset as u32)
      .u32(data.len() as u32);
    body.extend_from_slice(data);
    body.resize(body.len().next_multiple_of(4), 0);
    offset = 12 + 16 * tables.len() + body.len();
  }

  let mut file = header.finish();

  file.extend(body);
  file
}

/// The sum of a table's big-endian `uint32`s, its last one padded with zeros.
fn checksum(data: &[u8]) -> u32 {
  data.chunks(4).fold(0u32, |sum, chunk| {
    let mut word = [0; 4];

    word[..chunk.len()].copy_from_slice(chunk);
    sum.wrapping_add(u32::from_be_bytes(word))
  })
}

/// Big-endian table data.
#[derive(Default)]
struct Writer(Vec<u8>);

impl Writer {
  fn bytes(mut self, bytes: &[u8]) -> Self {
    self.0.extend_from_slice(bytes);
    self
  }

  fn u16(self, value: u16) -> Self {
    self.bytes(&value.to_be_bytes())
  }

  fn i16(self, value: i16) -> Self {
    self.bytes(&value.to_be_bytes())
  }

  fn u32(self, value: u32) -> Self {
    self.bytes(&value.to_be_bytes())
  }

  fn u64(self, value: u64) -> Self {
    self.bytes(&value.to_be_bytes())
  }

  fn finish(self) -> Vec<u8> {
    self.0
  }
}

#[cfg(test)]
mod tests {
  use skrifa::{
    FontRef, GlyphId, MetadataProvider,
    instance::{LocationRef, Size},
  };

  use super::*;
  use crate::{
    fonts::shaping::{
      harfbuzz_shaper::HarfBuzzShaper,
      testing::{GEIST, font, register},
    },
    resources::{
      font::{Fonts, ImageFontFace},
      image_buffer::ImageBuffer,
    },
    style::Direction,
  };

  /// A one-pixel image.
  fn pixel() -> ImageSource {
    ImageSource::from(ImageBuffer::from_rgba_bytes(vec![0, 0, 0, 255], 1, 1).unwrap())
  }

  /// Each run of `text` in Geist with an image face for 😀, the family 👨‍👩‍👧, and ❤: its
  /// text, whether the image face draws it, and its glyph ids.
  fn runs(text: &str) -> Vec<(&str, bool, Vec<u16>)> {
    let mut fonts = Fonts::default();

    register(&mut fonts, GEIST, "Geist");
    fonts.register_image_face(
      ImageFontFace::new("Emoji Images")
        .glyph("😀", pixel())
        .glyph("👨‍👩‍👧", pixel())
        .glyph("❤", pixel()),
    );

    HarfBuzzShaper::new(text)
      .shape_all(&font(&fonts, "Geist"), Direction::Ltr)
      .runs
      .iter()
      .map(|run| {
        let start = run.start_index as usize;

        (
          &text[start..start + run.num_characters as usize],
          run
            .font_data
            .as_ref()
            .is_some_and(|font_data| font_data.face.typeface.image_glyphs.is_some()),
          run
            .glyph_data
            .glyphs()
            .iter()
            .map(|glyph| glyph.glyph)
            .collect(),
        )
      })
      .collect()
  }

  #[test]
  fn draws_each_emoji_cluster_with_an_image_as_one_glyph() {
    assert_eq!(
      runs("a😀b👨‍👩‍👧"),
      [
        ("a", false, vec![runs("a")[0].2[0]]),
        ("😀", true, vec![1]),
        ("b", false, vec![runs("b")[0].2[0]]),
        ("👨‍👩‍👧", true, vec![2]),
      ]
    );
  }

  #[test]
  fn leaves_text_presentation_and_clusters_without_an_image_to_other_fonts() {
    assert!(runs("❤ 🚀").iter().all(|(_, image, _)| !image));
  }

  #[test]
  fn synthesizes_a_font_skrifa_reads() {
    let data = sfnt(3);
    let font = FontRef::new(&data).unwrap();
    let metrics = font.metrics(Size::new(512.0), LocationRef::default());
    let glyph_metrics = font.glyph_metrics(Size::new(512.0), LocationRef::default());

    assert_eq!(metrics.units_per_em, UNITS_PER_EM);
    assert_eq!(metrics.glyph_count, 3);
    assert_eq!(metrics.ascent, 448.0);
    assert_eq!(metrics.descent, -64.0);
    assert_eq!(metrics.leading, 46.0);
    assert_eq!(glyph_metrics.advance_width(GlyphId::new(2)), Some(512.0));
    assert_eq!(font.charmap().map('a'), None);
    assert!(font.outline_glyphs().get(GlyphId::new(1)).is_some());
  }
}
