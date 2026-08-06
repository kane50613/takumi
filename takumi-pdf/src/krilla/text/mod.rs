//! Text and font support.
//!
//! krilla has extensive support for OpenType fonts. It supports CFF-based as well
//! as TTF-based OpenType fonts. In addition to that, krilla also supports
//! all major tables used in color fonts, including the `SVG`, `COLR`, `sbix` and
//! `CBDT`/`EBDT` (only PNG) tables, something that, to the best of my knowledge, no other
//! Rust crates provides.
//!
//! Even better is the fact that you do not need to take care of choosing the right
//! table for drawing glyphs: All you need to do is to provide the [`Font`] object with
//! an appropriate index.

use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

use rustc_hash::FxHashMap;

use crate::krilla::text::cid::CIDFont;
use crate::krilla::text::type3::{ColoredGlyph, Type3Font, Type3FontMapper, Type3ID};
pub(crate) mod cid;
pub(crate) mod font;
pub(crate) mod glyph;
pub(crate) mod group;
pub(crate) mod type3;

pub use font::*;
pub use glyph::*;

pub(crate) const PDF_UNITS_PER_EM: f32 = 1000.0;

/// A unique CID identifier.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub(crate) struct CIDIdentifier(pub Font);

/// A unique Type3 font identifier. Type3 fonts can only hold 256 glyphs, which
/// means that we might have to create more than one Type3 font. This is why we
/// additionally store an index that indicates which specific Type3Font we are
/// referring to.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub(crate) struct Type3Identifier(pub Font, pub Type3ID);

/// A font identifier for a PDF font.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub(crate) enum FontIdentifier {
  Cid(CIDIdentifier),
  Type3(Type3Identifier),
}

/// A container that holds all PDF fonts belonging to an OTF font.
pub(crate) struct FontContainer {
  font: Font,
  type3_mapper: Type3FontMapper,
  cid_font: CIDFont,
  cid_cache: FxHashMap<u32, (FontIdentifier, PDFGlyph)>,
  type3_cache: HashMap<ColoredGlyph, (FontIdentifier, PDFGlyph)>,
}

impl FontContainer {
  pub(crate) fn new(font: Font) -> Self {
    Self {
      font: font.clone(),
      type3_mapper: Type3FontMapper::new(font.clone()),
      cid_font: CIDFont::new(font.clone()),
      cid_cache: Default::default(),
      type3_cache: Default::default(),
    }
  }

  pub(crate) fn type3_mapper(&self) -> &Type3FontMapper {
    &self.type3_mapper
  }

  pub(crate) fn cid_font(&self) -> &CIDFont {
    &self.cid_font
  }

  #[inline]
  pub(crate) fn font_identifier(&self, glyph: ColoredGlyph) -> Option<FontIdentifier> {
    let (id, _) = self
      .cid_cache
      .get(&glyph.glyph_id.to_u32())
      .or_else(|| self.type3_cache.get(&glyph.to_owned()))?;
    Some(id.clone())
  }

  #[inline]
  pub(crate) fn get_from_identifier_mut(
    &mut self,
    font_identifier: FontIdentifier,
  ) -> Option<&mut dyn PdfFont> {
    if self.cid_font.identifier() == font_identifier {
      Some(&mut self.cid_font)
    } else {
      // If the identifier doesn't match either of CID or Type3, this will
      // return `None`.
      Some(self.type3_mapper.font_mut_from_id(font_identifier)?)
    }
  }

  #[inline]
  pub(crate) fn get_from_identifier(
    &self,
    font_identifier: FontIdentifier,
  ) -> Option<&dyn PdfFont> {
    if self.cid_font.identifier() == font_identifier {
      Some(&self.cid_font)
    } else {
      // If the identifier doesn't match either of CID or Type3, this will
      // return `None`.
      Some(self.type3_mapper.font_from_id(font_identifier)?)
    }
  }

