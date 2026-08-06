use crate::subsetter::Error::MalformedFont;
use crate::subsetter::interjector::Interjector;
use crate::subsetter::{Context, MaxpData, glyf};
use std::borrow::Cow;

/// CFF2 fonts will currently be converted into TTF fonts.
pub fn subset(ctx: &mut Context) -> crate::subsetter::Result<()> {
  let mut maxp_data = MaxpData::default();
  let mut hmtx_data = Vec::new();

  glyf::subset_with(ctx, |old_gid, ctx| {
    let data = match &ctx.interjector {
      // We reject CFF2 fonts earlier if `variable-fonts` feature is not enabled.
      Interjector::Dummy(_) => unreachable!(),
      Interjector::Skrifa(s) => {
        let (advance, lsb, data) = s.interject(&mut maxp_data, old_gid).ok_or(MalformedFont)?;
        hmtx_data.push((advance, lsb));
        Cow::Owned(data)
      }
    };

    Ok(data)
  })?;

  ctx.custom_maxp_data = Some(maxp_data);
  ctx.custom_hmtx_data = Some(hmtx_data);

  Ok(())
}
