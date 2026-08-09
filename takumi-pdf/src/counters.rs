//! `@counter-style` formatting and substitution of the page-counter class hooks.

use takumi_core::layout::node::{Node, NodeKind};

/// Styles that write a decimal number in another script's digits, zero first.
const DIGIT_STYLES: [(&str, [char; 10]); 20] = [
  ("cjk-decimal", CHINESE_DIGITS),
  (
    "arabic-indic",
    ['٠', '١', '٢', '٣', '٤', '٥', '٦', '٧', '٨', '٩'],
  ),
  (
    "bengali",
    ['০', '১', '২', '৩', '৪', '৫', '৬', '৭', '৮', '৯'],
  ),
  (
    "cambodian",
    ['០', '១', '២', '៣', '៤', '៥', '៦', '៧', '៨', '៩'],
  ),
  ("khmer", ['០', '១', '២', '៣', '៤', '៥', '៦', '៧', '៨', '៩']),
  (
    "devanagari",
    ['०', '१', '२', '३', '४', '५', '६', '७', '८', '९'],
  ),
  (
    "gujarati",
    ['૦', '૧', '૨', '૩', '૪', '૫', '૬', '૭', '૮', '૯'],
  ),
  (
    "gurmukhi",
    ['੦', '੧', '੨', '੩', '੪', '੫', '੬', '੭', '੮', '੯'],
  ),
  (
    "kannada",
    ['೦', '೧', '೨', '೩', '೪', '೫', '೬', '೭', '೮', '೯'],
  ),
  ("lao", ['໐', '໑', '໒', '໓', '໔', '໕', '໖', '໗', '໘', '໙']),
  (
    "malayalam",
    ['൦', '൧', '൨', '൩', '൪', '൫', '൬', '൭', '൮', '൯'],
  ),
  (
    "mongolian",
    ['᠐', '᠑', '᠒', '᠓', '᠔', '᠕', '᠖', '᠗', '᠘', '᠙'],
  ),
  (
    "myanmar",
    ['၀', '၁', '၂', '၃', '၄', '၅', '၆', '၇', '၈', '၉'],
  ),
  ("oriya", ['୦', '୧', '୨', '୩', '୪', '୫', '୬', '୭', '୮', '୯']),
  (
    "persian",
    ['۰', '۱', '۲', '۳', '۴', '۵', '۶', '۷', '۸', '۹'],
  ),
  ("urdu", ['۰', '۱', '۲', '۳', '۴', '۵', '۶', '۷', '۸', '۹']),
  ("tamil", ['௦', '௧', '௨', '௩', '௪', '௫', '௬', '௭', '௮', '௯']),
  ("telugu", ['౦', '౧', '౨', '౩', '౪', '౫', '౬', '౭', '౮', '౯']),
  ("thai", ['๐', '๑', '๒', '๓', '๔', '๕', '๖', '๗', '๘', '๙']),
  (
    "tibetan",
    ['༠', '༡', '༢', '༣', '༤', '༥', '༦', '༧', '༨', '༩'],
  ),
];

