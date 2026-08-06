use crate::subsetter::{GlyphRemapper, Tag as SubsetTag, subset_with_variations};
use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::writers::WMode;
use pdf_writer::{Finish, Name, Ref, Str};
use rustc_hash::FxHashMap;
use skrifa::instance::Size;
use skrifa::outline::DrawSettings;
use skrifa::prelude::LocationRef;
use skrifa::raw::tables::cff::Cff;
use skrifa::raw::{TableProvider, TopLevelTable};
use std::hash::Hash;
use std::ops::DerefMut;
use std::sync::Arc;

use super::{CIDIdentifier, FontIdentifier, PDF_UNITS_PER_EM};
use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::configure::ValidationError;
use crate::krilla::error::{KrillaError, KrillaResult};
use crate::krilla::geom::Rect;
use crate::krilla::serialize::SerializeContext;
use crate::krilla::stream::FilterStreamBuilder;
use crate::krilla::surface::Location;
use crate::krilla::text::Font;
use crate::krilla::text::GlyphId;
use crate::krilla::text::outline::OutlineBuilder;
use crate::krilla::util::{SliceExt, stable_hash128};

const SUBSET_TAG_LEN: usize = 6;
pub(crate) const IDENTITY_H: &str = "Identity-H";
pub(crate) const CMAP_NAME: Name = Name(b"Custom");
pub(crate) const SYSTEM_INFO: SystemInfo = SystemInfo {
  registry: Str(b"Adobe"),
  ordering: Str(b"Identity"),
  supplement: 0,
};

pub(crate) type Cid = u16;

/// A shared function for CID fonts and Type3 fonts to write the cmap entries.
pub(crate) fn write_cmap_entry<G>(
  font: &Font,
  entry: Option<&(String, Option<Location>)>,
  sc: &mut SerializeContext,
  cmap: &mut UnicodeCmap<G>,
  g: G,
) where
  G: pdf_writer::types::GlyphId + Into<u32> + Copy,
{
  match entry {
    None => sc.register_validation_error(ValidationError::NoCodepointMapping(
      font.clone(),
      GlyphId::new(g.into()),
      None,
    )),
    Some((text, loc)) => {
      let mut invalid_codepoint = text.is_empty();
      let mut invalid_code = None;
      let mut private_unicode = None;

      for c in text.chars() {
        if matches!(c as u32, 0x0 | 0xFEFF | 0xFFFE) {
          invalid_code = Some(c);
          invalid_codepoint = true;
        }

        if matches!(c as u32, 0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD) {
          private_unicode = Some(c);
        }
      }

      match invalid_code {
        Some(c) => sc.register_validation_error(ValidationError::InvalidCodepointMapping(
          font.clone(),
          GlyphId::new(g.into()),
          c,
          *loc,
        )),
        None if invalid_codepoint => sc.register_validation_error(
          ValidationError::NoCodepointMapping(font.clone(), GlyphId::new(g.into()), *loc),
        ),
        _ => {}
      }

      if let Some(code) = private_unicode {
        sc.register_validation_error(ValidationError::UnicodePrivateArea(
          font.clone(),
          GlyphId::new(g.into()),
          code,
          *loc,
        ));
      }

      if !text.is_empty() {
        cmap.pair_with_multiple(g, text.chars());
      }
    }
  }
}

/// A CID-keyed font.
#[derive(Debug)]
pub(crate) struct CIDFont {
  /// The _actual_ underlying OTF font of the CID-keyed font.
  font: Font,
  /// A mapper that maps GIDs from the original font to CIDs, i.e. the corresponding GID in the font
  /// subset. The subsetter will ensure that for CID-keyed CFF fonts, the CID-to-GID mapping
  /// will be the identity mapping, regardless of what the mapping was in the original font. This
  /// allows us to index both, CFF and glyf-based fonts, transparently using GIDs,
  /// instead of having to distinguish according to the underlying font. See section
  /// 9.7.4.2 for more information on how glyphs are indexed in a CID-keyed font.
  glyph_remapper: GlyphRemapper,
  /// A mapping from CIDs to their string in the original text.
  cmap_entries: FxHashMap<u16, (String, Option<Location>)>,
  /// The widths of the glyphs, _indexed by their CID_.
  widths: Vec<f32>,
  is_empty: bool,
}

