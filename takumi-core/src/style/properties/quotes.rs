//! `quotes`, and the marks `quotes: auto` takes in each language.

use std::{fmt, sync::Arc};

use cssparser::{Parser, serialize_string};

use crate::style::{Animatable, FromCss, Lang, MakeComputed, ParseResult, ToCss};

/// `quotes`: the marks `open-quote` and `close-quote` write at each nesting depth.
/// <https://drafts.csswg.org/css-content-3/#quotes>
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub enum Quotes {
  /// The marks of the element's language, as Blink's `LayoutLocale::GetQuotesData` reads them
  /// from CLDR, or `“` `”` and `‘` `’` for a language CLDR has none for.
  #[default]
  Auto,
  /// No marks, though the quote keywords still move the depth.
  None,
  /// An opening and a closing mark for each depth, the last pair serving every deeper one.
  Pairs(Box<[(Arc<str>, Arc<str>)]>),
}

impl Quotes {
  /// The mark that opens, or closes when `open` is false, the quote at `depth` in `lang`.
  pub fn mark(&self, lang: Option<&Lang>, depth: usize, open: bool) -> &str {
    match self {
      Quotes::Auto => {
        let [open_1, close_1, open_2, close_2] = lang.map_or(BASIC_QUOTES, cldr_quotes);

        match (depth, open) {
          (0, true) => open_1,
          (0, false) => close_1,
          (_, true) => open_2,
          (_, false) => close_2,
        }
      }
      Quotes::None => "",
      Quotes::Pairs(pairs) => {
        pairs
          .get(depth.min(pairs.len().saturating_sub(1)))
          .map_or(
            "",
            |(opening, closing)| if open { opening } else { closing },
          )
      }
    }
  }
}

impl MakeComputed for Quotes {}

impl Animatable for Quotes {}

impl<'i> FromCss<'i> for Quotes {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("auto"))
      .is_ok()
    {
      return Ok(Quotes::Auto);
    }

    if input
      .try_parse(|input| input.expect_ident_matching("none"))
      .is_ok()
    {
      return Ok(Quotes::None);
    }

    let mut pairs = Vec::new();

    loop {
      let opening: Arc<str> = input.expect_string()?.as_ref().into();
      let closing: Arc<str> = input.expect_string()?.as_ref().into();

      pairs.push((opening, closing));

      if input.is_exhausted() {
        break;
      }
    }

    Ok(Quotes::Pairs(pairs.into_boxed_slice()))
  }

  const VALID_TOKENS: &'static [&'static str] = &["auto", "none", "<string> <string>"];
}

impl ToCss for Quotes {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    match self {
      Quotes::Auto => dest.write_str("auto"),
      Quotes::None => dest.write_str("none"),
      Quotes::Pairs(pairs) => {
        for (index, (opening, closing)) in pairs.iter().enumerate() {
          if index > 0 {
            dest.write_str(" ")?;
          }

          serialize_string(opening, dest)?;
          dest.write_str(" ")?;
          serialize_string(closing, dest)?;
        }

        Ok(())
      }
    }
  }
}

/// Blink's `BasicQuotesData`, for a language CLDR names no marks for.
const BASIC_QUOTES: [&str; 4] = ["\u{201C}", "\u{201D}", "\u{2018}", "\u{2019}"];

/// Blink's `LayoutLocale::GetQuotesData`: the marks of `lang` or, failing that, of the tag with
/// its last subtag dropped, until one has marks.
///
/// Approximate: Blink matches the locales its ICU build carries, where this matches every CLDR
/// locale `CLDR_QUOTES` holds.
fn cldr_quotes(lang: &Lang) -> [&'static str; 4] {
  let mut tag = lang.as_str();

  loop {
    let found = CLDR_QUOTES.iter().find(|(locale, _)| {
      locale.len() == tag.len()
        && locale.bytes().zip(tag.bytes()).all(|(locale, tag)| {
          locale == tag.to_ascii_lowercase() || (locale == b'_' && tag == b'-')
        })
    });

    if let Some((_, marks)) = found {
      return *marks;
    }

    let Some(end) = tag.rfind('-') else {
      return BASIC_QUOTES;
    };

    tag = &tag[..end];
  }
}

