//! `@counter-style` formatting and substitution of the page-counter class hooks.

use takumi_core::layout::node::{Node, NodeKind};

const COUNTER_STYLES: [&str; 7] = [
  "decimal",
  "decimal-leading-zero",
  "lower-roman",
  "upper-roman",
  "cjk-decimal",
  "trad-chinese-informal",
  "cjk-ideographic",
];

/// Formats a page counter in a CSS `@counter-style` named style. Unknown
/// styles fall back to `decimal`.
fn format_counter(value: usize, style: &str) -> String {
  match style {
    "cjk-decimal" => value
      .to_string()
      .bytes()
      .map(|digit| CHINESE_DIGITS[usize::from(digit - b'0')])
      .collect(),
    // Blink defines cjk-ideographic as `extends trad-chinese-informal`.
    "trad-chinese-informal" | "cjk-ideographic" => chinese_informal(value),
    "lower-roman" => roman(value).to_ascii_lowercase(),
    "upper-roman" => roman(value),
    "decimal-leading-zero" => format!("{value:02}"),
    _ => value.to_string(),
  }
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

/// The counter value a node's class hooks request, if any: `pageNumber` or
/// `totalPages`, optionally paired with a `@counter-style` name — the same
/// contract as Chromium's print header/footer templates.
pub(crate) fn counter_text(node: &Node, page: usize, pages: usize) -> Option<String> {
  let classes = node.class_name()?;
  let value = if classes
    .split_whitespace()
    .any(|class| class == "pageNumber")
  {
    page
  } else if classes
    .split_whitespace()
    .any(|class| class == "totalPages")
  {
    pages
  } else {
    return None;
  };
  let style = classes
    .split_whitespace()
    .find(|class| COUNTER_STYLES.contains(class))
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