impl CIDFont {
  /// Create a new CID-keyed font.
  pub(crate) fn new(font: Font) -> CIDFont {
    // Always include the .notdef glyph. Will also always be included by the subsetter in
    // the glyph remapper.
    let widths = vec![font.advance_width(GlyphId::new(0)).unwrap_or(0.0)];

    Self {
      glyph_remapper: GlyphRemapper::new(),
      cmap_entries: FxHashMap::default(),
      widths,
      font,
      is_empty: true,
    }
  }

  pub(crate) fn is_empty(&self) -> bool {
    self.is_empty
  }

  pub(crate) fn font(&self) -> Font {
    self.font.clone()
  }

  // Note that this refers to the units per em in PDF (which is always 1000), and not the
  // units per em of the underlying font.
  pub(crate) fn units_per_em(&self) -> f32 {
    PDF_UNITS_PER_EM
  }

  #[inline]
  pub(crate) fn get_cid(&self, glyph_id: GlyphId) -> Option<u16> {
    self.glyph_remapper.get(glyph_id.to_u32() as u16)
  }

  /// Add a new glyph (if it has not already been added) and return its CID.
  #[inline]
  pub(crate) fn add_glyph(&mut self, glyph_id: GlyphId) -> Cid {
    self.is_empty = false;

    let new_id = self
      .glyph_remapper
      .remap(u16::try_from(glyph_id.to_u32()).unwrap());

    // This means that the glyph ID has been newly assigned, and thus we need to add its width.
    if new_id as usize >= self.widths.len() {
      self
        .widths
        .push(self.font.advance_width(glyph_id).unwrap_or(0.0));
    }

    new_id
  }

  #[inline]
  pub(crate) fn get_codepoints(&self, cid: Cid) -> Option<&str> {
    self.cmap_entries.get(&cid).map(|s| s.0.as_str())
  }

  #[inline]
  pub(crate) fn set_codepoints(&mut self, cid: Cid, text: String, location: Option<Location>) {
    self.cmap_entries.insert(cid, (text, location));
  }

  #[inline]
  pub(crate) fn identifier(&self) -> FontIdentifier {
    FontIdentifier::Cid(CIDIdentifier(self.font.clone()))
  }