/// CLDR 49's quotation marks, the opening and closing marks and then the alternate pair, of each
/// locale whose marks differ from what its shorter tag finds. `scripts/quotes-data.mts` prints it.
const CLDR_QUOTES: &[(&str, [&str; 4])] = &[
  ("ady", ["«", "»", "‘", "’"]),
  ("agq", ["„", "”", "‚", "’"]),
  ("am", ["«", "»", "‹", "›"]),
  ("ar", ["”", "“", "’", "‘"]),
  ("ast", ["«", "»", "“", "”"]),
  ("az_cyrl", ["«", "»", "‹", "›"]),
  ("bas", ["«", "»", "„", "“"]),
  ("be", ["«", "»", "„", "“"]),
  ("bg", ["„", "“", "„", "“"]),
  ("blo", ["«", "»", "“", "”"]),
  ("bm", ["«", "»", "“", "”"]),
  ("br", ["«", "»", "“", "”"]),
  ("bs", ["„", "”", "‘", "’"]),
  ("bs_cyrl", ["„", "“", "‚", "‘"]),
  ("bua", ["«", "»", "„", "“"]),
  ("ca", ["«", "»", "“", "”"]),
  ("cs", ["„", "“", "‚", "‘"]),
  ("cv", ["«", "»", "“", "”"]),
  ("de", ["„", "“", "‚", "‘"]),
  ("dsb", ["„", "“", "‚", "‘"]),
  ("dua", ["«", "»", "‘", "’"]),
  ("dyo", ["«", "»", "“", "”"]),
  ("el", ["«", "»", "“", "”"]),
  ("el_polyton", ["«", "»", "‘", "’"]),
  ("eo", ["“", "”", "«", "»"]),
  ("es_us", ["«", "»", "“", "”"]),
  ("et", ["„", "“", "‚", "‘"]),
  ("eu", ["«", "»", "“", "”"]),
  ("ewo", ["«", "»", "“", "”"]),
  ("fa", ["«", "»", "‹", "›"]),
  ("ff", ["„", "”", "‚", "’"]),
  ("ff_adlm", ["“", "”", "‘", "’"]),
  ("fi", ["”", "”", "’", "’"]),
  ("fr", ["«", "»", "«", "»"]),
  ("fr_ca", ["«", "»", "”", "“"]),
  ("fr_ch", ["«", "»", "‹", "›"]),
  ("fur", ["‘", "’", "“", "”"]),
  ("gsw", ["«", "»", "‹", "›"]),
  ("he", ["”", "”", "’", "’"]),
  ("hr", ["„", "“", "‚", "‘"]),
  ("hsb", ["„", "“", "‚", "‘"]),
  ("hu", ["„", "”", "»", "«"]),
  ("hy", ["«", "»", "«", "»"]),
  ("ia", ["‘", "’", "“", "”"]),
  ("ie", ["«", "»", "“", "”"]),
  ("is", ["„", "“", "‚", "‘"]),
  ("it", ["«", "»", "“", "”"]),
  ("ja", ["「", "」", "『", "』"]),
  ("jgo", ["«", "»", "‹", "›"]),
  ("ka", ["„", "“", "«", "»"]),
  ("kab", ["«", "»", "“", "”"]),
  ("kbd", ["«", "»", "‘", "’"]),
  ("kk", ["«", "»", "“", "”"]),
  ("kk_arab", ["»", "«", "›", "‹"]),
  ("kkj", ["«", "»", "‹", "›"]),
  ("ksf", ["«", "»", "‘", "’"]),
  ("ksh", ["„", "“", "‚", "‘"]),
  ("ky", ["«", "»", "„", "“"]),
  ("lag", ["”", "”", "’", "’"]),
  ("lb", ["„", "“", "‚", "‘"]),
  ("lij", ["«", "»", "“", "”"]),
  ("lld", ["”", "“", "’", "‘"]),
  ("lt", ["„", "“", "„", "“"]),
  ("luy", ["„", "“", "‚", "‘"]),
  ("mg", ["«", "»", "“", "”"]),
  ("mk", ["„", "“", "‚", "‘"]),
  ("mua", ["«", "»", "“", "”"]),
  ("mzn", ["«", "»", "‹", "›"]),
  ("nb", ["«", "»", "‘", "’"]),
  ("nds", ["„", "“", "‚", "‘"]),
  ("nl", ["‘", "’", "‘", "’"]),
  ("nmg", ["„", "”", "«", "»"]),
  ("nn", ["«", "»", "‘", "’"]),
  ("nnh", ["«", "»", "“", "”"]),
  ("no", ["«", "»", "‘", "’"]),
  ("oc", ["«", "»", "«", "»"]),
  ("os", ["«", "»", "„", "“"]),
  ("pl", ["„", "”", "«", "»"]),
  ("pms", ["«", "»", "“", "”"]),
  ("prg", ["„", "“", "„", "“"]),
  ("pt_ao", ["«", "»", "“", "”"]),
  ("pt_ch", ["«", "»", "“", "”"]),
  ("pt_cv", ["«", "»", "“", "”"]),
  ("pt_gq", ["«", "»", "“", "”"]),
  ("pt_gw", ["«", "»", "“", "”"]),
  ("pt_lu", ["«", "»", "“", "”"]),
  ("pt_mo", ["«", "»", "“", "”"]),
  ("pt_mz", ["«", "»", "“", "”"]),
  ("pt_pt", ["«", "»", "“", "”"]),
  ("pt_st", ["«", "»", "“", "”"]),
  ("pt_tl", ["«", "»", "“", "”"]),
  ("rm", ["«", "»", "‹", "›"]),
  ("rn", ["”", "”", "’", "’"]),
  ("ro", ["„", "”", "«", "»"]),
  ("ru", ["«", "»", "„", "“"]),
  ("rw", ["«", "»", "‘", "’"]),
  ("sah", ["«", "»", "„", "“"]),
  ("sc", ["«", "»", "“", "”"]),
  ("se", ["”", "”", "’", "’"]),
  ("sg", ["«", "»", "“", "”"]),
  ("shi", ["«", "»", "„", "”"]),
  ("sk", ["„", "“", "‚", "‘"]),
  ("sl", ["„", "“", "‚", "‘"]),
  ("sn", ["”", "”", "’", "’"]),
  ("sr", ["„", "”", "’", "’"]),
  ("st", ["“", "’", "“", "”"]),
  ("sv", ["”", "”", "’", "’"]),
  ("syr", ["”", "“", "’", "‘"]),
  ("szl", ["„", "”", "»", "«"]),
  ("ti", ["«", "»", "“", "”"]),
  ("ti_er", ["‘", "’", "“", "”"]),
  ("tk", ["“", "”", "“", "”"]),
  ("tn", ["“", "”", "“", "”"]),
  ("tyv", ["«", "»", "„", "“"]),
  ("ug", ["»", "«", "›", "‹"]),
  ("uk", ["«", "»", "„", "“"]),
  ("ur", ["”", "“", "’", "‘"]),
  ("uz", ["“", "”", "’", "‘"]),
  ("uz_arab", ["“", "”", "‘", "’"]),
  ("uz_cyrl", ["“", "”", "‘", "’"]),
  ("wae", ["«", "»", "‹", "›"]),
  ("yav", ["«", "»", "«", "»"]),
  ("yi", ["”", "”", "’", "’"]),
  ("yue", ["「", "」", "『", "』"]),
  ("zgh", ["«", "»", "„", "”"]),
  ("zh_hant", ["「", "」", "『", "』"]),
];

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  fn lang(tag: &str) -> Lang {
    Lang::parse(tag).expect("valid tag")
  }

  #[test]
  fn parses_quotes() {
    assert_eq!(Quotes::from_css_str("auto"), Ok(Quotes::Auto));
    assert_eq!(Quotes::from_css_str("none"), Ok(Quotes::None));
    assert_eq!(
      Quotes::from_css_str("\"«\" \"»\" \"<\" \">\""),
      Ok(Quotes::Pairs(
        [("«".into(), "»".into()), ("<".into(), ">".into())].into()
      ))
    );
    assert!(Quotes::from_css_str("\"«\"").is_err());
  }

  #[test]
  fn deeper_quotes_take_the_last_pair() {
    let quotes = Quotes::from_css_str("\"P\" \"S\" \"A\" \"S\"").expect("parse");

    assert_eq!(quotes.mark(None, 0, true), "P");
    assert_eq!(quotes.mark(None, 1, true), "A");
    assert_eq!(quotes.mark(None, 5, false), "S");
    assert_eq!(Quotes::None.mark(None, 0, true), "");
  }

  #[test]
  fn auto_takes_the_marks_of_the_language() {
    assert_eq!(Quotes::Auto.mark(None, 1, false), "\u{2019}");
    assert_eq!(Quotes::Auto.mark(Some(&lang("de")), 0, true), "\u{201E}");
    assert_eq!(Quotes::Auto.mark(Some(&lang("ja")), 1, true), "\u{300E}");
    assert_eq!(Quotes::Auto.mark(Some(&lang("fr-CA")), 0, true), "\u{AB}");
    assert_eq!(
      Quotes::Auto.mark(Some(&lang("zh-Hant-TW")), 0, true),
      "\u{300C}"
    );
    assert_eq!(Quotes::Auto.mark(Some(&lang("de-AT")), 0, true), "\u{201E}");
    assert_eq!(Quotes::Auto.mark(Some(&lang("xx")), 0, true), "\u{201C}");
  }
}
