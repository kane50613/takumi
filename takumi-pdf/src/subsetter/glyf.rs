//! The `glyf` table contains the main description of the glyphs. In order to
//! subset it, there are 5 things we need to do:
//! 1. We need to form the glyph closure. Glyphs can reference other glyphs, meaning that
//!    if a user for example requests the glyph 1, and this glyph references the glyph 2, then
//!    we need to include both of them in our subset.
//! 2. We need to remove glyph descriptions that are not needed for the subset, and reorder
//!    the existing glyph descriptions to match the order defined by the remapper.
//! 3. For component glyphs, we need to rewrite their description so that they reference
//!    the new glyph ID of the glyphs they reference.
//! 4. We need to calculate which format to use in the `loca` table.
//! 5. We need to update the `loca` table itself with the new offsets.
use super::*;

/// Form the glyph closure of all glyphs in `gid_set`.
pub fn closure(face: &Face, glyph_remapper: &mut GlyphRemapper) -> Result<()> {
  let table = Table::new(face).ok_or(MalformedFont)?;

  let mut process_glyphs = glyph_remapper.remapped_gids().collect::<Vec<_>>();

  while let Some(glyph) = process_glyphs.pop() {
    glyph_remapper.remap(glyph);

    let glyph_data = match table.glyph_data(glyph) {
      Some(glph_data) => glph_data,
      None => continue,
    };

    if glyph_data.is_empty() {
      continue;
    }

    let mut r = Reader::new(glyph_data);
    let num_contours = r.read::<i16>().ok_or(MalformedFont)?;

    // If we have a composite glyph, add its components to the closure.
    if num_contours < 0 {
      for component in component_glyphs(glyph_data).ok_or(MalformedFont)? {
        if glyph_remapper.get(component).is_none() {
          process_glyphs.push(component);
        }
      }
    }
  }

  Ok(())
}

pub fn subset(ctx: &mut Context) -> Result<()> {
  let table = Table::new(&ctx.face).ok_or(MalformedFont)?;

  #[allow(unused_mut)]
  let mut maxp_data = MaxpData::default();
  #[allow(unused_mut)]
  let mut hmtx_data = Vec::new();

  subset_with(ctx, |old_gid, ctx| {
    let data = match &ctx.interjector {
      Interjector::Dummy(_) => Cow::Borrowed(table.glyph_data(old_gid).ok_or(MalformedFont)?),
      Interjector::Skrifa(s) => {
        let (advance, lsb, data) = s.interject(&mut maxp_data, old_gid).ok_or(MalformedFont)?;
        hmtx_data.push((advance, lsb));
        Cow::Owned(data)
      }
    };

    Ok(data)
  })?;

  if ctx.interjector.is_skrifa() {
    ctx.custom_maxp_data = Some(maxp_data);
    ctx.custom_hmtx_data = Some(hmtx_data);
  }

  Ok(())
}

pub(crate) fn subset_with<'a>(
  ctx: &mut Context<'a>,
  glyph_data_fn: impl FnMut(u16, &Context<'a>) -> Result<Cow<'a, [u8]>>,
) -> Result<()> {
  let subsetted_entries = subset_glyf_entries(ctx, glyph_data_fn)?;

  let mut sub_glyf = Writer::new();
  let mut sub_loca = Writer::new();

  let mut write_offset = |offset: usize| {
    if ctx.long_loca {
      sub_loca.write::<u32>(offset as u32);
    } else {
      sub_loca.write::<u16>((offset / 2) as u16);
    }
  };

  for entry in &subsetted_entries {
    write_offset(sub_glyf.len());
    sub_glyf.extend(entry);

    if !ctx.long_loca {
      sub_glyf.align(2);
    }
  }

  // Write the final offset.
  write_offset(sub_glyf.len());

  ctx.push(Tag::LOCA, sub_loca.finish());
  ctx.push(Tag::GLYF, sub_glyf.finish());

  Ok(())
}

/// A glyf + loca table.
struct Table<'a> {
  loca: &'a [u8],
  glyf: &'a [u8],
  long: bool,
}

impl<'a> Table<'a> {
  fn new(face: &Face<'a>) -> Option<Self> {
    let loca = face.table(Tag::LOCA)?;
    let glyf = face.table(Tag::GLYF)?;
    let head = face.table(Tag::HEAD)?;

    let mut r = Reader::new_at(head, 50);
    let long = r.read::<i16>()? != 0;
    Some(Self { loca, glyf, long })
  }

  fn glyph_data(&self, id: u16) -> Option<&'a [u8]> {
    let read_offset = |n| {
      Some(if self.long {
        let mut r = Reader::new_at(self.loca, 4 * n);
        r.read::<u32>()? as usize
      } else {
        let mut r = Reader::new_at(self.loca, 2 * n);
        2 * r.read::<u16>()? as usize
      })
    };

    let from = read_offset(id as usize)?;
    let to = read_offset(id as usize + 1)?;
    self.glyf.get(from..to)
  }
}