  pub(crate) fn serialize(
    &self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
  ) -> KrillaResult<()> {
    let chunk = &mut chunk_container.non_stream.fonts;
    let mut stream_chunk = sc.new_chunk();

    let cid_ref = sc.new_ref();
    let descriptor_ref = sc.new_ref();
    let cmap_ref = sc.new_ref();
    let cid_set_ref = sc.new_ref();
    let data_ref = sc.new_ref();

    let glyph_remapper = &self.glyph_remapper;

    let is_glyf = self.font.font_ref().glyf().is_ok();
    let is_cff = self.font.font_ref().cff().is_ok();
    let is_cff2 = self.font.font_ref().cff2().is_ok();

    if !is_glyf && !is_cff && !is_cff2 {
      return Err(KrillaError::Font(
        self.font.clone(),
        "font is missing an outline table".to_string(),
      ));
    }

    // OS/2 fsType has a few bits that describe which kind of license the font has. Of
    // particular interest is bit 2 "Restricted License embedding", which places restrictions on
    // font embedding that are incompatible with some PDF standards.
    //
    // The OpenType spec intended for the different bits to be mutually exclusive. However, some
    // fonts have multiple of the bits sets. The OpenType spec was thus adjusted to require
    // mutual exclusion, allowing applications to assume the least-restrictive specified
    // variant. Specifically, it also clarifies that "For Restricted License embedding to take
    // effect, the Embedding permissions sub-field must have the value 2 (that is, only bit 1 is
    // set)."
    //
    // Hence, we are not checking `fsType & 2 != 0` (as would be usual for bit flags),
    // but rather `fsType & 0xF == 2`.
    if self
      .font
      .font_ref()
      .os2()
      .is_ok_and(|os2| os2.fs_type() & 0xF == 2)
    {
      sc.register_validation_error(ValidationError::RestrictedLicense(self.font.clone()));
    }

    let (subsetted, global_bbox) = subset_font(self.font.clone(), glyph_remapper)?;
    let num_glyphs = subsetted.num_glyphs();
    let subsetted_data = subsetted.font_data().0;

    let font_stream = {
      let mut data = subsetted_data.as_ref().as_ref();

      // If we have a CFF font, only embed the standalone CFF program.
      let subsetted_ref = skrifa::FontRef::new(data).map_err(|_| {
        KrillaError::Font(self.font.clone(), "failed to read font subset".to_string())
      })?;

      if let Some(cff) = subsetted_ref.data_for_tag(Cff::TAG) {
        data = cff.as_bytes();
      }

      FilterStreamBuilder::new_from_binary_data(data).finish(&sc.serialize_settings())
    };

    let base_font = base_font_name(&self.font, &self.glyph_remapper);
    let base_font_type0 = if is_cff {
      format!("{base_font}-{IDENTITY_H}")
    } else {
      base_font.clone()
    };

    chunk
      .type0_font(root_ref)
      .base_font(Name(base_font_type0.as_bytes()))
      .encoding_predefined(Name(IDENTITY_H.as_bytes()))
      .descendant_font(cid_ref)
      .to_unicode(cmap_ref);

    let mut cid = chunk.cid_font(cid_ref);
    cid.subtype(if is_cff {
      CidFontType::Type0
    } else {
      CidFontType::Type2
    });
    cid.base_font(Name(base_font.as_bytes()));
    cid.system_info(SYSTEM_INFO);
    cid.font_descriptor(descriptor_ref);
    cid.default_width(0.0);

    if !is_cff {
      cid.cid_to_gid_map_predefined(Name(b"Identity"));
    }

    // IN CID fonts, a upem value of 1000 is assumed for all fonts, so we need to convert.
    let to_pdf_units = |v: f32| v / self.font.units_per_em() * self.units_per_em();

    let mut first = 0;
    let mut width_writer = cid.widths();
    for (w, group) in self.widths.group_by_key(|&w| w) {
      let end = first + group.len();
      if w != 0.0 {
        let last = end - 1;
        width_writer.same(first as u16, last as u16, to_pdf_units(w));
      }
      first = end;
    }

    width_writer.finish();
    cid.finish();

    // The only reason we write this in the first place is that PDF/A-1b requires
    // a CIDSet.
    if !sc.serialize_settings().pdf_version().deprecates_cid_set() {
      let cid_stream_data = {
        // It's always guaranteed by the subsetter that CIDs start from 0 and are
        // consecutive, so this encoding is very straight-forward.
        let mut bytes = vec![];
        bytes.extend([0xFFu8].repeat((num_glyphs / 8) as usize));
        let padding = num_glyphs % 8;
        if padding != 0 {
          bytes.push(!(0xFF >> padding))
        }

        bytes
      };

      let cid_stream = FilterStreamBuilder::new_from_binary_data(&cid_stream_data)
        .finish(&sc.serialize_settings());
      let mut cid_set = stream_chunk.stream(cid_set_ref, cid_stream.encoded_data());
      cid_stream.write_filters(cid_set.deref_mut());
      cid_set.finish();
      cid_stream.finish();
    }

    let mut flags = FontFlags::empty();
    flags.set(
      FontFlags::SERIF,
      self
        .font
        .postscript_name()
        .is_some_and(|n| n.contains("Serif")),
    );
    flags.set(FontFlags::FIXED_PITCH, self.font.is_monospaced());
    flags.set(FontFlags::ITALIC, self.font.italic_angle() != 0.0);
    flags.insert(FontFlags::SYMBOLIC);
    flags.insert(FontFlags::SMALL_CAP);

    let bbox = {
      Rect::from_ltrb(
        to_pdf_units(global_bbox.left()),
        to_pdf_units(global_bbox.top()),
        to_pdf_units(global_bbox.right()),
        to_pdf_units(global_bbox.bottom()),
      )
      .unwrap()
    }
    .to_pdf_rect();

    let italic_angle = self.font.italic_angle();
    let ascender = to_pdf_units(self.font.ascent());
    let descender = to_pdf_units(self.font.descent());
    let cap_height = self.font.cap_height().map(to_pdf_units).unwrap_or(ascender);
    let stem_v = 10.0 + 0.244 * (self.font.weight() - 50.0);

    let mut font_descriptor = chunk.font_descriptor(descriptor_ref);
    font_descriptor
      .name(Name(base_font.as_bytes()))
      .flags(flags)
      .bbox(bbox)
      .italic_angle(italic_angle)
      .ascent(ascender)
      .descent(descender)
      .cap_height(cap_height)
      .stem_v(stem_v);

    if !sc.serialize_settings().pdf_version().deprecates_cid_set() {
      font_descriptor.cid_set(cid_set_ref);
    }

    if is_cff {
      font_descriptor.font_file3(data_ref);
    } else {
      font_descriptor.font_file2(data_ref);
    }

    font_descriptor.finish();

    let cmap = {
      let mut cmap = UnicodeCmap::new(CMAP_NAME, SYSTEM_INFO);

      // For the .notdef glyph, it's fine if no mapping exists, since it is included
      // even if it was not referenced in the text.
      for g in 1..self.glyph_remapper.num_gids() {
        let entry = self.cmap_entries.get(&g);
        write_cmap_entry(&self.font, entry, sc, &mut cmap, g);
      }

      cmap
    };

    let cmap_stream = cmap.finish();
    let cmap_stream =
      FilterStreamBuilder::new_from_content_stream(&cmap_stream, &sc.serialize_settings())
        .finish(&sc.serialize_settings());
    let mut cmap = stream_chunk.cmap(cmap_ref, cmap_stream.encoded_data());
    cmap_stream.write_filters(cmap.deref_mut().deref_mut());
    cmap.writing_mode(WMode::Horizontal);
    cmap.finish();

    let mut stream = stream_chunk.stream(data_ref, font_stream.encoded_data());
    font_stream.write_filters(stream.deref_mut());
    if is_cff {
      stream.pair(Name(b"Subtype"), Name(b"CIDFontType0C"));
    }

    stream.finish();
    chunk_container.streams.fonts.push(stream_chunk);

    Ok(())
  }
}

