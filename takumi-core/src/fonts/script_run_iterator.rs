//! Blink's [`ScriptRunIterator`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/script_run_iterator.cc).

use std::{collections::VecDeque, mem::swap};

use icu_properties::{
  CodePointMapData,
  props::{BidiMirroringGlyph, BidiPairedBracketType, EastAsianWidth, Script},
  script::ScriptWithExtensions,
};

use crate::text::character::LEFT_CORNER_BRACKET;

/// Blink's `kMaxUnicodeScriptExtensions`.
pub(crate) const MAX_UNICODE_SCRIPT_EXTENSIONS: usize = 23;
/// Blink's `kMaxScriptCount`.
pub(crate) const MAX_SCRIPT_COUNT: usize = MAX_UNICODE_SCRIPT_EXTENSIONS + 1;
/// Blink's `kMaxBrackets`.
const MAX_BRACKETS: usize = 32;

/// Blink's `UScriptCodeList`.
pub(crate) type ScriptCodeList = Vec<Script>;

/// Blink's `ScriptData::PairedBracketType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PairedBracketType {
  None,
  Open,
  Close,
}

/// Blink's `ScriptData`.
pub(crate) trait ScriptData {
  /// Blink's `GetScripts`.
  fn get_scripts(&self, ch: char, dst: &mut ScriptCodeList);

  /// Blink's `GetPairedBracket`.
  fn get_paired_bracket(&self, ch: char) -> char;

  /// Blink's `GetPairedBracketType`.
  fn get_paired_bracket_type(&self, ch: char) -> PairedBracketType;
}

/// Blink's `ICUScriptData`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct IcuScriptData;

impl IcuScriptData {
  /// Blink's `GetScriptForOpenType`.
  fn get_script_for_open_type(ch: char) -> Script {
    match CodePointMapData::<Script>::new().get(ch) {
      Script::Katakana | Script::KatakanaOrHiragana => Script::Hiragana,
      script => script,
    }
  }

  /// The script extensions of `ch`, as many as `MAX_SCRIPT_COUNT - 1`.
  fn script_extensions(ch: char, dst: &mut ScriptCodeList) {
    dst.clear();
    dst.extend(
      ScriptWithExtensions::new()
        .get_script_extensions_val(ch)
        .iter()
        .take(MAX_SCRIPT_COUNT - 1),
    );
  }
}

impl ScriptData for IcuScriptData {
  fn get_scripts(&self, ch: char, dst: &mut ScriptCodeList) {
    let primary_script = Self::get_script_for_open_type(ch);

    if primary_script == Script::Hiragana {
      dst.clear();
      dst.push(primary_script);

      return;
    }

    Self::script_extensions(ch, dst);

    if primary_script == dst[0] {
      return;
    }

    if primary_script != Script::Inherited && primary_script != Script::Common {
      match dst[1..].iter().position(|&script| script == primary_script) {
        Some(index) => dst.swap(0, index + 1),
        None => {
          dst.push(primary_script);

          let last = dst.len() - 1;

          dst.swap(0, last);
        }
      }

      return;
    }

    if primary_script == Script::Common {
      if dst.len() == 1 {
        dst.insert(0, primary_script);

        return;
      }

      for i in 1..dst.len() {
        if dst[0] == Script::Latin || dst[i] < dst[0] {
          dst.swap(0, i);
        }
      }

      return;
    }

    dst.push(dst[0]);
    dst[0] = primary_script;

    for i in 2..dst.len() {
      if dst[1] == Script::Latin || dst[i] < dst[1] {
        dst.swap(1, i);
      }
    }
  }

  fn get_paired_bracket(&self, ch: char) -> char {
    let glyph = CodePointMapData::<BidiMirroringGlyph>::new().get(ch);

    match glyph.paired_bracket_type {
      BidiPairedBracketType::None => ch,
      _ => glyph.mirroring_glyph.unwrap_or(ch),
    }
  }

