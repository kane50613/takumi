//! Character references, decoded as the JS `fromHtml` decodes them: HTML4's named references
//! plus `&apos;`, and numeric references, each ending in `;`.
//!
//! Approximate: the HTML tokenizer knows 2,231 names and decodes some without a `;`
//! (<https://html.spec.whatwg.org/multipage/named-characters.html>). Any other reference stays as
//! written.

use std::borrow::Cow;

/// Decodes the character references in `text`.
pub(crate) fn decode(text: &str) -> Cow<'_, str> {
  if !text.contains('&') {
    return Cow::Borrowed(text);
  }

  let mut out = String::with_capacity(text.len());
  let mut rest = text;

  while let Some(start) = rest.find('&') {
    out.push_str(&rest[..start]);
    rest = &rest[start..];

    match reference(rest) {
      Some((character, length)) => {
        out.push(character);
        rest = &rest[length..];
      }
      None => {
        out.push('&');
        rest = &rest[1..];
      }
    }
  }

  out.push_str(rest);
  Cow::Owned(out)
}

/// The character a reference at the start of `text` stands for, and the reference's length.
fn reference(text: &str) -> Option<(char, usize)> {
  let end = text
    .bytes()
    .take(MAX_REFERENCE_LENGTH)
    .position(|byte| byte == b';')?;
  let body = &text[1..end];
  let character = match body.strip_prefix('#') {
    Some(number) => {
      let code_point = match number.strip_prefix(['x', 'X']) {
        Some(hex) if !hex.is_empty() && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) => {
          u32::from_str_radix(hex, 16).ok()?
        }
        _ if !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()) => {
          number.parse().ok()?
        }
        _ => return None,
      };

      code_point_character(code_point)?
    }
    None => {
      let index = NAMED.binary_search_by(|(name, _)| (*name).cmp(body)).ok()?;

      NAMED[index].1
    }
  };

  Some((character, end + 1))
}

/// <https://html.spec.whatwg.org/multipage/parsing.html#numeric-character-reference-end-state>
fn code_point_character(code_point: u32) -> Option<char> {
  match code_point {
    0 => Some('\u{fffd}'),
    0x80..=0x9f => WINDOWS_1252_C1.chars().nth((code_point - 0x80) as usize),
    _ => char::from_u32(code_point),
  }
}

/// Bytes from `&` to `;` past which no reference this module decodes can end, bounding the
/// search for `;` after a stray `&`.
const MAX_REFERENCE_LENGTH: usize = 32;

/// The characters a C1 numeric reference resolves to, 0x80 to 0x9f.
const WINDOWS_1252_C1: &str = "€\u{81}‚ƒ„…†‡ˆ‰Š‹Œ\u{8d}Ž\u{8f}\u{90}‘’“”•–—˜™š›œ\u{9d}žŸ";

