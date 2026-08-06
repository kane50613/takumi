use std::collections::HashSet;
use std::hash::Hash;
use std::ops::DerefMut;

use pdf_writer::types::{FontFlags, UnicodeCmap};
use pdf_writer::writers::WMode;
use pdf_writer::{Finish, Name, Ref, Str};
use rustc_hash::FxHashMap;

use super::{FontIdentifier, Type3Identifier};
use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::color::rgb;
use crate::krilla::configure::PdfVersion;
use crate::krilla::geom::Path;
use crate::krilla::geom::{Rect, Transform};
use crate::krilla::graphics::paint::Fill;
use crate::krilla::graphics::xobject::XObject;
use crate::krilla::resource::ResourceDictionaryBuilder;
use crate::krilla::serialize::SerializeContext;
use crate::krilla::stream::{FilterStreamBuilder, StreamBuilder};
use crate::krilla::surface::Location;
use crate::krilla::text::GlyphId;
use crate::krilla::text::cid::write_cmap_entry;
use crate::krilla::text::cid::{CMAP_NAME, IDENTITY_H, SYSTEM_INFO};
use crate::krilla::text::outline::glyph_path;
use crate::krilla::text::{self, Font};
use crate::krilla::util::NameExt;

pub(crate) type Gid = u8;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(crate) struct ColoredGlyph {
  pub(crate) glyph_id: GlyphId,
  // Some formats like COLR and SVG allow drawing the same glyph
  // with a different context color, so we need to store that as well.
  pub(crate) context_color: rgb::Color,
}

impl ColoredGlyph {
  pub(crate) fn new(glyph_id: GlyphId, context_color: rgb::Color) -> ColoredGlyph {
    Self {
      glyph_id,
      context_color,
    }
  }
}

#[derive(Debug)]
pub(crate) struct Type3Font {
  font: Font,
  glyphs: Vec<ColoredGlyph>,
  widths: Vec<f32>,
  cmap_entries: FxHashMap<Gid, (String, Option<Location>)>,
  glyph_set: HashSet<ColoredGlyph>,
  index: usize,
}

impl Type3Font {
  pub(crate) fn new(font: Font, index: usize) -> Self {
    Self {
      font,
      glyphs: Vec::new(),
      cmap_entries: FxHashMap::default(),
      widths: Vec::new(),
      glyph_set: HashSet::new(),
      index,
    }
  }

  // Unlike CID fonts, the units per em of type 3 fonts does not have to be 1000.
  pub(crate) fn unit_per_em(&self) -> f32 {
    self.font.units_per_em()
  }

  pub(crate) fn is_full(&self) -> bool {
    self.count() == 256
  }

  pub(crate) fn count(&self) -> u16 {
    u16::try_from(self.glyphs.len()).unwrap()
  }

  #[inline]
  pub(crate) fn covers(&self, glyph: &ColoredGlyph) -> bool {
    self.glyph_set.contains(glyph)
  }

  #[inline]
  pub(crate) fn get_gid(&self, glyph: &ColoredGlyph) -> Option<u8> {
    self
      .glyphs
      .iter()
      .position(|g| g == glyph)
      .and_then(|n| u8::try_from(n).ok())
  }

  #[inline]
  pub(crate) fn add_glyph(&mut self, glyph: ColoredGlyph) -> u8 {
    if let Some(pos) = self.get_gid(&glyph) {
      pos
    } else {
      assert!(self.glyphs.len() < 256);

      self.glyph_set.insert(glyph);
      self.glyphs.push(glyph);
      self
        .widths
        .push(self.font.advance_width(glyph.glyph_id).unwrap_or(0.0));
      u8::try_from(self.glyphs.len() - 1).unwrap()
    }
  }

  #[inline]
  pub(crate) fn get_codepoints(&self, gid: Gid) -> Option<&str> {
    self.cmap_entries.get(&gid).map(|s| s.0.as_str())
  }

  #[inline]
  pub(crate) fn set_codepoints(&mut self, gid: Gid, text: String, location: Option<Location>) {
    self.cmap_entries.insert(gid, (text, location));
  }

  #[inline]
  pub(crate) fn font(&self) -> Font {
    self.font.clone()
  }

