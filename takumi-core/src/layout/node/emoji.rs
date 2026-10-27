//! Emoji drawn as images from a CDN, split out of text before the cascade as the JavaScript
//! `extractEmojis` helper splits them.
//!
//! Not what a browser does: Chromium draws emoji with a color font its font fallback picks. This
//! replaces each emoji cluster with an inline image sized to the em box instead, so it never needs
//! an emoji font and looks the same everywhere.

use std::{mem::take, sync::LazyLock};

use icu_properties::{
  CodePointSetData,
  props::{EmojiModifier, EmojiModifierBase, EmojiPresentation, ExtendedPictographic},
};
use icu_segmenter::GraphemeClusterSegmenter;
use serde::Deserialize;

use crate::{
  layout::node::{Node, NodeKind},
  style::{Style, StyleDeclarationBlock},
};

const ZERO_WIDTH_JOINER: char = '\u{200D}';
const TEXT_VARIATION_SELECTOR: char = '\u{FE0E}';
const EMOJI_VARIATION_SELECTOR: char = '\u{FE0F}';
const COMBINING_ENCLOSING_KEYCAP: char = '\u{20E3}';

/// The style of each emoji image, sized and aligned to sit in a line of text.
static EMOJI_STYLE: LazyLock<Style> = LazyLock::new(|| {
  Style::from(StyleDeclarationBlock::parse_loosy(
    "display: inline-block; width: 1em; height: 1em; margin: 0 0.05em 0 0.1em; vertical-align: -0.1em",
  ))
});

/// A CDN that serves an SVG for each emoji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EmojiProvider {
  /// Twemoji, from the `jdecked/twemoji` fork.
  Twemoji,
  /// OpenMoji.
  Openmoji,
  /// Blobmoji.
  Blobmoji,
  /// Noto Color Emoji.
  Noto,
  /// Fluent Emoji, color.
  Fluent,
  /// Fluent Emoji, flat.
  FluentFlat,
}

impl EmojiProvider {
  /// The URL of the SVG for `emoji`, a grapheme cluster.
  pub fn url(self, emoji: &str) -> String {
    let code = icon_code(emoji);

    match self {
      Self::Twemoji => format!(
        "https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.2/assets/svg/{}.svg",
        code.to_lowercase()
      ),
      Self::Openmoji => format!(
        "https://cdn.jsdelivr.net/npm/@svgmoji/openmoji@2.0.0/svg/{}.svg",
        code.to_uppercase()
      ),
      Self::Blobmoji => format!(
        "https://cdn.jsdelivr.net/npm/@svgmoji/blob@2.0.0/svg/{}.svg",
        code.to_uppercase()
      ),
      Self::Noto => format!(
        "https://cdn.jsdelivr.net/gh/googlefonts/noto-emoji@v2.051/svg/emoji_u{}.svg",
        code.to_lowercase().replace('-', "_")
      ),
      Self::Fluent => format!("{FLUENT}{}_color.svg", code.to_lowercase()),
      Self::FluentFlat => format!("{FLUENT}{}_flat.svg", code.to_lowercase()),
    }
  }
}

const FLUENT: &str = "https://cdn.jsdelivr.net/gh/shuding/fluentui-emoji-unicode@7a40f1a2d064d76e436813edc0f09b6c8cde5da8/assets/";

/// The hex code points of `emoji` joined by `-`, without its emoji variation selectors unless it
/// is a zero width joiner sequence.
fn icon_code(emoji: &str) -> String {
  let keeps_selectors = emoji.contains(ZERO_WIDTH_JOINER);

  emoji
    .chars()
    .filter(|&c| keeps_selectors || c != EMOJI_VARIATION_SELECTOR)
    .map(|c| format!("{:x}", u32::from(c)))
    .collect::<Vec<_>>()
    .join("-")
}

/// Whether a grapheme cluster presents as an emoji.
/// <https://github.com/google/emoji-segmenter/blob/master/emoji_presentation_scanner.rl>
fn is_emoji(segment: &str) -> bool {
  if segment.contains(TEXT_VARIATION_SELECTOR) {
    return false;
  }

  let mut chars = segment.chars();
  let Some(first) = chars.next() else {
    return false;
  };
  let second = chars.next();

  if is_regional_indicator_pair(segment) || is_keycap(segment) {
    return true;
  }

  CodePointSetData::new::<ExtendedPictographic>().contains(first)
    && (segment.contains(EMOJI_VARIATION_SELECTOR)
      || segment.contains(ZERO_WIDTH_JOINER)
      || CodePointSetData::new::<EmojiPresentation>().contains(first)
      || (CodePointSetData::new::<EmojiModifierBase>().contains(first)
        && second.is_some_and(|c| CodePointSetData::new::<EmojiModifier>().contains(c))))
}

/// Whether `c` is a code point every emoji cluster holds one of.
fn may_start_emoji(c: char) -> bool {
  c == COMBINING_ENCLOSING_KEYCAP
    || ('\u{1F1E6}'..='\u{1F1FF}').contains(&c)
    || CodePointSetData::new::<ExtendedPictographic>().contains(c)
}

fn is_regional_indicator_pair(segment: &str) -> bool {
  let count = segment.chars().count();

  (1..=2).contains(&count)
    && segment
      .chars()
      .all(|c| ('\u{1F1E6}'..='\u{1F1FF}').contains(&c))
}

