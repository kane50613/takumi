//! The `maxp` table contains the number of glyphs (and some additional information
//! depending on the version). All we need to do is rewrite the number of glyphs, the rest
//! can be copied from the old table.

use super::*;

pub fn subset(ctx: &mut Context) -> Result<()> {
  const POST_TRUETYPE_VERSION: u32 = 0x00010000;
  const POST_CFF_VERSION: u32 = 0x00005000;

  let maxp = ctx.expect_table(Tag::MAXP).ok_or(MalformedFont)?;
  let mut r = Reader::new(maxp);
  // Version
  let _ = r.read::<u32>().ok_or(MalformedFont)?;
  // number of glyphs
  r.read::<u16>().ok_or(MalformedFont)?;

  let version = match ctx.flavor {
    FontFlavor::TrueType => POST_TRUETYPE_VERSION,
    FontFlavor::Cff => POST_CFF_VERSION,
    // Since we convert to TrueType.
    FontFlavor::Cff2 => POST_TRUETYPE_VERSION,
  };

  let mut sub_maxp = Writer::new();
  sub_maxp.write::<u32>(version);
  sub_maxp.write::<u16>(ctx.mapper.num_gids());

  if version == POST_TRUETYPE_VERSION {
    if let Some(custom_data) = &ctx.custom_maxp_data {
      sub_maxp.write::<u16>(custom_data.max_points);
      sub_maxp.write::<u16>(custom_data.max_contours);
      sub_maxp.write::<u16>(custom_data.max_composite_points);
      sub_maxp.write::<u16>(custom_data.max_composite_contours);
      sub_maxp.write::<u16>(custom_data.max_zones);
      sub_maxp.write::<u16>(custom_data.max_twilight_points);
      sub_maxp.write::<u16>(custom_data.max_storage);
      sub_maxp.write::<u16>(custom_data.max_function_defs);
      sub_maxp.write::<u16>(custom_data.max_instruction_defs);
      sub_maxp.write::<u16>(custom_data.max_stack_elements);
      sub_maxp.write::<u16>(custom_data.max_size_of_instructions);
      sub_maxp.write::<u16>(custom_data.max_component_elements);
      sub_maxp.write::<u16>(custom_data.max_component_depth);
    } else {
      sub_maxp.extend(r.tail().ok_or(MalformedFont)?);
    }
  }

  ctx.push(Tag::MAXP, sub_maxp.finish());
  Ok(())
}

// When converting CFF2 to TTF, we need to write a version 1.0 MAXP table.
pub(crate) struct MaxpData {
  pub(crate) max_points: u16,
  pub(crate) max_contours: u16,
  max_composite_points: u16,
  max_composite_contours: u16,
  max_zones: u16,
  max_twilight_points: u16,
  max_storage: u16,
  max_function_defs: u16,
  max_instruction_defs: u16,
  max_stack_elements: u16,
  max_size_of_instructions: u16,
  max_component_elements: u16,
  max_component_depth: u16,
}

impl Default for MaxpData {
  fn default() -> Self {
    Self {
      max_points: 0,
      max_contours: 0,
      max_composite_points: 0,
      max_composite_contours: 0,
      max_zones: 1,
      max_twilight_points: 0,
      max_storage: 0,
      max_function_defs: 0,
      max_instruction_defs: 0,
      max_stack_elements: 0,
      max_size_of_instructions: 0,
      max_component_elements: 0,
      // Could probably be 0 too since we only use simple glyphs when converting?
      max_component_depth: 1,
    }
  }
}