  fn get_paired_bracket_type(&self, ch: char) -> PairedBracketType {
    match CodePointMapData::<BidiMirroringGlyph>::new()
      .get(ch)
      .paired_bracket_type
    {
      BidiPairedBracketType::Open => PairedBracketType::Open,
      BidiPairedBracketType::Close => PairedBracketType::Close,
      _ => PairedBracketType::None,
    }
  }
}

/// Blink's `IsHanScript`.
fn is_han_script(script: Script) -> bool {
  matches!(script, Script::Han | Script::Hiragana | Script::Bopomofo)
}

/// Blink's `FirstHanScript`.
fn first_han_script(list: &[Script]) -> Option<Script> {
  list.iter().copied().find(|&script| is_han_script(script))
}

/// Blink's `FixScriptsByEastAsianWidth`.
fn fix_scripts_by_east_asian_width(ch: char, set: &mut ScriptCodeList) {
  debug_assert!(!set.is_empty());

  if set.len() > 1 || set[0] != Script::Common {
    return;
  }

  let east_asian_width = CodePointMapData::<EastAsianWidth>::new().get(ch);

  if matches!(
    east_asian_width,
    EastAsianWidth::Wide | EastAsianWidth::Fullwidth | EastAsianWidth::Halfwidth
  ) {
    IcuScriptData::script_extensions(LEFT_CORNER_BRACKET, set);
  }
}

/// Blink's `BracketRec`.
#[derive(Debug, Clone, Copy)]
struct BracketRec {
  ch: char,
  script: Script,
}

/// Blink's `ScriptRunIterator`.
pub(crate) struct ScriptRunIterator<'a, D: ScriptData = IcuScriptData> {
  text: &'a str,
  brackets: VecDeque<BracketRec>,
  brackets_fixup_depth: usize,
  current_set: ScriptCodeList,
  next_set: ScriptCodeList,
  ahead_set: ScriptCodeList,
  ahead_character: char,
  ahead_pos: usize,
  common_preferred: Script,
  script_data: D,
}

impl<'a> ScriptRunIterator<'a> {
  pub(crate) fn new(text: &'a str) -> Self {
    Self::with_script_data(text, IcuScriptData)
  }
}

impl<'a, D: ScriptData> ScriptRunIterator<'a, D> {
  pub(crate) fn with_script_data(text: &'a str, script_data: D) -> Self {
    let mut iterator = Self {
      text,
      brackets: VecDeque::new(),
      brackets_fixup_depth: 0,
      current_set: Vec::with_capacity(MAX_SCRIPT_COUNT),
      next_set: Vec::with_capacity(MAX_SCRIPT_COUNT),
      ahead_set: Vec::with_capacity(MAX_SCRIPT_COUNT),
      ahead_character: '\0',
      ahead_pos: 0,
      common_preferred: Script::Common,
      script_data,
    };

    if let Some(ch) = text.chars().next() {
      iterator.current_set.push(Script::Common);
      iterator.ahead_character = ch;
      iterator.ahead_pos = ch.len_utf8();
      iterator
        .script_data
        .get_scripts(ch, &mut iterator.ahead_set);
    }

    iterator
  }

  /// Blink's `Consume`.
  pub(crate) fn consume(&mut self) -> Option<(usize, Script)> {
    if self.current_set.is_empty() {
      return None;
    }

    while let Some((pos, ch)) = self.fetch() {
      let paired_type = self.script_data.get_paired_bracket_type(ch);

      match paired_type {
        PairedBracketType::Open => self.open_bracket(ch),
        PairedBracketType::Close => self.close_bracket(ch),
        PairedBracketType::None => {}
      }

      if !self.merge_sets() {
        let script = self.resolve_current_script();

        self.fixup_stack(script, paired_type == PairedBracketType::Open);
        self.current_set.clone_from(&self.next_set);

        return Some((pos, script));
      }
    }

    let script = self.resolve_current_script();

    self.current_set.clear();

    Some((self.text.len(), script))
  }