fn subset_glyf_entries<'a>(
  ctx: &mut Context<'a>,
  mut glyph_data_fn: impl FnMut(u16, &Context<'a>) -> Result<Cow<'a, [u8]>>,
) -> Result<Vec<Cow<'a, [u8]>>> {
  let mut size = 0;
  let mut glyf_entries = vec![];

  for old_gid in ctx.mapper.remapped_gids() {
    let glyph_data = glyph_data_fn(old_gid, ctx)?;

    // Empty glyph.
    if glyph_data.is_empty() {
      glyf_entries.push(glyph_data);
      continue;
    }

    let mut r = Reader::new(&glyph_data);
    let num_contours = r.read::<i16>().ok_or(MalformedFont)?;

    let glyph_data = if num_contours < 0 {
      Cow::Owned(remap_component_glyph(&ctx.mapper, &glyph_data)?)
    } else {
      // Simple glyphs don't need any subsetting.
      glyph_data
    };

    let mut len = glyph_data.len();
    len += (len % 2 != 0) as usize;
    size += len;

    glyf_entries.push(glyph_data);
  }

  // Decide on the loca format.
  ctx.long_loca = size > 2 * (u16::MAX as usize);

  Ok(glyf_entries)
}

fn remap_component_glyph(mapper: &GlyphRemapper, data: &[u8]) -> Result<Vec<u8>> {
  let mut r = Reader::new(data);
  let mut w = Writer::with_capacity(data.len());

  // number of contours
  w.write(r.read::<i16>().ok_or(MalformedFont)?);

  // xMin, yMin, xMax, yMax
  w.write(r.read::<i16>().ok_or(MalformedFont)?);
  w.write(r.read::<i16>().ok_or(MalformedFont)?);
  w.write(r.read::<i16>().ok_or(MalformedFont)?);
  w.write(r.read::<i16>().ok_or(MalformedFont)?);

  const ARG_1_AND_2_ARE_WORDS: u16 = 0x0001;
  const WE_HAVE_A_SCALE: u16 = 0x0008;
  const MORE_COMPONENTS: u16 = 0x0020;
  const WE_HAVE_AN_X_AND_Y_SCALE: u16 = 0x0040;
  const WE_HAVE_A_TWO_BY_TWO: u16 = 0x0080;
  const WE_HAVE_INSTRUCTIONS: u16 = 0x0100;

  let mut done;

  loop {
    let flags = r.read::<u16>().ok_or(MalformedFont)?;
    w.write(flags);
    let old_component = r.read::<u16>().ok_or(MalformedFont)?;
    let new_component = mapper.get(old_component).ok_or(MalformedFont)?;
    w.write(new_component);

    if flags & ARG_1_AND_2_ARE_WORDS != 0 {
      w.write(r.read::<i16>().ok_or(MalformedFont)?);
      w.write(r.read::<i16>().ok_or(MalformedFont)?);
    } else {
      w.write(r.read::<u16>().ok_or(MalformedFont)?);
    }

    if flags & WE_HAVE_A_SCALE != 0 {
      w.write(r.read::<F2Dot14>().ok_or(MalformedFont)?);
    } else if flags & WE_HAVE_AN_X_AND_Y_SCALE != 0 {
      w.write(r.read::<F2Dot14>().ok_or(MalformedFont)?);
      w.write(r.read::<F2Dot14>().ok_or(MalformedFont)?);
    } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
      w.write(r.read::<F2Dot14>().ok_or(MalformedFont)?);
      w.write(r.read::<F2Dot14>().ok_or(MalformedFont)?);
      w.write(r.read::<F2Dot14>().ok_or(MalformedFont)?);
      w.write(r.read::<F2Dot14>().ok_or(MalformedFont)?);
    }

    done = flags & MORE_COMPONENTS == 0;

    if done {
      if flags & WE_HAVE_INSTRUCTIONS != 0 {
        w.write(r.tail().ok_or(MalformedFont)?);
      }

      break;
    }
  }

  Ok(w.finish())
}

/// Returns an iterator over the component glyphs of a glyph.
fn component_glyphs(glyph_data: &[u8]) -> Option<impl Iterator<Item = u16> + '_> {
  let mut r = Reader::new(glyph_data);

  // Number of contours
  r.read::<i16>()?;

  // xMin, yMin, xMax, yMax
  r.read::<i16>()?;
  r.read::<i16>()?;
  r.read::<i16>()?;
  r.read::<i16>()?;

  const ARG_1_AND_2_ARE_WORDS: u16 = 0x0001;
  const WE_HAVE_A_SCALE: u16 = 0x0008;
  const MORE_COMPONENTS: u16 = 0x0020;
  const WE_HAVE_AN_X_AND_Y_SCALE: u16 = 0x0040;
  const WE_HAVE_A_TWO_BY_TWO: u16 = 0x0080;

  let mut done = false;
  Some(std::iter::from_fn(move || {
    if done {
      return None;
    }

    let flags = r.read::<u16>()?;
    let component = r.read::<u16>()?;

    if flags & ARG_1_AND_2_ARE_WORDS != 0 {
      r.read::<i16>();
      r.read::<i16>();
    } else {
      r.read::<u16>();
    }

    if flags & WE_HAVE_A_SCALE != 0 {
      r.read::<F2Dot14>();
    } else if flags & WE_HAVE_AN_X_AND_Y_SCALE != 0 {
      r.read::<F2Dot14>();
      r.read::<F2Dot14>();
    } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
      r.read::<F2Dot14>();
      r.read::<F2Dot14>();
      r.read::<F2Dot14>();
      r.read::<F2Dot14>();
    }

    done = flags & MORE_COMPONENTS == 0;
    Some(component)
  }))
}