  #[inline]
  pub(crate) fn add_glyph(&mut self, glyph: ColoredGlyph) -> (FontIdentifier, PDFGlyph) {
    if let Some(e) = self
      .cid_cache
      .get(&glyph.glyph_id.to_u32())
      .or_else(|| self.type3_cache.get(&glyph.to_owned()))
    {
      // We already know whether this glyph uses a CID or Type3 glyph.
      e.clone()
    } else if should_outline(&self.font, glyph.glyph_id) {
      let cid = self.cid_font.add_glyph(glyph.glyph_id);
      let res = (self.cid_font.identifier(), PDFGlyph::Cid(cid));
      self.cid_cache.insert(glyph.glyph_id.to_u32(), res.clone());
      res
    } else {
      let (identifier, gid) = self.type3_mapper.add_glyph(glyph.to_owned());
      let res = (identifier, PDFGlyph::Type3(gid));
      self.type3_cache.insert(glyph.to_owned(), res.clone());
      res
    }
  }
}

pub(crate) trait PdfFont {
  fn units_per_em(&self) -> f32;
  fn font(&self) -> Font;
  fn get_codepoints(&self, pdf_glyph: PDFGlyph) -> Option<&str>;
  fn set_codepoints(
    &mut self,
    pdf_glyph: PDFGlyph,
    text: String,
    location: Option<crate::krilla::surface::Location>,
  );
  fn get_gid(&self, glyph: ColoredGlyph) -> Option<PDFGlyph>;
  fn force_fill(&self) -> bool;
}

impl PdfFont for Type3Font {
  fn units_per_em(&self) -> f32 {
    self.unit_per_em()
  }

  fn font(&self) -> Font {
    Type3Font::font(self)
  }

  #[track_caller]
  fn get_codepoints(&self, pdf_glyph: PDFGlyph) -> Option<&str> {
    match pdf_glyph {
      PDFGlyph::Type3(t3) => self.get_codepoints(t3),
      PDFGlyph::Cid(_) => panic!("attempted to pass cid to type 3 font"),
    }
  }

  #[track_caller]
  fn set_codepoints(
    &mut self,
    pdf_glyph: PDFGlyph,
    text: String,
    location: Option<crate::krilla::surface::Location>,
  ) {
    match pdf_glyph {
      PDFGlyph::Type3(t3) => self.set_codepoints(t3, text, location),
      PDFGlyph::Cid(_) => panic!("attempted to pass cid to type 3 font"),
    }
  }

  fn get_gid(&self, glyph: ColoredGlyph) -> Option<PDFGlyph> {
    self.get_gid(&glyph.to_owned()).map(PDFGlyph::Type3)
  }

  fn force_fill(&self) -> bool {
    true
  }
}

impl PdfFont for CIDFont {
  fn units_per_em(&self) -> f32 {
    self.units_per_em()
  }

  fn font(&self) -> Font {
    CIDFont::font(self)
  }

  #[track_caller]
  fn get_codepoints(&self, pdf_glyph: PDFGlyph) -> Option<&str> {
    match pdf_glyph {
      PDFGlyph::Type3(_) => panic!("attempted to pass type 3 glyph to cid font"),
      PDFGlyph::Cid(cid) => self.get_codepoints(cid),
    }
  }

  #[track_caller]
  fn set_codepoints(
    &mut self,
    pdf_glyph: PDFGlyph,
    text: String,
    location: Option<crate::krilla::surface::Location>,
  ) {
    match pdf_glyph {
      PDFGlyph::Type3(_) => panic!("attempted to pass type 3 glyph to cid font"),
      PDFGlyph::Cid(cid) => self.set_codepoints(cid, text, location),
    }
  }

  fn get_gid(&self, glyph: ColoredGlyph) -> Option<PDFGlyph> {
    self.get_cid(glyph.glyph_id).map(PDFGlyph::Cid)
  }

  fn force_fill(&self) -> bool {
    false
  }
}
