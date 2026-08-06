//! The `hmtx` table contains the horizontal metrics for each glyph.
//! All we need to do is to rewrite the table so that it matches the
//! sequence of the new glyphs. A minor pain point is that the table
//! allows omitting the advance width for the last few glyphs if it is
//! the same. In order to keep the code simple, we do not keep this optimization
//! when rewriting the table.
//! While doing so, we also rewrite the `hhea` table, which contains
//! the number of glyphs that contain both, advance width and
//! left side bearing metrics.

// The parsing logic was taken from ttf-parser.

use super::*;
use crate::subsetter::Error::OverflowError;

pub fn subset(ctx: &mut Context) -> Result<()> {
  let hmtx = ctx.expect_table(Tag::HMTX).ok_or(MalformedFont)?;

  let new_metrics = {
    let mut new_metrics = vec![];

    match &ctx.interjector {
      Interjector::Dummy(_) => extract_metrics(hmtx, &mut new_metrics, ctx)?,
      Interjector::Skrifa(_) => {
        new_metrics = ctx.custom_hmtx_data.take().ok_or(Error::SubsetError)?
      }
    }

    new_metrics
  };

  // Find out the last index we need to include the advance width for.
  let mut last_advance_width_index =
    u16::try_from(new_metrics.len()).map_err(|_| OverflowError)? - 1;
  let last_advance_width = new_metrics[last_advance_width_index as usize].0;

  for gid in new_metrics.iter().rev().skip(1) {
    if gid.0 == last_advance_width {
      last_advance_width_index -= 1;
    } else {
      break;
    }
  }

  let mut sub_hmtx = Writer::new();

  for (index, metric) in new_metrics.iter().enumerate() {
    let index = u16::try_from(index).map_err(|_| OverflowError)?;
    if index <= last_advance_width_index {
      sub_hmtx.write::<u16>(metric.0);
    }

    sub_hmtx.write::<i16>(metric.1);
  }

  ctx.push(Tag::HMTX, sub_hmtx.finish());

  let hhea = ctx.expect_table(Tag::HHEA).ok_or(MalformedFont)?;
  let mut sub_hhea = Writer::new();
  sub_hhea.extend(hhea.get(..hhea.len() - 2).ok_or(MalformedFont)?);
  sub_hhea.write::<u16>(last_advance_width_index + 1);

  ctx.push(Tag::HHEA, sub_hhea.finish());

  Ok(())
}

fn extract_metrics(
  hmtx: &[u8],
  new_metrics: &mut Vec<(u16, i16)>,
  ctx: &mut Context,
) -> Result<()> {
  // Extract the number of horizontal metrics from the `hhea` table.
  let num_h_metrics = {
    let hhea = ctx.expect_table(Tag::HHEA).ok_or(MalformedFont)?;
    let mut r = Reader::new(hhea);
    r.skip_bytes(34);
    r.read::<u16>().ok_or(MalformedFont)?
  };

  let last_advance_width = {
    let index = 4 * num_h_metrics.checked_sub(1).ok_or(OverflowError)? as usize;
    let mut r = Reader::new(hmtx.get(index..).ok_or(MalformedFont)?);
    r.read::<u16>().ok_or(MalformedFont)?
  };

  for old_gid in ctx.mapper.remapped_gids() {
    let has_advance_width = old_gid < num_h_metrics;

    let offset = if has_advance_width {
      old_gid as usize * 4
    } else {
      let num_h_metrics = num_h_metrics as usize;
      num_h_metrics * 4 + (old_gid as usize - num_h_metrics) * 2
    };

    let mut r = Reader::new(hmtx.get(offset..).ok_or(MalformedFont)?);

    if has_advance_width {
      let adv = r.read::<u16>().ok_or(MalformedFont)?;
      let lsb = r.read::<i16>().ok_or(MalformedFont)?;
      new_metrics.push((adv, lsb));
    } else {
      new_metrics.push((last_advance_width, r.read::<i16>().ok_or(MalformedFont)?));
    }
  }

  Ok(())
}
