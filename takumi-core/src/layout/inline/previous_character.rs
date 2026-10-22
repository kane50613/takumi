//! Blink's [`LayoutText::PreviousCharacter`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/layout_text.cc),
//! the character before a text, which tells `capitalize` whether the text starts a word.

use std::borrow::Cow;

use crate::{
  layout::tree::RenderNode,
  style::{ComputedStyle, Display, Float, TextTransformCase},
};

/// The character before the next text of an inline formatting context's walk.
pub(crate) struct PreviousCharacter<'c> {
  character: char,
  /// Boxes passed since `character`, whose content is laid out apart and read only when a text
  /// needs it.
  boxes: Vec<&'c RenderNode>,
}

impl<'c> PreviousCharacter<'c> {
  /// The character before a formatting context's first text.
  pub(crate) fn new() -> Self {
    Self {
      character: ' ',
      boxes: Vec::new(),
    }
  }

  /// Blink's `ApplyTextTransform` of a text with `style`, and the walk past it.
  pub(crate) fn transform_text<'t>(
    &mut self,
    style: &ComputedStyle,
    text: &'t str,
  ) -> Cow<'t, str> {
    if style.inherited_data.text_transform.case == TextTransformCase::Capitalize
      && !self.boxes.is_empty()
    {
      self.character = character_after(&self.boxes, self.character);
      self.boxes.clear();
    }

    let text = style.apply_text_transform(text, self.character);

    // Blink skips an empty text.
    if let Some(character) = text.chars().next_back() {
      self.character = character;
      self.boxes.clear();
    }

    text
  }

  /// The walk past an atomic inline, a float, or an out-of-flow box.
  pub(crate) fn pass_box(&mut self, node: &'c RenderNode) {
    self.boxes.push(node);
  }

  /// The walk into `node`'s content, which ends a word unless the box passes the character before.
  pub(crate) fn enter_box(&mut self, node: &RenderNode) {
    if !node.passes_previous_character() {
      self.character = ' ';
      self.boxes.clear();
    }
  }
}

impl RenderNode {
  /// Blink skips out-of-flow and inline boxes looking for the character before a text.
  fn passes_previous_character(&self) -> bool {
    let style = &self.context.style.base_data;

    self.is_out_of_flow()
      || (style.display == Display::Inline
        && style.float == Float::None
        && !self.is_atomic_inline())
  }
}

/// A step of the walk backward through boxes in pre-order.
enum Step<'c> {
  Enter(&'c RenderNode),
  Text(&'c RenderNode),
  Exit(&'c RenderNode),
}

/// Blink's walk back from a text through the content of `boxes`: the last character of the last
/// text, a space at a box that ends a word, or `character_before` when neither comes first.
fn character_after(boxes: &[&RenderNode], character_before: char) -> char {
  let mut steps: Vec<Step> = boxes.iter().map(|&node| Step::Enter(node)).collect();

  while let Some(step) = steps.pop() {
    match step {
      Step::Enter(node) => {
        steps.push(Step::Exit(node));
        steps.extend(node.marker().map(Step::Enter));

        if node.own_text().is_some() {
          steps.push(Step::Text(node));
        }

        steps.extend(node.children_after_marker().iter().map(Step::Enter));
      }
      Step::Text(node) => {
        let Some(text) = node.own_text() else {
          continue;
        };

        // Titlecasing keeps a letter a letter, and the next text reads only the word-break class
        // of this one's last character, so a space stands in for the character before it.
        if let Some(character) = node
          .context
          .style
          .apply_text_transform(text, ' ')
          .chars()
          .next_back()
        {
          return character;
        }
      }
      Step::Exit(node) => {
        if !node.passes_previous_character() {
          return ' ';
        }
      }
    }
  }

  character_before
}