  #[inline]
  pub(crate) fn identifier(&self) -> FontIdentifier {
    FontIdentifier::Type3(Type3Identifier(self.font.clone(), self.index))
  }

  pub(crate) fn serialize(
    &self,
    sc: &mut SerializeContext,
    chunk_container: &mut ChunkContainer,
    root_ref: Ref,
  ) {
    let mut stream_chunk = sc.new_chunk();

    let mut rd_builder = ResourceDictionaryBuilder::new();
    let mut font_bbox = Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap();

    let glyph_streams = self
      .glyphs
      .iter()
      .enumerate()
      .map(|(index, glyph)| {
        let mut stream_surface = StreamBuilder::new(sc, chunk_container);
        let mut surface = stream_surface.surface();

        // In case this returns `None`, the surface is guaranteed to be empty.
        let drawn_color_glyph = text::draw_color_glyph(
          self.font.clone(),
          glyph.context_color,
          glyph.glyph_id,
          Transform::default(),
          &mut surface,
        );

        if drawn_color_glyph.is_none() {
          // If this code path is reached, either we are dealing with a glyph that has no outline
          // (like .notdef), or it means we tried to create a Type3
          // font from a font that does not have any (valid) color/bitmap/svg table.
          // Therefore, as a last resort we draw the outline glyph as a black glyph.
          // PDF does have the concept of shape glyphs which take the color or where
          // they are drawn from.
          // The problem with those is that they seemingly have very bad viewer support. For
          // example, filled glyphs with gradients become broken in some viewers,
          // and stroking does not work at all consistently. Because of this,
          // we don't bother trying to implement it for now.

          // If this is the case (i.e. no color glyph was drawn, either because no table
          // exists or an error occurred, the surface is guaranteed to be empty.
          // So we can just safely draw the outline glyph instead without having to
          // worry about the surface being "dirty".
          if let Some(path) = glyph_path(self.font.clone(), glyph.glyph_id) {
            surface.set_fill(Some(Fill::default()));
            surface.set_stroke(None);
            surface.draw_path(&Path(path));
          }
        };

        surface.finish();
        let stream = stream_surface.finish();
        let mut content = sc.new_content();

        // I considered writing into the stream directly instead of creating an XObject
        // and showing that, but it seems like many viewers don't like that, and emojis
        // look messed up. Using XObjects seems like the best choice here.
        content.start_color_glyph(self.widths[index]);
        let x_object = XObject::new(stream, false, false, None);
        if !x_object.is_empty() {
          font_bbox.expand(&x_object.bbox());
          let x_name =
            rd_builder.register_resource(sc.register_resourceable(chunk_container, x_object));
          content.x_object(x_name.to_pdf_name());
        }

        let stream = content.finish();

        let font_stream =
          FilterStreamBuilder::new_from_content_stream(stream.as_slice(), &sc.serialize_settings())
            .finish(&sc.serialize_settings());

        let stream_ref = sc.new_ref();
        let mut stream = stream_chunk.stream(stream_ref, font_stream.encoded_data());
        font_stream.write_filters(stream.deref_mut());

        stream_ref
      })
      .collect::<Vec<Ref>>();

    let resource_dictionary = rd_builder.finish();
    let chunk = &mut chunk_container.non_stream.fonts;

    let descriptor_ref = if sc.serialize_settings().pdf_version() >= PdfVersion::Pdf15 {
      Some(sc.new_ref())
    } else {
      None
    };
    let cmap_ref = sc.new_ref();

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

    let italic_angle = self.font.italic_angle();
    let ascender = font_bbox.bottom();
    let descender = font_bbox.top();

    let mut gids = self
      .glyph_set
      .iter()
      .map(|g| g.glyph_id)
      .collect::<Vec<_>>();
    gids.sort();
    let base_font = base_font_name(&self.font);

    if let Some(descriptor_ref) = descriptor_ref {
      // Write the font descriptor (contains metrics about the font).
      let mut font_descriptor = chunk.font_descriptor(descriptor_ref);
      font_descriptor
        .name(Name(base_font.as_bytes()))
        .flags(flags)
        .bbox(font_bbox.to_pdf_rect())
        .italic_angle(italic_angle)
        .ascent(ascender)
        .descent(descender)
        // Adobe recommends these for tagged PDF for 1.5+ (descriptors for Type3 fonts
        // are only written for 1.5+ in the first place, so no additional checks needed)
        // so we write them as well.
        // Unfortunately we have no way of determining the actual family name, so we just
        // take the next best thing
        .family(Str(base_font.as_bytes()))
        .stretch(
          match skrifa::attribute::Stretch::new(self.font().stretch()) {
            skrifa::attribute::Stretch::ULTRA_CONDENSED => {
              pdf_writer::types::FontStretch::UltraCondensed
            }
            skrifa::attribute::Stretch::EXTRA_CONDENSED => {
              pdf_writer::types::FontStretch::ExtraCondensed
            }
            skrifa::attribute::Stretch::CONDENSED => pdf_writer::types::FontStretch::Condensed,
            skrifa::attribute::Stretch::SEMI_CONDENSED => {
              pdf_writer::types::FontStretch::SemiCondensed
            }
            skrifa::attribute::Stretch::NORMAL => pdf_writer::types::FontStretch::Normal,
            skrifa::attribute::Stretch::SEMI_EXPANDED => {
              pdf_writer::types::FontStretch::SemiExpanded
            }
            skrifa::attribute::Stretch::EXPANDED => pdf_writer::types::FontStretch::Expanded,
            skrifa::attribute::Stretch::EXTRA_EXPANDED => {
              pdf_writer::types::FontStretch::ExtraExpanded
            }
            skrifa::attribute::Stretch::ULTRA_EXPANDED => {
              pdf_writer::types::FontStretch::UltraExpanded
            }
            // Fallback
            _ => pdf_writer::types::FontStretch::Normal,
          },
        )
        .weight(match self.font.weight() as i32 {
          150..250 => 200,
          250..350 => 300,
          350..450 => 400,
          450..550 => 500,
          550..650 => 600,
          650..750 => 700,
          750..850 => 800,
          other => {
            if other < 150 {
              100
            } else {
              900
            }
          }
        });

      font_descriptor.finish();
    }

    let mut type3_font = chunk.type3_font(root_ref);
    resource_dictionary.to_pdf_resources(
      &mut type3_font,
      sc,
      &mut chunk_container.non_stream.resource_dictionaries,
    );

    // See https://github.com/typst/typst/issues/5067 as to why we write this.
    type3_font.name(Name(base_font.as_bytes()));
    type3_font.bbox(font_bbox.to_pdf_rect());
    type3_font.to_unicode(cmap_ref);
    type3_font.matrix(
      Transform::from_scale(
        1.0 / (self.font.units_per_em()),
        1.0 / (self.font.units_per_em()),
      )
      .to_pdf_transform(),
    );
    type3_font.first_char(0);
    type3_font.last_char(u8::try_from(self.glyphs.len() - 1).unwrap());
    type3_font.widths(self.widths.iter().copied());
    if let Some(descriptor_ref) = descriptor_ref {
      type3_font.font_descriptor(descriptor_ref);
    }

    let mut char_procs = type3_font.char_procs();
    for (gid, ref_) in glyph_streams.iter().enumerate() {
      char_procs.pair(format!("g{gid}").to_pdf_name(), *ref_);
    }
    char_procs.finish();

    let names = (0..self.glyphs.len() as u16)
      .map(|gid| format!("g{gid}"))
      .collect::<Vec<_>>();

    type3_font
      .encoding_custom()
      .differences()
      .consecutive(0, names.iter().map(|n| n.to_pdf_name()));

    type3_font.finish();

    let cmap = {
      let mut cmap = UnicodeCmap::new(CMAP_NAME, SYSTEM_INFO);

      for g in 0..self.glyphs.len() {
        let g = u8::try_from(g).unwrap();
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
    chunk_container.streams.fonts.push(stream_chunk);
  }
}

pub(crate) type Type3ID = usize;

// A font can have multiple Type3 fonts, if more than 256 glyphs are used.
// We use this struct to keep track of them.
#[derive(Debug)]
pub(crate) struct Type3FontMapper {
  font: Font,
  fonts: Vec<Type3Font>,
}

impl Type3FontMapper {
  pub(crate) fn new(font: Font) -> Type3FontMapper {
    Self {
      font,
      fonts: Vec::new(),
    }
  }
}

impl Type3FontMapper {
  /// Given a requested glyph coverage, find the corresponding font identifier of the
  /// font that contains it.
  pub(crate) fn id_from_glyph(&self, glyph: &ColoredGlyph) -> Option<FontIdentifier> {
    self
      .fonts
      .iter()
      .position(|f| f.covers(glyph))
      .map(|p| self.fonts[p].identifier())
  }

  /// Given a font identifier, return the corresponding Type3 font if it is part of
  /// that type 3 font mapper.
  pub(crate) fn font_from_id(&self, identifier: FontIdentifier) -> Option<&Type3Font> {
    let pos = self
      .fonts
      .iter()
      .position(|f| f.identifier() == identifier)?;
    self.fonts.get(pos)
  }

  pub(crate) fn font_mut_from_id(&mut self, identifier: FontIdentifier) -> Option<&mut Type3Font> {
    let pos = self
      .fonts
      .iter()
      .position(|f| f.identifier() == identifier)?;
    self.fonts.get_mut(pos)
  }

  pub(crate) fn is_empty(&self) -> bool {
    self.fonts.is_empty()
  }

  pub(crate) fn fonts(&self) -> &[Type3Font] {
    &self.fonts
  }

  pub(crate) fn add_glyph(&mut self, glyph: ColoredGlyph) -> (FontIdentifier, Gid) {
    // If the glyph has already been added, return the font identifier of
    // the type 3 font as well as the Type3 gid in that font.
    if let Some(id) = self.id_from_glyph(&glyph) {
      let gid = self
        .font_from_id(id.clone())
        .unwrap()
        .get_gid(&glyph)
        .unwrap();
      return (id, gid);
    }

    if let Some(last_font) = self.fonts.last_mut() {
      if last_font.is_full() {
        // If the last Type3 font is full, create a new one.
        let mut font = Type3Font::new(self.font.clone(), self.fonts.len());
        let id = font.identifier();
        let gid = font.add_glyph(glyph);
        self.fonts.push(font);
        (id, gid)
      } else {
        // Otherwise, insert it into the last Type3 font.
        let id = last_font.identifier();
        (id, last_font.add_glyph(glyph))
      }
    } else {
      // If not Type3 font exists yet, create a new one.
      let mut font = Type3Font::new(self.font.clone(), self.fonts.len());
      let id = font.identifier();
      let gid = font.add_glyph(glyph);
      self.fonts.push(font);
      (id, gid)
    }
  }
}

pub(crate) fn base_font_name(font: &Font) -> String {
  const REST_LEN: usize = 1 + 1 + IDENTITY_H.len();
  let postscript_name = font.postscript_name().unwrap_or("unknown");

  let max_len = 127 - REST_LEN;

  let trimmed = &postscript_name[..postscript_name.len().min(max_len)];

  trimmed.to_string()
}

#[cfg(test)]
mod tests {
  use crate::krilla::color::rgb;
  use crate::krilla::text::Font;
  use crate::krilla::text::GlyphId;
  use crate::krilla::text::type3::{ColoredGlyph, Type3FontMapper};
  use crate::krilla::util::test_utils::NOTO_COLOR_EMOJI_COLR;

  #[test]
  fn type3_more_than_256_glyphs() {
    let font = Font::new(NOTO_COLOR_EMOJI_COLR.clone(), 0).unwrap();
    let mut t3 = Type3FontMapper::new(font.clone());

    for i in 2..258 {
      t3.add_glyph(ColoredGlyph::new(GlyphId::new(i), rgb::Color::black()));
    }

    assert_eq!(t3.fonts.len(), 1);
    assert_eq!(
      t3.fonts[0].add_glyph(ColoredGlyph::new(GlyphId::new(20), rgb::Color::black())),
      18
    );

    t3.add_glyph(ColoredGlyph::new(GlyphId::new(512), rgb::Color::black()));
    assert_eq!(t3.fonts.len(), 2);
  }
}
