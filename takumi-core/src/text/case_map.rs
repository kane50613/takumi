//! Blink's [`CaseMap`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/wtf/text/case_map.cc), reading ICU4X.

use std::borrow::Cow;

use icu_casemap::CaseMapper;
use icu_locale_core::{LanguageIdentifier, langid};

/// Blink's `CaseMap`: case conversion under a language's rules.
pub(crate) struct CaseMap {
  case_map_locale: LanguageIdentifier,
}

impl CaseMap {
  /// Blink's `CaseMap::Locale`: only Turkic, Greek, Lithuanian, and Dutch convert case their own
  /// way.
  pub(crate) fn new(locale: Option<&str>) -> Self {
    let matches_lang = |lang| locale.is_some_and(|locale| locale_id_matches_lang(locale, lang));
    let case_map_locale = if matches_lang("tr") || matches_lang("az") {
      langid!("tr")
    } else if matches_lang("el") {
      langid!("el")
    } else if matches_lang("lt") {
      langid!("lt")
    } else if matches_lang("nl") {
      langid!("nl")
    } else {
      LanguageIdentifier::UNKNOWN
    };

    Self { case_map_locale }
  }

  /// Blink's `ToLower`.
  pub(crate) fn to_lower<'s>(&self, source: &'s str) -> Cow<'s, str> {
    CaseMapper::new().lowercase_to_string(source, &self.case_map_locale)
  }

  /// Blink's `ToUpper`.
  pub(crate) fn to_upper<'s>(&self, source: &'s str) -> Cow<'s, str> {
    CaseMapper::new().uppercase_to_string(source, &self.case_map_locale)
  }
}

/// Blink's `LocaleIdMatchesLang`: whether `locale_id` is `lang`, alone or before a delimiter.
fn locale_id_matches_lang(locale_id: &str, lang: &str) -> bool {
  locale_id
    .get(..lang.len())
    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(lang))
    && matches!(
      locale_id.as_bytes().get(lang.len()),
      None | Some(b'-' | b'_' | b'@')
    )
}

#[cfg(test)]
mod tests {
  use super::CaseMap;

  const TURKIC_INPUT: &str = "isI\u{130} \u{131}s\u{130}I";
  const GREEK_INPUT: &str = "\u{39F}\u{394}\u{38C}\u{3A3} \u{39F}\u{3B4}\u{3CC}\u{3C2} \u{3A3}\u{3BF} \u{3A3}\u{39F} o\u{3A3} \u{39F}\u{3A3} \u{3C3} \u{1F15}\u{3BE}";
  const LITHUANIAN_INPUT: &str = "I \u{CF} J J\u{308} \u{12E} \u{12E}\u{308} \u{CC} \u{CD} \u{128} xi\u{307}\u{308} xj\u{307}\u{308} x\u{12F}\u{307}\u{308} xi\u{307}\u{300} xi\u{307}\u{301} xi\u{307}\u{303} XI X\u{CF} XJ XJ\u{308} X\u{12E} X\u{12E}\u{308}";
  const TURKIC_LOCALES: &[&str] = &[
    "tr",
    "tr-TR",
    "tr_TR",
    "tr@foo=bar",
    "tr-US",
    "TR",
    "tr-tr",
    "tR",
    "az",
    "az-AZ",
    "az_AZ",
    "az@foo=bar",
    "az-US",
    "Az",
    "AZ-AZ",
  ];
  const NON_TURKIC_LOCALES: &[&str] = &[
    "en",
    "en-US",
    "en_US",
    "en@foo=bar",
    "EN",
    "En",
    "ja",
    "el",
    "fil",
    "fi",
    "lt",
  ];
  const GREEK_LOCALES: &[&str] = &[
    "el",
    "el-GR",
    "el_GR",
    "el@foo=bar",
    "el-US",
    "EL",
    "el-gr",
    "eL",
  ];
  const NON_GREEK_LOCALES: &[&str] = &[
    "en",
    "en-US",
    "en_US",
    "en@foo=bar",
    "EN",
    "En",
    "ja",
    "tr",
    "az",
    "fil",
    "fi",
    "lt",
  ];
  const LITHUANIAN_LOCALES: &[&str] = &[
    "lt",
    "lt-LT",
    "lt_LT",
    "lt@foo=bar",
    "lt-US",
    "LT",
    "lt-lt",
    "lT",
  ];
  const NON_LITHUANIAN_LOCALES: &[&str] = &[
    "en",
    "en-US",
    "en_US",
    "en@foo=bar",
    "EN",
    "En",
    "ja",
    "fil",
    "fi",
    "el",
  ];

  /// Blink's `CaseMapTest` data, without its title cases.
  #[test]
  fn to_lower_and_to_upper() {
    let cases = [
      ("", "", "", ""),
      ("123", "", "123", "123"),
      ("xyz", "", "xyz", "XYZ"),
      ("XYZ", "", "xyz", "XYZ"),
      ("Xyz", "", "xyz", "XYZ"),
      ("xYz", "", "xyz", "XYZ"),
      ("abc\u{A0}def", "", "abc\u{A0}def", "ABC\u{A0}DEF"),
      ("\u{DF}", "", "\u{DF}", "SS"),
      ("\u{DF}z", "", "\u{DF}z", "SSZ"),
      ("x\u{DF}", "", "x\u{DF}", "XSS"),
      ("x\u{DF}z", "", "x\u{DF}z", "XSSZ"),
      ("\u{130}", "tr", "i", "\u{130}"),
      ("I\u{307}", "tr", "i", "I\u{307}"),
      ("i\u{307}", "lt", "i\u{307}", "I"),
      ("i\u{307}z", "lt", "i\u{307}z", "IZ"),
      ("xi\u{307}", "lt", "xi\u{307}", "XI"),
      ("xi\u{307}z", "lt", "xi\u{307}z", "XIZ"),
      ("\u{CC}", "lt", "i\u{307}\u{300}", "\u{CC}"),
      ("ij", "nl", "ij", "IJ"),
      ("ijsland", "nl", "ijsland", "IJSLAND"),
      ("aij", "nl", "aij", "AIJ"),
      ("\u{DF}i\u{307}", "lt", "\u{DF}i\u{307}", "SSI"),
      ("\u{DF}yi\u{307}z", "lt", "\u{DF}yi\u{307}z", "SSYIZ"),
      ("i\u{307}\u{DF}", "lt", "i\u{307}\u{DF}", "ISS"),
    ];

    for (source, locale, lower, upper) in cases {
      let case_map = CaseMap::new(Some(locale));

      assert_eq!(case_map.to_lower(source), lower, "{source}");
      assert_eq!(case_map.to_upper(source), upper, "{source}");
    }
  }