  /// Blink's `OpenBracket`.
  fn open_bracket(&mut self, ch: char) {
    if self.brackets.len() == MAX_BRACKETS {
      self.brackets.pop_front();

      if self.brackets_fixup_depth == MAX_BRACKETS {
        self.brackets_fixup_depth -= 1;
      }
    }

    fix_scripts_by_east_asian_width(ch, &mut self.next_set);
    self.brackets.push_back(BracketRec {
      ch,
      script: Script::Common,
    });
    self.brackets_fixup_depth += 1;
  }

  /// Blink's `CloseBracket`.
  fn close_bracket(&mut self, ch: char) {
    let target = self.script_data.get_paired_bracket(ch);
    let Some(index) = self
      .brackets
      .iter()
      .rposition(|bracket| bracket.ch == target)
    else {
      return;
    };
    let mut script = self.brackets[index].script;

    if is_han_script(script)
      && let Some(current_han_script) = first_han_script(&self.current_set)
    {
      script = current_han_script;
    }

    if script != Script::Common {
      self.next_set.clear();
      self.next_set.push(script);
    }

    let num_popped = self.brackets.len() - 1 - index;

    self.brackets.truncate(index + 1);
    self.brackets_fixup_depth = self.brackets_fixup_depth.saturating_sub(num_popped);
  }

  /// Blink's `MergeSets`.
  fn merge_sets(&mut self) -> bool {
    if self.next_set.is_empty() || self.current_set.is_empty() {
      return false;
    }

    let mut priority_script = self.current_set[0];

    if self.next_set[0] <= Script::Inherited {
      if self.next_set.len() == 2
        && priority_script <= Script::Inherited
        && self.common_preferred == Script::Common
      {
        self.common_preferred = self.next_set[1];
      }

      return true;
    }

    if priority_script <= Script::Inherited {
      self.current_set.clone_from(&self.next_set);

      return true;
    }

    let mut have_priority = self.next_set.contains(&priority_script);

    if self.current_set.len() == 1 {
      return have_priority;
    }

    let mut next_start = 0;

    if !have_priority {
      priority_script = self.next_set[0];
      next_start = 1;
      have_priority = self.current_set[1..].contains(&priority_script);
    }

    // Writes never pass reads, so the set narrows in place.
    let mut written = 0;

    if have_priority {
      self.current_set[written] = priority_script;
      written += 1;
    }

    if next_start < self.next_set.len() {
      for read in 1..self.current_set.len() {
        let script = self.current_set[read];

        if self.next_set[next_start..].contains(&script) {
          self.current_set[written] = script;
          written += 1;
        }
      }
    }

    if written > 0 {
      self.current_set.truncate(written);

      return true;
    }

    false
  }

  /// Blink's `FixupStack`.
  fn fixup_stack(&mut self, resolved_script: Script, exclude_last: bool) {
    let mut count = self.brackets_fixup_depth;

    if count == 0 {
      return;
    }

    debug_assert!(count <= self.brackets.len());
    count = count.min(self.brackets.len());

    let skip = usize::from(exclude_last);

    count -= skip;
    self.brackets_fixup_depth = skip;

    for bracket in self.brackets.iter_mut().rev().skip(skip).take(count) {
      bracket.script = resolved_script;
    }
  }

  /// Blink's `Fetch`.
  fn fetch(&mut self) -> Option<(usize, char)> {
    if self.ahead_pos > self.text.len() {
      return None;
    }

    let pos = self.ahead_pos - self.ahead_character.len_utf8();
    let ch = self.ahead_character;

    swap(&mut self.next_set, &mut self.ahead_set);

    if self.ahead_pos == self.text.len() {
      self.ahead_pos += 1;

      return Some((pos, ch));
    }

    self.fetch_next_character().then_some((pos, ch))
  }

  /// Blink's `FetchNextCharacter`.
  fn fetch_next_character(&mut self) -> bool {
    let Some(ch) = self.text[self.ahead_pos..].chars().next() else {
      return false;
    };

    self.ahead_character = ch;
    self.ahead_pos += ch.len_utf8();
    self.script_data.get_scripts(ch, &mut self.ahead_set);

    if self.ahead_set.is_empty() {
      return false;
    }

    if self.ahead_set[0] == Script::Inherited && self.ahead_set.len() > 1 {
      if self.next_set[0] == Script::Common {
        self.next_set.clear();
        self.next_set.extend_from_slice(&self.ahead_set[1..]);
      }

      self.ahead_set.truncate(1);
    }

    true
  }

