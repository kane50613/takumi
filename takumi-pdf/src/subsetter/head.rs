//! The `head` table mostly contains information that can be reused from the
//! old table, except for the `loca` format, which depends on the size of the
//! glyph data. The checksum will be recalculated in the very end.

use super::*;

pub fn subset(ctx: &mut Context) -> Result<()> {
  let mut head = ctx.expect_table(Tag::HEAD).ok_or(MalformedFont)?.to_vec();
  let index_to_loc = head.get_mut(50..52).ok_or(MalformedFont)?;
  index_to_loc[0] = 0;
  index_to_loc[1] = ctx.long_loca as u8;
  ctx.push(Tag::HEAD, head);
  Ok(())
}