/// Create a tag for a font subset.
pub(crate) fn subset_tag<T: Hash>(data: &T) -> String {
  const BASE: u128 = 26;
  let mut hash = stable_hash128(data);
  let mut letter = [b'A'; SUBSET_TAG_LEN];
  for l in letter.iter_mut() {
    *l = b'A' + (hash % BASE) as u8;
    hash /= BASE;
  }
  std::str::from_utf8(&letter).unwrap().to_string()
}

pub(crate) fn base_font_name<T: Hash>(font: &Font, data: &T) -> String {
  const REST_LEN: usize = SUBSET_TAG_LEN + 1 + 1 + IDENTITY_H.len();

  let postscript_name = font.postscript_name().unwrap_or("unknown");
  let max_len = 127 - REST_LEN;
  let trimmed = &postscript_name[..postscript_name.len().min(max_len)];
  let subset_tag = subset_tag(&data);

  format!("{subset_tag}+{trimmed}")
}

fn subset_font(font: Font, glyph_remapper: &GlyphRemapper) -> KrillaResult<(Font, Rect)> {
  let mut bbox: Option<Rect> = None;

  let variation_coordinates = font
    .variation_coordinates()
    .iter()
    .map(|v| (SubsetTag::new(v.0.get()), v.1.get()))
    .collect::<Vec<_>>();
  let font = subset_with_variations(
    font.font_data().as_ref(),
    font.index(),
    &variation_coordinates,
    glyph_remapper,
  )
  .map_err(|e| KrillaError::Font(font.clone(), format!("failed to subset font: {e}")))
  .and_then(|data| {
    Font::new(Arc::new(data).into(), 0).ok_or(KrillaError::Font(
      font.clone(),
      "failed to subset font".to_string(),
    ))
  })?;
  let global_bbox = font.bbox();

  for g in 0..font.num_glyphs() {
    if let Some(path_bbox) = compute_bbox(&font, skrifa::GlyphId::new(g)) {
      bbox = bbox
        .map(|mut r| {
          r.expand(&path_bbox);
          r
        })
        .or(Some(path_bbox));
    }
  }

  Ok((font, bbox.unwrap_or(global_bbox)))
}

fn compute_bbox(font: &Font, glyph: skrifa::GlyphId) -> Option<Rect> {
  let outline_glyphs = font.outline_glyphs();

  if let Some(outline_glyph) = outline_glyphs.get(glyph) {
    let mut glyph_builder = OutlineBuilder::new();
    let _ = outline_glyph.draw(
      DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
      &mut glyph_builder,
    );
    glyph_builder.finish().map(|p| Rect::from_tsp(p.bounds()))
  } else {
    None
  }
}