/// Styles that count through an alphabet: a, b, ..., z, aa, ab, and so on.
const ALPHABET_STYLES: [(&str, &str); 7] = [
  ("lower-alpha", "abcdefghijklmnopqrstuvwxyz"),
  ("lower-latin", "abcdefghijklmnopqrstuvwxyz"),
  ("upper-alpha", "ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
  ("upper-latin", "ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
  ("lower-greek", "αβγδεζηθικλμνξοπρστυφχψω"),
  (
    "hiragana",
    "あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめもやゆよらりるれろわゐゑをん",
  ),
  (
    "katakana",
    "アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヰヱヲン",
  ),
];

/// The class hooks a page counter is requested through.
const COUNTER_HOOKS: [&str; 2] = ["pageNumber", "totalPages"];

const OTHER_STYLES: [&str; 5] = [
  "decimal",
  "decimal-leading-zero",
  "lower-roman",
  "upper-roman",
  "trad-chinese-informal",
];

/// Whether a class names a counter style this renderer knows.
fn is_counter_style(name: &str) -> bool {
  name == "cjk-ideographic"
    || OTHER_STYLES.contains(&name)
    || DIGIT_STYLES.iter().any(|(style, _)| *style == name)
    || ALPHABET_STYLES.iter().any(|(style, _)| *style == name)
}

/// Formats a page counter in a CSS `@counter-style` named style. Unknown
/// styles fall back to `decimal`.
fn format_counter(value: usize, style: &str) -> String {
  if let Some((_, digits)) = DIGIT_STYLES.iter().find(|(name, _)| *name == style) {
    return value
      .to_string()
      .bytes()
      .map(|digit| digits[usize::from(digit - b'0')])
      .collect();
  }
  if let Some((_, alphabet)) = ALPHABET_STYLES.iter().find(|(name, _)| *name == style) {
    return alphabetic(value, alphabet);
  }

  match style {
    // Blink defines cjk-ideographic as `extends trad-chinese-informal`.
    "trad-chinese-informal" | "cjk-ideographic" => chinese_informal(value),
    "lower-roman" => roman(value).to_ascii_lowercase(),
    "upper-roman" => roman(value),
    "decimal-leading-zero" => format!("{value:02}"),
    _ => value.to_string(),
  }
}

/// Counts through `alphabet` the way a spreadsheet names its columns, where the
/// last letter carries no zero and so `z` is followed by `aa`.
fn alphabetic(value: usize, alphabet: &str) -> String {
  let letters: Vec<char> = alphabet.chars().collect();

  if value == 0 || letters.is_empty() {
    return value.to_string();
  }
  let mut remaining = value;
  let mut out = Vec::new();

  while remaining > 0 {
    remaining -= 1;
    out.push(letters[remaining % letters.len()]);
    remaining /= letters.len();
  }
  out.reverse();
  out.into_iter().collect()
}

const CHINESE_DIGITS: [char; 10] = ['零', '一', '二', '三', '四', '五', '六', '七', '八', '九'];

/// Reading-style Chinese numerals (一, 十二, 一百零三) up to 9999; larger
/// values fall back to positional digits.
fn chinese_informal(value: usize) -> String {
  if value >= 10_000 {
    return format_counter(value, "cjk-decimal");
  }
  if value == 0 {
    return CHINESE_DIGITS[0].to_string();
  }
  let mut out = String::new();
  let mut needs_zero = false;

  for (unit, name) in [
    (1000, Some('千')),
    (100, Some('百')),
    (10, Some('十')),
    (1, None),
  ] {
    let digit = value / unit % 10;

    if digit == 0 {
      needs_zero = !out.is_empty();
      continue;
    }
    if needs_zero {
      out.push(CHINESE_DIGITS[0]);
      needs_zero = false;
    }
    // 10-19 reads 十 not 一十.
    if !(unit == 10 && digit == 1 && value < 20) {
      out.push(CHINESE_DIGITS[digit]);
    }
    if let Some(name) = name {
      out.push(name);
    }
  }
  out
}

fn roman(value: usize) -> String {
  const NUMERALS: [(usize, &str); 13] = [
    (1000, "M"),
    (900, "CM"),
    (500, "D"),
    (400, "CD"),
    (100, "C"),
    (90, "XC"),
    (50, "L"),
    (40, "XL"),
    (10, "X"),
    (9, "IX"),
    (5, "V"),
    (4, "IV"),
    (1, "I"),
  ];
  let mut remaining = value;
  let mut out = String::new();

  for (unit, numeral) in NUMERALS {
    while remaining >= unit {
      out.push_str(numeral);
      remaining -= unit;
    }
  }
  out
}

/// Every character a counter in `style` can produce.
fn style_characters(style: &str) -> String {
  if let Some((_, digits)) = DIGIT_STYLES.iter().find(|(name, _)| *name == style) {
    return digits.iter().collect();
  }
  if let Some((_, alphabet)) = ALPHABET_STYLES.iter().find(|(name, _)| *name == style) {
    return (*alphabet).to_string();
  }

  match style {
    "lower-roman" => "ivxlcdm".to_string(),
    "upper-roman" => "IVXLCDM".to_string(),
    "trad-chinese-informal" | "cjk-ideographic" => {
      CHINESE_DIGITS.iter().collect::<String>() + "十百千"
    }
    _ => "0123456789".to_string(),
  }
}

/// The characters the page counters named by `classes` can produce, or nothing
/// when no class asks for a counter.
///
/// A counter's characters appear nowhere in the document, so a caller choosing
/// which faces to load has no other way to learn it needs, say, Thai digits.
pub fn counter_characters<'c>(classes: impl IntoIterator<Item = &'c str>) -> String {
  let mut hooked = false;
  let mut characters = String::new();

  for class in classes {
    if COUNTER_HOOKS.contains(&class) {
      hooked = true;
      continue;
    }
    if is_counter_style(class) {
      characters.push_str(&style_characters(class));
    }
  }

  if !hooked {
    return String::new();
  }
  // A hook without a style counts in decimal, and a band can hold both: a
  // plain `pageNumber` beside a `totalPages thai` still needs its own digits.
  style_characters("decimal") + &characters
}

/// The counter value a node's class hooks request, if any: `pageNumber` or
/// `totalPages`, optionally paired with a `@counter-style` name — the same
/// contract as Chromium's print header/footer templates.
pub(crate) fn counter_text(node: &Node, page: usize, pages: usize) -> Option<String> {
  let classes = node.class_name()?;
  let value = if classes
    .split_whitespace()
    .any(|class| class == COUNTER_HOOKS[0])
  {
    page
  } else if classes
    .split_whitespace()
    .any(|class| class == COUNTER_HOOKS[1])
  {
    pages
  } else {
    return None;
  };
  let style = classes
    .split_whitespace()
    .find(|class| is_counter_style(class))
    .unwrap_or("decimal");

  Some(format_counter(value, style))
}

/// Fills `pageNumber` / `totalPages` class hooks with the formatted counter,
/// like Chromium assigning `textContent` in its header/footer template.
pub(crate) fn substitute_page_counters(node: &mut Node, page: usize, pages: usize) {
  if let Some(text) = counter_text(node, page, pages) {
    match &mut node.kind {
      NodeKind::Text(data) => data.text = text,
      NodeKind::Container { children } => *children = vec![Node::text(text)],
      _ => {}
    }
    return;
  }
  if let NodeKind::Container { children } = &mut node.kind {
    for child in children {
      substitute_page_counters(child, page, pages);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn digit_styles_spell_the_number_in_their_own_script() {
    assert_eq!(format_counter(2026, "devanagari"), "२०२६");
    assert_eq!(format_counter(2026, "thai"), "๒๐๒๖");
    assert_eq!(format_counter(2026, "arabic-indic"), "٢٠٢٦");
    // `khmer` and `urdu` are the same digits under another name.
    assert_eq!(format_counter(7, "khmer"), format_counter(7, "cambodian"));
    assert_eq!(format_counter(7, "urdu"), format_counter(7, "persian"));
  }

  #[test]
  fn alphabet_styles_carry_past_their_last_letter() {
    assert_eq!(format_counter(1, "lower-alpha"), "a");
    assert_eq!(format_counter(26, "lower-alpha"), "z");
    assert_eq!(format_counter(27, "lower-alpha"), "aa");
    assert_eq!(format_counter(28, "upper-latin"), "AB");
    assert_eq!(format_counter(1, "lower-greek"), "α");
    assert_eq!(format_counter(1, "katakana"), "ア");
  }

  #[test]
  fn a_styled_counter_leaves_an_unstyled_one_its_digits() {
    let mixed = counter_characters(["pageNumber", "totalPages", "thai"]);

    assert!(mixed.contains('7'), "the plain counter lost its digits");
    assert!(mixed.contains('๗'), "the styled counter lost its digits");
    assert_eq!(
      counter_characters(["nav", "thai"]),
      "",
      "no counter, no characters"
    );
    assert_eq!(counter_characters(["pageNumber"]), "0123456789");
  }

  #[test]
  fn unknown_styles_count_in_decimal() {
    assert_eq!(format_counter(42, "no-such-style"), "42");
    assert!(!is_counter_style("no-such-style"));
    assert!(is_counter_style("tibetan"));
  }
}