/// Sorted by name for binary search.
/// <https://html.spec.whatwg.org/entities.json>
const NAMED: [(&str, char); 253] = [
  ("AElig", '\u{c6}'),
  ("Aacute", '\u{c1}'),
  ("Acirc", '\u{c2}'),
  ("Agrave", '\u{c0}'),
  ("Alpha", '\u{391}'),
  ("Aring", '\u{c5}'),
  ("Atilde", '\u{c3}'),
  ("Auml", '\u{c4}'),
  ("Beta", '\u{392}'),
  ("Ccedil", '\u{c7}'),
  ("Chi", '\u{3a7}'),
  ("Dagger", '\u{2021}'),
  ("Delta", '\u{394}'),
  ("ETH", '\u{d0}'),
  ("Eacute", '\u{c9}'),
  ("Ecirc", '\u{ca}'),
  ("Egrave", '\u{c8}'),
  ("Epsilon", '\u{395}'),
  ("Eta", '\u{397}'),
  ("Euml", '\u{cb}'),
  ("Gamma", '\u{393}'),
  ("Iacute", '\u{cd}'),
  ("Icirc", '\u{ce}'),
  ("Igrave", '\u{cc}'),
  ("Iota", '\u{399}'),
  ("Iuml", '\u{cf}'),
  ("Kappa", '\u{39a}'),
  ("Lambda", '\u{39b}'),
  ("Mu", '\u{39c}'),
  ("Ntilde", '\u{d1}'),
  ("Nu", '\u{39d}'),
  ("OElig", '\u{152}'),
  ("Oacute", '\u{d3}'),
  ("Ocirc", '\u{d4}'),
  ("Ograve", '\u{d2}'),
  ("Omega", '\u{3a9}'),
  ("Omicron", '\u{39f}'),
  ("Oslash", '\u{d8}'),
  ("Otilde", '\u{d5}'),
  ("Ouml", '\u{d6}'),
  ("Phi", '\u{3a6}'),
  ("Pi", '\u{3a0}'),
  ("Prime", '\u{2033}'),
  ("Psi", '\u{3a8}'),
  ("Rho", '\u{3a1}'),
  ("Scaron", '\u{160}'),
  ("Sigma", '\u{3a3}'),
  ("THORN", '\u{de}'),
  ("Tau", '\u{3a4}'),
  ("Theta", '\u{398}'),
  ("Uacute", '\u{da}'),
  ("Ucirc", '\u{db}'),
  ("Ugrave", '\u{d9}'),
  ("Upsilon", '\u{3a5}'),
  ("Uuml", '\u{dc}'),
  ("Xi", '\u{39e}'),
  ("Yacute", '\u{dd}'),
  ("Yuml", '\u{178}'),
  ("Zeta", '\u{396}'),
  ("aacute", '\u{e1}'),
  ("acirc", '\u{e2}'),
  ("acute", '\u{b4}'),
  ("aelig", '\u{e6}'),
  ("agrave", '\u{e0}'),
  ("alefsym", '\u{2135}'),
  ("alpha", '\u{3b1}'),
  ("amp", '&'),
  ("and", '\u{2227}'),
  ("ang", '\u{2220}'),
  ("apos", '\''),
  ("aring", '\u{e5}'),
  ("asymp", '\u{2248}'),
  ("atilde", '\u{e3}'),
  ("auml", '\u{e4}'),
  ("bdquo", '\u{201e}'),
  ("beta", '\u{3b2}'),
  ("brvbar", '\u{a6}'),
  ("bull", '\u{2022}'),
  ("cap", '\u{2229}'),
  ("ccedil", '\u{e7}'),
  ("cedil", '\u{b8}'),
  ("cent", '\u{a2}'),
  ("chi", '\u{3c7}'),
  ("circ", '\u{2c6}'),
  ("clubs", '\u{2663}'),
  ("cong", '\u{2245}'),
  ("copy", '\u{a9}'),
  ("crarr", '\u{21b5}'),
  ("cup", '\u{222a}'),
  ("curren", '\u{a4}'),
  ("dArr", '\u{21d3}'),
  ("dagger", '\u{2020}'),
  ("darr", '\u{2193}'),
  ("deg", '\u{b0}'),
  ("delta", '\u{3b4}'),
  ("diams", '\u{2666}'),
  ("divide", '\u{f7}'),
  ("eacute", '\u{e9}'),
  ("ecirc", '\u{ea}'),
  ("egrave", '\u{e8}'),
  ("empty", '\u{2205}'),
  ("emsp", '\u{2003}'),
  ("ensp", '\u{2002}'),
  ("epsilon", '\u{3b5}'),
  ("equiv", '\u{2261}'),
  ("eta", '\u{3b7}'),
  ("eth", '\u{f0}'),
  ("euml", '\u{eb}'),
  ("euro", '\u{20ac}'),
  ("exist", '\u{2203}'),
  ("fnof", '\u{192}'),
  ("forall", '\u{2200}'),
  ("frac12", '\u{bd}'),
  ("frac14", '\u{bc}'),
  ("frac34", '\u{be}'),
  ("frasl", '\u{2044}'),
  ("gamma", '\u{3b3}'),
  ("ge", '\u{2265}'),
  ("gt", '>'),
  ("hArr", '\u{21d4}'),
  ("harr", '\u{2194}'),
  ("hearts", '\u{2665}'),
  ("hellip", '\u{2026}'),
  ("iacute", '\u{ed}'),
  ("icirc", '\u{ee}'),
  ("iexcl", '\u{a1}'),
  ("igrave", '\u{ec}'),
  ("image", '\u{2111}'),
  ("infin", '\u{221e}'),
  ("int", '\u{222b}'),
  ("iota", '\u{3b9}'),
  ("iquest", '\u{bf}'),
  ("isin", '\u{2208}'),
  ("iuml", '\u{ef}'),
  ("kappa", '\u{3ba}'),
  ("lArr", '\u{21d0}'),
  ("lambda", '\u{3bb}'),
  ("lang", '\u{27e8}'),
  ("laquo", '\u{ab}'),
  ("larr", '\u{2190}'),
  ("lceil", '\u{2308}'),
  ("ldquo", '\u{201c}'),
  ("le", '\u{2264}'),
  ("lfloor", '\u{230a}'),
  ("lowast", '\u{2217}'),
  ("loz", '\u{25ca}'),
  ("lrm", '\u{200e}'),
  ("lsaquo", '\u{2039}'),
  ("lsquo", '\u{2018}'),
  ("lt", '<'),
  ("macr", '\u{af}'),
  ("mdash", '\u{2014}'),
  ("micro", '\u{b5}'),
  ("middot", '\u{b7}'),
  ("minus", '\u{2212}'),
  ("mu", '\u{3bc}'),
  ("nabla", '\u{2207}'),
  ("nbsp", '\u{a0}'),
  ("ndash", '\u{2013}'),
  ("ne", '\u{2260}'),
  ("ni", '\u{220b}'),
  ("not", '\u{ac}'),
  ("notin", '\u{2209}'),
  ("nsub", '\u{2284}'),
  ("ntilde", '\u{f1}'),
  ("nu", '\u{3bd}'),
  ("oacute", '\u{f3}'),
  ("ocirc", '\u{f4}'),
  ("oelig", '\u{153}'),
  ("ograve", '\u{f2}'),
  ("oline", '\u{203e}'),
  ("omega", '\u{3c9}'),
  ("omicron", '\u{3bf}'),
  ("oplus", '\u{2295}'),
  ("or", '\u{2228}'),
  ("ordf", '\u{aa}'),
  ("ordm", '\u{ba}'),
  ("oslash", '\u{f8}'),
  ("otilde", '\u{f5}'),
  ("otimes", '\u{2297}'),
  ("ouml", '\u{f6}'),
  ("para", '\u{b6}'),
  ("part", '\u{2202}'),
  ("permil", '\u{2030}'),
  ("perp", '\u{22a5}'),
  ("phi", '\u{3c6}'),
  ("pi", '\u{3c0}'),
  ("piv", '\u{3d6}'),
  ("plusmn", '\u{b1}'),
  ("pound", '\u{a3}'),
  ("prime", '\u{2032}'),
  ("prod", '\u{220f}'),
  ("prop", '\u{221d}'),
  ("psi", '\u{3c8}'),
  ("quot", '"'),
  ("rArr", '\u{21d2}'),
  ("radic", '\u{221a}'),
  ("rang", '\u{27e9}'),
  ("raquo", '\u{bb}'),
  ("rarr", '\u{2192}'),
  ("rceil", '\u{2309}'),
  ("rdquo", '\u{201d}'),
  ("real", '\u{211c}'),
  ("reg", '\u{ae}'),
  ("rfloor", '\u{230b}'),
  ("rho", '\u{3c1}'),
  ("rlm", '\u{200f}'),
  ("rsaquo", '\u{203a}'),
  ("rsquo", '\u{2019}'),
  ("sbquo", '\u{201a}'),
  ("scaron", '\u{161}'),
  ("sdot", '\u{22c5}'),
  ("sect", '\u{a7}'),
  ("shy", '\u{ad}'),
  ("sigma", '\u{3c3}'),
  ("sigmaf", '\u{3c2}'),
  ("sim", '\u{223c}'),
  ("spades", '\u{2660}'),
  ("sub", '\u{2282}'),
  ("sube", '\u{2286}'),
  ("sum", '\u{2211}'),
  ("sup", '\u{2283}'),
  ("sup1", '\u{b9}'),
  ("sup2", '\u{b2}'),
  ("sup3", '\u{b3}'),
  ("supe", '\u{2287}'),
  ("szlig", '\u{df}'),
  ("tau", '\u{3c4}'),
  ("there4", '\u{2234}'),
  ("theta", '\u{3b8}'),
  ("thetasym", '\u{3d1}'),
  ("thinsp", '\u{2009}'),
  ("thorn", '\u{fe}'),
  ("tilde", '\u{2dc}'),
  ("times", '\u{d7}'),
  ("trade", '\u{2122}'),
  ("uArr", '\u{21d1}'),
  ("uacute", '\u{fa}'),
  ("uarr", '\u{2191}'),
  ("ucirc", '\u{fb}'),
  ("ugrave", '\u{f9}'),
  ("uml", '\u{a8}'),
  ("upsih", '\u{3d2}'),
  ("upsilon", '\u{3c5}'),
  ("uuml", '\u{fc}'),
  ("weierp", '\u{2118}'),
  ("xi", '\u{3be}'),
  ("yacute", '\u{fd}'),
  ("yen", '\u{a5}'),
  ("yuml", '\u{ff}'),
  ("zeta", '\u{3b6}'),
  ("zwj", '\u{200d}'),
  ("zwnj", '\u{200c}'),
];