  #[test]
  fn to_upper_locale() {
    let cases = [
      (TURKIC_INPUT, TURKIC_LOCALES, "\u{130}SI\u{130} IS\u{130}I"),
      (TURKIC_INPUT, NON_TURKIC_LOCALES, "ISI\u{130} IS\u{130}I"),
      (
        GREEK_INPUT,
        GREEK_LOCALES,
        "\u{39F}\u{394}\u{39F}\u{3A3} \u{39F}\u{394}\u{39F}\u{3A3} \u{3A3}\u{39F} \u{3A3}\u{39F} O\u{3A3} \u{39F}\u{3A3} \u{3A3} \u{395}\u{39E}",
      ),
      (
        GREEK_INPUT,
        NON_GREEK_LOCALES,
        "\u{39F}\u{394}\u{38C}\u{3A3} \u{39F}\u{394}\u{38C}\u{3A3} \u{3A3}\u{39F} \u{3A3}\u{39F} O\u{3A3} \u{39F}\u{3A3} \u{3A3} \u{1F1D}\u{39E}",
      ),
      (
        LITHUANIAN_INPUT,
        LITHUANIAN_LOCALES,
        "I \u{CF} J J\u{308} \u{12E} \u{12E}\u{308} \u{CC} \u{CD} \u{128} XI\u{308} XJ\u{308} X\u{12E}\u{308} XI\u{300} XI\u{301} XI\u{303} XI X\u{CF} XJ XJ\u{308} X\u{12E} X\u{12E}\u{308}",
      ),
      (
        LITHUANIAN_INPUT,
        NON_LITHUANIAN_LOCALES,
        "I \u{CF} J J\u{308} \u{12E} \u{12E}\u{308} \u{CC} \u{CD} \u{128} XI\u{307}\u{308} XJ\u{307}\u{308} X\u{12E}\u{307}\u{308} XI\u{307}\u{300} XI\u{307}\u{301} XI\u{307}\u{303} XI X\u{CF} XJ XJ\u{308} X\u{12E} X\u{12E}\u{308}",
      ),
    ];

    for (source, locales, expected) in cases {
      for &locale in locales {
        assert_eq!(
          CaseMap::new(Some(locale)).to_upper(source),
          expected,
          "{locale}"
        );
      }
    }
  }

  #[test]
  fn to_lower_locale() {
    let cases = [
      (TURKIC_INPUT, TURKIC_LOCALES, "is\u{131}i \u{131}si\u{131}"),
      (
        TURKIC_INPUT,
        NON_TURKIC_LOCALES,
        "isii\u{307} \u{131}si\u{307}i",
      ),
      (
        GREEK_INPUT,
        GREEK_LOCALES,
        "\u{3BF}\u{3B4}\u{3CC}\u{3C2} \u{3BF}\u{3B4}\u{3CC}\u{3C2} \u{3C3}\u{3BF} \u{3C3}\u{3BF} o\u{3C2} \u{3BF}\u{3C2} \u{3C3} \u{1F15}\u{3BE}",
      ),
      (
        GREEK_INPUT,
        NON_GREEK_LOCALES,
        "\u{3BF}\u{3B4}\u{3CC}\u{3C2} \u{3BF}\u{3B4}\u{3CC}\u{3C2} \u{3C3}\u{3BF} \u{3C3}\u{3BF} o\u{3C2} \u{3BF}\u{3C2} \u{3C3} \u{1F15}\u{3BE}",
      ),
      (
        LITHUANIAN_INPUT,
        LITHUANIAN_LOCALES,
        "i \u{EF} j j\u{307}\u{308} \u{12F} \u{12F}\u{307}\u{308} i\u{307}\u{300} i\u{307}\u{301} i\u{307}\u{303} xi\u{307}\u{308} xj\u{307}\u{308} x\u{12F}\u{307}\u{308} xi\u{307}\u{300} xi\u{307}\u{301} xi\u{307}\u{303} xi x\u{EF} xj xj\u{307}\u{308} x\u{12F} x\u{12F}\u{307}\u{308}",
      ),
      (
        LITHUANIAN_INPUT,
        NON_LITHUANIAN_LOCALES,
        "i \u{EF} j j\u{308} \u{12F} \u{12F}\u{308} \u{EC} \u{ED} \u{129} xi\u{307}\u{308} xj\u{307}\u{308} x\u{12F}\u{307}\u{308} xi\u{307}\u{300} xi\u{307}\u{301} xi\u{307}\u{303} xi x\u{EF} xj xj\u{308} x\u{12F} x\u{12F}\u{308}",
      ),
    ];

    for (source, locales, expected) in cases {
      for &locale in locales {
        assert_eq!(
          CaseMap::new(Some(locale)).to_lower(source),
          expected,
          "{locale}"
        );
      }
    }
  }
}