  /// Blink's `ResolveCurrentScript`.
  fn resolve_current_script(&self) -> Script {
    match self.current_set[0] {
      Script::Common => self.common_preferred,
      script => script,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Blink's `MockScriptData`.
  struct MockScriptData;

  impl MockScriptData {
    const CHAR_MIN: u32 = 0xE000;
    const LATIN: u32 = 1;
    const HAN: u32 = 2;
    const GREEK: u32 = 3;
    const CODE_LIST_INDEX_MASK: u32 = 0xF;
    const CODE_SPECIAL_MASK: u32 = 0x30;
    const CODE_SPECIAL_COMMON: u32 = 0x10;
    const CODE_SPECIAL_INHERITED: u32 = 0x20;
    const CODE_BRACKET_CLOSE_BIT: u32 = 0x40;
    const CODE_BRACKET_BIT: u32 = 0x80;
    const CODE_SQUARE_BRACKET_BIT: u32 = 0x100;
    const LIST_SHIFT: u32 = 2;
    const LIST_MASK: u32 = 0x3;
    const BRACKET_DELTA: u32 = Self::CODE_BRACKET_CLOSE_BIT;
    const TABLE: [u32; 16] = {
      let (l2, h2, g2) = (Self::LATIN << 2, Self::HAN << 2, Self::GREEK << 2);
      let (l3, h3, g3) = (Self::LATIN << 4, Self::HAN << 4, Self::GREEK << 4);

      [
        0,
        Self::LATIN,
        Self::HAN,
        Self::GREEK,
        l2 + Self::HAN,
        l2 + Self::GREEK,
        h2 + Self::LATIN,
        h2 + Self::GREEK,
        g2 + Self::LATIN,
        g2 + Self::HAN,
        l3 + h2 + Self::GREEK,
        l3 + g2 + Self::HAN,
        h3 + l2 + Self::GREEK,
        h3 + g2 + Self::LATIN,
        g3 + l2 + Self::HAN,
        g3 + h2 + Self::LATIN,
      ]
    };

    fn code(ch: char) -> u32 {
      u32::from(ch) - Self::CHAR_MIN
    }

    fn table_lookup(value: u32) -> u32 {
      Self::TABLE
        .iter()
        .position(|&entry| entry == value)
        .unwrap() as u32
    }

    fn character(code: u32) -> char {
      char::from_u32(Self::CHAR_MIN + code).unwrap()
    }

    /// Blink's `ToTestString`.
    fn to_test_string(input: &str) -> String {
      let mut result = String::new();
      let mut in_set = false;
      let mut code = 0;
      let mut list = 0;
      let mut current_shift = 0;

      for c in input.chars() {
        if in_set {
          match c {
            '(' => code |= Self::CODE_BRACKET_BIT,
            '[' => code |= Self::CODE_BRACKET_BIT | Self::CODE_SQUARE_BRACKET_BIT,
            ')' => code |= Self::CODE_BRACKET_BIT | Self::CODE_BRACKET_CLOSE_BIT,
            ']' => {
              code |= Self::CODE_BRACKET_BIT
                | Self::CODE_SQUARE_BRACKET_BIT
                | Self::CODE_BRACKET_CLOSE_BIT;
            }
            'i' => code |= Self::CODE_SPECIAL_INHERITED,
            'c' => code |= Self::CODE_SPECIAL_COMMON,
            'l' | 'h' | 'g' => {
              let script = match c {
                'l' => Self::LATIN,
                'h' => Self::HAN,
                _ => Self::GREEK,
              };

              list |= script << (2 * current_shift);
              current_shift += 1;
            }
            '>' => {
              code |= Self::table_lookup(list);
              result.push(Self::character(code));
              in_set = false;
            }
            _ => panic!("illegal mock string set char {c:?}"),
          }

          continue;
        }

        code = match c {
          '<' => {
            code = 0;
            list = 0;
            current_shift = 0;
            in_set = true;

            continue;
          }
          '(' => Self::CODE_BRACKET_BIT | Self::CODE_SPECIAL_COMMON,
          '[' => Self::CODE_BRACKET_BIT | Self::CODE_SQUARE_BRACKET_BIT | Self::CODE_SPECIAL_COMMON,
          ')' => Self::CODE_BRACKET_BIT | Self::CODE_BRACKET_CLOSE_BIT | Self::CODE_SPECIAL_COMMON,
          ']' => {
            Self::CODE_BRACKET_BIT
              | Self::CODE_SQUARE_BRACKET_BIT
              | Self::CODE_BRACKET_CLOSE_BIT
              | Self::CODE_SPECIAL_COMMON
          }
          'i' => Self::CODE_SPECIAL_INHERITED,
          'c' => Self::CODE_SPECIAL_COMMON,
          'l' => Self::LATIN,
          'h' => Self::HAN,
          'g' => Self::GREEK,
          '?' => 0,
          _ => panic!("illegal mock string char {c:?}"),
        };
        result.push(Self::character(code));
      }

      result
    }
  }

  impl ScriptData for MockScriptData {
    fn get_scripts(&self, ch: char, dst: &mut ScriptCodeList) {
      let code = Self::code(ch);

      dst.clear();

      match code & Self::CODE_SPECIAL_MASK {
        Self::CODE_SPECIAL_COMMON => dst.push(Script::Common),
        Self::CODE_SPECIAL_INHERITED => dst.push(Script::Inherited),
        _ => {}
      }

      let mut list_bits = Self::TABLE[(code & Self::CODE_LIST_INDEX_MASK) as usize];

      if dst.is_empty() && list_bits == 0 {
        dst.push(Script::Unknown);

        return;
      }

      while list_bits != 0 {
        match list_bits & Self::LIST_MASK {
          Self::LATIN => dst.push(Script::Latin),
          Self::HAN => dst.push(Script::Han),
          Self::GREEK => dst.push(Script::Greek),
          _ => {}
        }

        list_bits >>= Self::LIST_SHIFT;
      }
    }

    fn get_paired_bracket(&self, ch: char) -> char {
      match self.get_paired_bracket_type(ch) {
        PairedBracketType::Close => Self::character(Self::code(ch) - Self::BRACKET_DELTA),
        PairedBracketType::Open => Self::character(Self::code(ch) + Self::BRACKET_DELTA),
        PairedBracketType::None => ch,
      }
    }

    fn get_paired_bracket_type(&self, ch: char) -> PairedBracketType {
      let code = Self::code(ch);

      if code & Self::CODE_BRACKET_BIT == 0 {
        PairedBracketType::None
      } else if code & Self::CODE_BRACKET_CLOSE_BIT != 0 {
        PairedBracketType::Close
      } else {
        PairedBracketType::Open
      }
    }
  }

  fn verify_runs<D: ScriptData>(
    mut iterator: ScriptRunIterator<'_, D>,
    expect: &[(usize, Script)],
  ) {
    let mut actual = Vec::new();

    while let Some(run) = iterator.consume() {
      actual.push(run);
    }

    assert_eq!(actual, expect);
  }

  fn check_runs(runs: &[(&str, Script)]) {
    let mut text = String::new();
    let mut expect = Vec::new();

    for &(run, script) in runs {
      text.push_str(run);
      expect.push((text.len(), script));
    }

    verify_runs(ScriptRunIterator::new(&text), &expect);
  }

  fn check_mock_runs(runs: &[(&str, Script)]) {
    let mut text = String::new();
    let mut expect = Vec::new();

    for &(run, script) in runs {
      text.push_str(&MockScriptData::to_test_string(run));
      expect.push((text.len(), script));
    }

    verify_runs(
      ScriptRunIterator::with_script_data(&text, MockScriptData),
      &expect,
    );
  }

  #[test]
  fn empty() {
    assert_eq!(ScriptRunIterator::new("").consume(), None);
  }

  #[test]
  fn whitespace() {
    check_runs(&[(" \t ", Script::Common)]);
  }

  #[test]
  fn common() {
    check_runs(&[(" ... !?", Script::Common)]);
  }

  #[test]
  fn combining_circle() {
    check_runs(&[("◌́◌̀◌̈◌̂◌̄◌̊", Script::Latin)]);
  }

  #[test]
  fn latin() {
    check_runs(&[("latin", Script::Latin)]);
  }

  #[test]
  fn chinese() {
    check_runs(&[("萬國碼", Script::Han)]);
  }

  #[test]
  fn japanese_mixed_script() {
    let cases = [
      ("あ", Script::Hiragana),
      ("ア", Script::Hiragana),
      ("\u{30FC}", Script::Hiragana),
      ("\u{303C}", Script::Han),
      ("\u{3003}", Script::Bopomofo),
      ("\u{3001}", Script::Bopomofo),
    ];

    for (string, script) in cases {
      check_runs(&[(string, script)]);

      for text in [
        format!("か{string}"),
        format!("{string}か"),
        format!("カ{string}"),
        format!("{string}カ"),
        format!("か{string}カ"),
        format!("カ{string}か"),
      ] {
        check_runs(&[(&text, Script::Hiragana)]);
      }
    }
  }

  #[test]
  fn unbalanced_parens_1() {
    check_runs(&[
      ("(萬", Script::Han),
      ("a]", Script::Latin),
      (")", Script::Han),
    ]);
  }

  #[test]
  fn unbalanced_parens_2() {
    check_runs(&[
      ("(萬", Script::Han),
      ("a[", Script::Latin),
      (")]", Script::Han),
    ]);
  }

  #[test]
  fn latin_han() {
    check_runs(&[("Unicode ", Script::Latin), ("萬國碼", Script::Han)]);
  }

  #[test]
  fn han_latin() {
    check_runs(&[("萬國碼 ", Script::Han), ("Unicode", Script::Latin)]);
  }

  #[test]
  fn paren_empty_paren() {
    check_runs(&[("()", Script::Common)]);
  }

  #[test]
  fn paren_chinese_paren() {
    check_runs(&[("(萬國碼)", Script::Han)]);
  }

  #[test]
  fn paren_latin_paren() {
    check_runs(&[("(Unicode)", Script::Latin)]);
  }

  #[test]
  fn latin_paren_chinese_paren() {
    check_runs(&[
      ("Unicode (", Script::Latin),
      ("萬國碼", Script::Han),
      (")", Script::Latin),
    ]);
  }

  #[test]
  fn paren_chinese_paren_latin() {
    check_runs(&[("(萬國碼) ", Script::Han), ("Unicode", Script::Latin)]);
  }

  #[test]
  fn quote_paren_chinese_paren_latin_quote() {
    check_runs(&[("\"(萬國碼) ", Script::Han), ("Unicode\"", Script::Latin)]);
  }

  #[test]
  fn cjk_consecutive_parens_1() {
    check_runs(&[("「あ", Script::Hiragana), ("国。」", Script::Han)]);
  }

  #[test]
  fn cjk_consecutive_parens_2() {
    check_runs(&[("あ「あ", Script::Hiragana), ("国（国）」", Script::Han)]);
  }

  #[test]
  fn cjk_consecutive_parens_3() {
    check_runs(&[
      ("国「国", Script::Han),
      ("ア（", Script::Hiragana),
      ("A", Script::Latin),
      ("）」", Script::Hiragana),
    ]);
  }

  #[test]
  fn cjk_consecutive_parens_4() {
    check_runs(&[
      ("A", Script::Latin),
      ("「", Script::Bopomofo),
      ("A", Script::Latin),
      ("あ（", Script::Hiragana),
      ("国）」", Script::Han),
    ]);
  }

  #[test]
  fn cjk_consecutive_parens_5() {
    check_runs(&[
      ("「あ", Script::Hiragana),
      ("国", Script::Han),
      ("A", Script::Latin),
      ("」", Script::Hiragana),
    ]);
  }

  #[test]
  fn cjk_consecutive_parens_6() {
    check_runs(&[
      ("A", Script::Latin),
      ("「", Script::Bopomofo),
      ("A", Script::Latin),
      ("あ（", Script::Hiragana),
      ("国）", Script::Han),
      ("A", Script::Latin),
      ("」", Script::Bopomofo),
    ]);
  }

  #[test]
  fn cjk_consecutive_parens_7() {
    check_runs(&[("「あ", Script::Hiragana), ("国1」", Script::Han)]);
  }

  #[test]
  fn cjk_consecutive_parens_8() {
    check_runs(&[
      ("A", Script::Latin),
      ("「", Script::Bopomofo),
      ("A", Script::Latin),
      ("あ（", Script::Hiragana),
      ("国）1」", Script::Han),
    ]);
  }

  #[test]
  fn cjk_consecutive_parens_9() {
    check_runs(&[
      ("「あ", Script::Hiragana),
      ("国", Script::Han),
      ("A1", Script::Latin),
      ("」", Script::Hiragana),
    ]);
  }

  #[test]
  fn cjk_consecutive_parens_10() {
    check_runs(&[
      ("A", Script::Latin),
      ("「", Script::Bopomofo),
      ("A", Script::Latin),
      ("あ（", Script::Hiragana),
      ("国）", Script::Han),
      ("A1", Script::Latin),
      ("」", Script::Bopomofo),
    ]);
  }

  #[test]
  fn cjk_consecutive_parens_latin_1() {
    check_runs(&[
      ("「", Script::Bopomofo),
      ("A", Script::Latin),
      ("「", Script::Bopomofo),
      ("A", Script::Latin),
      ("」」", Script::Bopomofo),
    ]);
  }

  #[test]
  fn cjk_consecutive_parens_latin_2() {
    check_runs(&[
      ("「", Script::Bopomofo),
      ("A", Script::Latin),
      ("（", Script::Bopomofo),
      ("A", Script::Latin),
      ("）」", Script::Bopomofo),
    ]);
  }

  #[test]
  fn cjk_consecutive_parens_latin_3() {
    check_runs(&[
      ("「", Script::Bopomofo),
      ("A", Script::Latin),
      ("（国）」", Script::Han),
    ]);
  }

  #[test]
  fn emoji_common() {
    check_runs(&[("百家姓🌱🌲🌳🌴", Script::Han)]);
  }

  #[test]
  fn unmatched_close() {
    check_runs(&[
      ("Unicode (", Script::Latin),
      ("萬國碼] ", Script::Han),
      (") Unicode\"", Script::Latin),
    ]);
  }

  #[test]
  fn match_32_brackets() {
    check_runs(&[
      ("[萬國碼 ", Script::Han),
      (
        "Unicode (((((((((((((((((((((((((((((((!)))))))))))))))))))))))))))))))",
        Script::Latin,
      ),
      ("]", Script::Han),
    ]);
  }

  #[test]
  fn match_32_most_recent_brackets() {
    check_runs(&[
      ("((([萬國碼 ", Script::Han),
      ("Unicode (((((((((((((((((((((((((((((((", Script::Latin),
      ("萬國碼!", Script::Han),
      (")))))))))))))))))))))))))))))))", Script::Latin),
      ("]", Script::Han),
      ("But )))", Script::Latin),
    ]);
  }

  #[test]
  fn extensions_prefer_leading_context() {
    check_mock_runs(&[("h<lh>", Script::Han), ("l", Script::Latin)]);
  }

  #[test]
  fn extensions_match_trailing_context() {
    check_mock_runs(&[("h", Script::Han), ("<gl>l", Script::Latin)]);
  }

  #[test]
  fn extensions_retain_first_priority_script() {
    check_mock_runs(&[("<lhg><gh><hgl>", Script::Greek)]);
  }

  #[test]
  fn extensions_parens() {
    check_mock_runs(&[
      ("<gl><(lg>", Script::Greek),
      ("h<[hl>", Script::Han),
      ("l", Script::Latin),
      ("<]hl>", Script::Han),
      ("<)lg>", Script::Greek),
    ]);
  }

  #[test]
  fn extensions_parens_2() {
    check_mock_runs(&[("<(lhg><gh><)lhg>", Script::Greek)]);
  }

  #[test]
  fn common_with_priority() {
    check_mock_runs(&[("<ch>", Script::Han)]);
  }

  #[test]
  fn common_with_priority_2() {
    check_mock_runs(&[("<ch><lh>", Script::Latin)]);
  }

  #[test]
  fn common_with_priority_3() {
    check_mock_runs(&[("<ch><cl><cg>", Script::Han)]);
  }

  #[test]
  fn latin_dotted_circle_udatta() {
    check_runs(&[("Latin \u{25CC}\u{951}", Script::Latin)]);
  }

  #[test]
  fn han_dotted_circle_udatta() {
    check_runs(&[
      ("萬國碼 ", Script::Han),
      ("\u{25CC}\u{951}", Script::Bengali),
    ]);
  }

  #[test]
  fn latin_tatweel_fathatan() {
    check_runs(&[
      ("Latin ", Script::Latin),
      ("\u{640}\u{64B}", Script::Arabic),
    ]);
  }

  #[test]
  fn syriac_tatweel_fathatan() {
    check_runs(&[("\u{722}\u{640}\u{64B}", Script::Syriac)]);
  }

  #[test]
  fn han_udatta() {
    check_runs(&[("萬國碼\u{951}", Script::Han)]);
  }

  #[test]
  fn han_space_udatta() {
    check_runs(&[("萬國碼", Script::Han), (" \u{951}", Script::Bengali)]);
  }

  #[test]
  fn hangul() {
    check_runs(&[("키스의 고유조건은", Script::Hangul)]);
  }

  #[test]
  fn hiragana_mixed_punctuation() {
    check_runs(&[("いろはに.…¡ほへと", Script::Hiragana)]);
  }

  #[test]
  fn mock_han_inherited_gl() {
    check_mock_runs(&[("h<igl>", Script::Han)]);
  }

  #[test]
  fn mock_han_common_inherited_gl() {
    check_mock_runs(&[("h", Script::Han), ("c<igl>", Script::Greek)]);
  }

  #[test]
  fn mock_leading_inherited() {
    check_mock_runs(&[("<igl>", Script::Common)]);
  }

  #[test]
  fn mock_leading_inherited_2() {
    check_mock_runs(&[("<igl><ih>", Script::Common)]);
  }

  #[test]
  fn leading_inherited_han() {
    check_runs(&[("\u{951}萬國碼", Script::Han)]);
  }

  #[test]
  fn leading_inherited_han_2() {
    check_runs(&[("\u{951}\u{64B}萬國碼", Script::Han)]);
  }

  #[test]
  fn odd_latin_string() {
    check_runs(&[("ç̈", Script::Latin)]);
  }

  #[test]
  fn common_malayalam() {
    check_runs(&[("100-ാം", Script::Malayalam)]);
  }

  #[test]
  fn ideographic_comma_does_not_count_as_latin() {
    check_runs(&[
      ("也：", Script::Han),
      ("ABC", Script::Latin),
      ("、", Script::Bopomofo),
      ("DEF", Script::Latin),
    ]);
  }

  fn characters() -> impl Iterator<Item = char> {
    (0..=0x10FFFF).filter_map(char::from_u32)
  }

  #[test]
  fn max_unicode_script_extensions() {
    let max_extensions = characters()
      .map(|ch| {
        ScriptWithExtensions::new()
          .get_script_extensions_val(ch)
          .iter()
          .count()
      })
      .max()
      .unwrap();

    assert!(max_extensions <= MAX_UNICODE_SCRIPT_EXTENSIONS);
  }

  #[test]
  fn common_have_no_more_than_one_extension() {
    let mut extensions = Vec::new();

    for ch in characters() {
      IcuScriptData.get_scripts(ch, &mut extensions);

      if extensions[0] == Script::Common {
        assert!(extensions.len() <= 2, "{ch:?}");
      }
    }
  }
}
