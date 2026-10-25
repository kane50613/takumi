use smallvec::{SmallVec, smallvec};

use crate::{
  style::{Direction, impl_css_enum},
  text::character::{
    FIRST_STRONG_ISOLATE, LEFT_TO_RIGHT_EMBEDDING, LEFT_TO_RIGHT_ISOLATE, LEFT_TO_RIGHT_OVERRIDE,
    POP_DIRECTIONAL_FORMATTING, POP_DIRECTIONAL_ISOLATE, RIGHT_TO_LEFT_EMBEDDING,
    RIGHT_TO_LEFT_ISOLATE, RIGHT_TO_LEFT_OVERRIDE,
  },
};

/// `unicode-bidi`: how a box's text joins the bidi algorithm, with its `direction`.
/// <https://drafts.csswg.org/css-writing-modes-3/#unicode-bidi>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum UnicodeBidi {
  /// No embedding level of its own.
  #[default]
  Normal,
  /// An embedding level in its `direction`.
  Embed,
  /// An isolated run in its `direction`.
  Isolate,
  /// Its characters all run in its `direction`.
  BidiOverride,
  /// An isolated run whose characters all run in its `direction`.
  IsolateOverride,
  /// Paragraphs whose direction their first strong character decides.
  Plaintext,
}

impl_css_enum!(
  UnicodeBidi,
  "normal" => UnicodeBidi::Normal,
  "embed" => UnicodeBidi::Embed,
  "isolate" => UnicodeBidi::Isolate,
  "bidi-override" => UnicodeBidi::BidiOverride,
  "isolate-override" => UnicodeBidi::IsolateOverride,
  "plaintext" => UnicodeBidi::Plaintext,
);

impl UnicodeBidi {
  /// The pairs of opening and closing controls an inline box of this `unicode-bidi` and
  /// `direction` puts around its content, outermost first.
  /// <https://drafts.csswg.org/css-writing-modes-3/#bidi-control-codes-injection-table>
  pub(crate) fn inline_controls(self, direction: Direction) -> SmallVec<[(char, char); 2]> {
    let directional = |[ltr, rtl, exit]: [char; 3]| match direction {
      Direction::Ltr => (ltr, exit),
      Direction::Rtl => (rtl, exit),
    };

    match self {
      Self::Normal => SmallVec::new(),
      Self::Embed => smallvec![directional(EMBED)],
      Self::BidiOverride => smallvec![directional(OVERRIDE)],
      Self::Isolate => smallvec![directional(ISOLATE)],
      Self::Plaintext => smallvec![(FIRST_STRONG_ISOLATE, POP_DIRECTIONAL_ISOLATE)],
      Self::IsolateOverride => smallvec![
        (FIRST_STRONG_ISOLATE, POP_DIRECTIONAL_ISOLATE),
        directional(OVERRIDE),
      ],
    }
  }

  /// The opening and closing controls a block of this `unicode-bidi` and `direction` puts around
  /// its content, an override alone as the block's direction sets the paragraph's.
  pub(crate) fn block_controls(self, direction: Direction) -> Option<(char, char)> {
    matches!(self, Self::BidiOverride | Self::IsolateOverride).then(|| match direction {
      Direction::Ltr => (LEFT_TO_RIGHT_OVERRIDE, POP_DIRECTIONAL_FORMATTING),
      Direction::Rtl => (RIGHT_TO_LEFT_OVERRIDE, POP_DIRECTIONAL_FORMATTING),
    })
  }
}

/// The left-to-right opening, right-to-left opening, and closing controls of an embedding.
const EMBED: [char; 3] = [
  LEFT_TO_RIGHT_EMBEDDING,
  RIGHT_TO_LEFT_EMBEDDING,
  POP_DIRECTIONAL_FORMATTING,
];
/// The left-to-right opening, right-to-left opening, and closing controls of an override.
const OVERRIDE: [char; 3] = [
  LEFT_TO_RIGHT_OVERRIDE,
  RIGHT_TO_LEFT_OVERRIDE,
  POP_DIRECTIONAL_FORMATTING,
];
/// The left-to-right opening, right-to-left opening, and closing controls of an isolate.
const ISOLATE: [char; 3] = [
  LEFT_TO_RIGHT_ISOLATE,
  RIGHT_TO_LEFT_ISOLATE,
  POP_DIRECTIONAL_ISOLATE,
];
