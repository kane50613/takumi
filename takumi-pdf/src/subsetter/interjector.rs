use std::marker::PhantomData;

pub(crate) enum Interjector<'a> {
  Dummy(PhantomData<&'a ()>),
  Skrifa(skrifa::SkrifaInterjector<'a>),
}

impl Interjector<'_> {
  pub fn is_skrifa(&self) -> bool {
    match self {
      Self::Dummy(_) => false,
      Self::Skrifa(_) => true,
    }
  }
}

pub(crate) mod skrifa {
  use crate::subsetter::{MaxpData, Tag};
  use kurbo::{BezPath, CubicBez};
  use skrifa::instance::{Location, LocationRef};
  use skrifa::outline::{DrawSettings, OutlinePen};
  use skrifa::prelude::Size;
  use skrifa::{FontRef, GlyphId, MetadataProvider};
  use takumi_core::resources::glyph::ErasedPen;
  use write_fonts::tables::glyf::SimpleGlyph;
  use write_fonts::{FontWrite, TableWriter, dump_table};

  pub(crate) struct SkrifaInterjector<'a> {
    font_ref: FontRef<'a>,
    location: Location,
  }

  impl<'a> SkrifaInterjector<'a> {
    pub(crate) fn new(data: &'a [u8], index: u32, location: &[(Tag, f32)]) -> Option<Self> {
      let font_ref = FontRef::from_index(data, index).ok()?;
      let location = font_ref
        .axes()
        .location(location.iter().map(|i| (skrifa::Tag::new(i.0.get()), i.1)));

      Some(Self { font_ref, location })
    }

    pub(crate) fn is_default_location(&self) -> bool {
      LocationRef::from(&self.location).is_default()
    }
  }

  impl<'a> SkrifaInterjector<'a> {
    /// Return the glyph description in the `glyf` outline format.
    pub(crate) fn interject<'b>(
      &'b self,
      maxp_data: &'b mut MaxpData,
      glyph: u16,
    ) -> Option<(u16, i16, Vec<u8>)> {
      let outlines = self.font_ref.outline_glyphs();
      let metrics = self
        .font_ref
        .glyph_metrics(Size::unscaled(), &self.location);

      let mut outline_builder = OutlinePath::new();
      let glyph_id = GlyphId::new(glyph as u32);

      if let Some(outline_glyph) = outlines.get(glyph_id) {
        outline_glyph
          .draw(
            DrawSettings::unhinted(Size::unscaled(), &self.location),
            &mut ErasedPen(&mut outline_builder),
          )
          .ok()?;
      }

      let path = outline_builder.path;

      let simple_glyph = SimpleGlyph::from_bezpath(&path).ok()?;
      let advance = metrics.advance_width(glyph_id)?.round() as u16;

      // We derive the LSB from the resulting bounding box rather than
      // from the font's metrics, because the latter does not always agree
      // with the `xMin` of the fresh outline we've generated.
      //
      // The OpenType spec heavily advises xMin and LSB to match (it
      // actually requires it for variable fonts or when `head.flags` bit
      // 1 is set).
      //
      // If `LSB != xMin`, glyphs get repositioned by PDF readers and the
      // kerning gets very wonky.
      let lsb = simple_glyph.bbox.x_min;

      if path.is_empty() {
        return Some((advance, lsb, vec![]));
      }

      maxp_data.max_points = maxp_data
        .max_points
        .max(simple_glyph.contours.iter().map(|c| c.len() as u16).sum());
      maxp_data.max_contours = maxp_data
        .max_contours
        .max(simple_glyph.contours.len() as u16);

      let mut writer = TableWriter::default();
      simple_glyph.write_into(&mut writer);
      let data = dump_table(&simple_glyph).ok()?;

      Some((advance, lsb, data))
    }
  }

  pub(crate) struct OutlinePath {
    last_move_to: (f32, f32),
    last_point: (f32, f32),
    path: BezPath,
  }

  impl OutlinePath {
    fn new() -> Self {
      Self {
        last_move_to: (0.0, 0.0),
        last_point: (0.0, 0.0),
        path: BezPath::new(),
      }
    }
  }

  impl OutlinePen for OutlinePath {
    #[inline]
    fn move_to(&mut self, x: f32, y: f32) {
      self.path.move_to((x, y));
      self.last_move_to = (x, y);
      self.last_point = (x, y);
    }

    #[inline]
    fn line_to(&mut self, x: f32, y: f32) {
      self.path.line_to((x, y));
      self.last_point = (x, y);
    }

    #[inline]
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
      // Only called by TrueType fonts.
      self.path.quad_to((cx, cy), (x, y));
      self.last_point = (x, y);
    }

    #[inline]
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
      // Only called by CFF2 fonts.
      let cubic = CubicBez::new(
        (self.last_point.0 as f64, self.last_point.1 as f64),
        (cx0 as f64, cy0 as f64),
        (cx1 as f64, cy1 as f64),
        (x as f64, y as f64),
      );

      // It is not entirely clear how small the `accuracy` parameter needs to be
      // to produce sensible results. In `vello_cpu`, a value of around 0.025 is used
      // (0.25 for the flattening accuracy and * 0.1 in the flattening code, so we choose
      // the same value here.
      for (_, _, quad) in cubic.to_quads(0.025) {
        // Note that `quad.p2` is the same as `quad.p0` of the next point in the iterator.
        self.quad_to(
          quad.p1.x as f32,
          quad.p1.y as f32,
          quad.p2.x as f32,
          quad.p2.y as f32,
        );
      }
    }

    #[inline]
    fn close(&mut self) {
      self.path.close_path();
      self.last_point = self.last_move_to;
    }
  }
}