fn is_keycap(segment: &str) -> bool {
  let chars: Vec<char> = segment.chars().collect();

  match chars.as_slice() {
    [base, COMBINING_ENCLOSING_KEYCAP]
    | [base, EMOJI_VARIATION_SELECTOR, COMBINING_ENCLOSING_KEYCAP] => {
      base.is_ascii_digit() || *base == '#' || *base == '*'
    }
    _ => false,
  }
}

/// `text` split into text and emoji image nodes, or `None` when it holds no emoji.
fn split(text: &str, provider: EmojiProvider) -> Option<Vec<Node>> {
  if !text.chars().any(may_start_emoji) {
    return None;
  }

  let segmenter = GraphemeClusterSegmenter::new();
  let breaks: Vec<usize> = segmenter.segment_str(text).collect();
  let segments = breaks.windows(2).map(|bounds| &text[bounds[0]..bounds[1]]);

  if !segments.clone().any(is_emoji) {
    return None;
  }

  let mut nodes = Vec::new();
  let mut run = String::new();

  for segment in segments {
    if is_emoji(segment) {
      if !run.is_empty() {
        nodes.push(Node::text(take(&mut run)));
      }

      nodes.push(Node::image(provider.url(segment).as_str()).with_style(EMOJI_STYLE.clone()));
    } else {
      run.push_str(segment);
    }
  }

  if !run.is_empty() {
    nodes.push(Node::text(run));
  }

  Some(nodes)
}

impl Node {
  /// Replaces each emoji in this tree's text with an image `provider` serves, turning a text node
  /// that holds one into a container of text and image nodes that keeps its attributes.
  pub fn extract_emojis(&mut self, provider: EmojiProvider) {
    let mut stack = vec![self];

    while let Some(node) = stack.pop() {
      let parts = match &node.kind {
        NodeKind::Text(text) => split(&text.text, provider),
        _ => None,
      };

      if let Some(children) = parts {
        node.kind = NodeKind::Container { children };
      } else if let NodeKind::Container { children } = &mut node.kind {
        stack.extend(children.iter_mut());
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::layout::node::ImageSourceInput;

  /// `text` after extraction, as its text runs and the twemoji codes of its images.
  fn parts(text: &str) -> Vec<String> {
    let mut node = Node::text(text);

    node.extract_emojis(EmojiProvider::Twemoji);

    let NodeKind::Container { children } = &node.kind else {
      return vec![format!("text {text}")];
    };

    children
      .iter()
      .map(|child| match &child.kind {
        NodeKind::Text(text) => format!("text {}", text.text),
        NodeKind::Image(image) => match &image.src {
          ImageSourceInput::Url(url) => format!(
            "emoji {}",
            url
              .trim_start_matches("https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.2/assets/svg/")
              .trim_end_matches(".svg")
          ),
          _ => "image".to_owned(),
        },
        NodeKind::Container { .. } => "container".to_owned(),
      })
      .collect()
  }

  #[test]
  fn splits_as_the_javascript_helper_does() {
    let cases: &[(&str, &[&str])] = &[
      ("Hello, world!", &["text Hello, world!"]),
      (
        "#1 *2 \u{200D}\u{FE0F}\u{1F3FB} é",
        &["text #1 *2 \u{200D}\u{FE0F}\u{1F3FB} é"],
      ),
      ("Hello 😀", &["text Hello ", "emoji 1f600"]),
      (
        "😀 transformation 🚀",
        &["emoji 1f600", "text  transformation ", "emoji 1f680"],
      ),
      ("👨‍👩‍👧‍👦", &["emoji 1f468-200d-1f469-200d-1f467-200d-1f466"]),
      ("US flag 🇺🇸", &["text US flag ", "emoji 1f1fa-1f1f8"]),
      (
        "Press 1️⃣ now",
        &["text Press ", "emoji 31-20e3", "text  now"],
      ),
      ("‼", &["text ‼"]),
      ("\u{1F17F}", &["text \u{1F17F}"]),
      ("▶", &["text ▶"]),
      ("😀︎", &["text 😀︎"]),
      ("1︎⃣", &["text 1︎⃣"]),
      ("‼️", &["emoji 203c"]),
      ("\u{1F17F}️", &["emoji 1f17f"]),
      ("▶️", &["emoji 25b6"]),
      ("☝\u{1F3FD}", &["emoji 261d-1f3fd"]),
      ("❤‍🔥", &["emoji 2764-200d-1f525"]),
      ("🇺", &["emoji 1f1fa"]),
    ];

    for (text, expected) in cases {
      assert_eq!(parts(text), *expected, "{text:?}");
    }
  }

  #[test]
  fn names_each_provider_file_as_the_javascript_helper_does() {
    assert_eq!(
      EmojiProvider::Noto.url("😀"),
      "https://cdn.jsdelivr.net/gh/googlefonts/noto-emoji@v2.051/svg/emoji_u1f600.svg"
    );
    assert_eq!(
      EmojiProvider::Openmoji.url("😀"),
      "https://cdn.jsdelivr.net/npm/@svgmoji/openmoji@2.0.0/svg/1F600.svg"
    );
  }

  #[test]
  fn keeps_a_text_node_attributes_on_its_container() {
    let mut node = Node::text("😀").with_class_name("emoji");

    node.extract_emojis(EmojiProvider::Twemoji);

    assert!(matches!(node.kind, NodeKind::Container { .. }));
    assert_eq!(node.metadata.class_name.as_deref(), Some("emoji"));
  }
}
