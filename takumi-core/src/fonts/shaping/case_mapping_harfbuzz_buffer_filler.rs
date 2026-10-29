//! Blink's [`CaseMappingHarfBuzzBufferFiller`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/shaping/case_mapping_harfbuzz_buffer_filler.cc).

use harfrust::Buffer;

use crate::text::case_map::CaseMap;

/// Blink's `CaseMapIntend`: the case synthesized small capitals draw a run in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(clippy::enum_variant_names, reason = "Blink's names")]
pub(crate) enum CaseMapIntend {
  KeepSameCase,
  UpperCase,
  LowerCase,
}

impl CaseMapIntend {
  /// Blink's `CaseMappingHarfBuzzBufferFiller`.
  pub(crate) fn fill_buffer(
    self,
    buffer: &mut Buffer,
    locale: Option<&str>,
    text: &str,
    start: usize,
    end: usize,
  ) {
    let case_map = CaseMap::new(locale);
    let case_mapped_text = match self {
      Self::KeepSameCase => {
        for (offset, character) in text[start..end].char_indices() {
          buffer.push(u32::from(character), (start + offset) as u32);
        }

        buffer.set_pre_context(&text[..start]);
        buffer.set_post_context(&text[end..]);

        return;
      }
      Self::UpperCase => case_map.to_upper(text),
      Self::LowerCase => case_map.to_lower(text),
    };

    // Blink maps the whole text, which keeps contextual mappings, when that keeps its UTF-16
    // length, and each character alone otherwise.
    if case_mapped_text.encode_utf16().count() != text.encode_utf16().count() {
      self.fill_slow_case(buffer, &case_map, text, start, end);

      return;
    }

    // Blink adds the mapped text at its source's UTF-16 offsets; a mapped character takes the
    // source character at its offset.
    let mut sources = text
      .char_indices()
      .scan(0, |utf16_offset, (byte, character)| {
        let source = (*utf16_offset, byte);

        *utf16_offset += character.len_utf16();

        Some(source)
      })
      .peekable();
    let mut source_byte = 0;
    let mut utf16_offset = 0;
    let mut pre_context_end = None;
    let mut post_context_start = case_mapped_text.len();

    for (byte, character) in case_mapped_text.char_indices() {
      while let Some(&(source_utf16_offset, byte)) = sources.peek() {
        if source_utf16_offset > utf16_offset {
          break;
        }

        source_byte = byte;
        sources.next();
      }

      if source_byte >= end {
        post_context_start = byte;

        break;
      }

      if source_byte >= start {
        pre_context_end.get_or_insert(byte);
        buffer.push(u32::from(character), source_byte as u32);
      }

      utf16_offset += character.len_utf16();
    }

    let pre_context_end = pre_context_end.unwrap_or(post_context_start);

    buffer.set_pre_context(&case_mapped_text[..pre_context_end]);
    buffer.set_post_context(&case_mapped_text[post_context_start..]);
  }

  /// Blink's `FillSlowCase`: each character's mapping at its cluster.
  fn fill_slow_case(
    self,
    buffer: &mut Buffer,
    case_map: &CaseMap,
    text: &str,
    start: usize,
    end: usize,
  ) {
    let mut character_text = [0; 4];

    for (offset, character) in text[start..end].char_indices() {
      let character_text = character.encode_utf8(&mut character_text);
      let case_mapped_character = if self == Self::UpperCase {
        case_map.to_upper(character_text)
      } else {
        case_map.to_lower(character_text)
      };

      for case_mapped in case_mapped_character.chars() {
        buffer.push(u32::from(case_mapped), (start + offset) as u32);
      }
    }

    buffer.set_pre_context(&text[..start]);
    buffer.set_post_context(&text[end..]);
  }
}
