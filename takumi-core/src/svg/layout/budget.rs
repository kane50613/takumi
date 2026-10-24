//! What one document may spend. A `use` can instantiate a group that instantiates a group, and a
//! mask or an image can be referenced by every one of them, so what a document lays out and
//! paints can outgrow its markup exponentially.

use crate::layout::node::MAXIMUM_DOM_TREE_DEPTH;

/// How many element instances a document may lay out in total, nested SVG images included.
const MAXIMUM_INSTANCES: usize = 100_000;
/// How many ops and path commands a document may paint, which counts a resource again each time
/// something references it.
const MAXIMUM_PAINTED: usize = 4_000_000;
/// How many bytes of images a document may decode and parse.
const MAXIMUM_IMAGE_BYTES: usize = 32 << 20;
/// How many bytes of text a document may shape, which `tref` elements multiply.
const MAXIMUM_TEXT_BYTES: usize = 1 << 18;

/// The limits of one document, shared with the SVG images inside it.
#[derive(Debug)]
pub(super) struct Budget {
  instances: usize,
  painted: usize,
  image_bytes: usize,
  text_bytes: usize,
  depth: usize,
}

impl Default for Budget {
  fn default() -> Self {
    Self {
      instances: MAXIMUM_INSTANCES,
      painted: MAXIMUM_PAINTED,
      image_bytes: MAXIMUM_IMAGE_BYTES,
      text_bytes: MAXIMUM_TEXT_BYTES,
      depth: 0,
    }
  }
}

impl Budget {
  /// Takes one element instance, false once they are spent.
  pub(super) fn spend_instance(&mut self) -> bool {
    Self::spend(&mut self.instances, 1)
  }

  /// Takes `amount` of painted ops and path commands, false once they are spent.
  pub(super) fn spend_painted(&mut self, amount: usize) -> bool {
    Self::spend(&mut self.painted, amount)
  }

  /// Takes `bytes` of image payload, false once they are spent.
  pub(super) fn spend_image_bytes(&mut self, bytes: usize) -> bool {
    Self::spend(&mut self.image_bytes, bytes)
  }

  /// Takes `bytes` of text to shape, false once they are spent, which leaves nothing to paint.
  pub(super) fn spend_text_bytes(&mut self, bytes: usize) -> bool {
    let spent = Self::spend(&mut self.text_bytes, bytes);

    if !spent {
      self.painted = 0;
    }

    spent
  }

  /// Goes one level deeper, false at the depth a document may nest to. Each success needs a
  /// [`Self::leave`].
  pub(super) fn enter(&mut self) -> bool {
    if self.depth >= MAXIMUM_DOM_TREE_DEPTH {
      return false;
    }

    self.depth += 1;
    true
  }

  pub(super) fn leave(&mut self) {
    self.depth = self.depth.saturating_sub(1);
  }

  fn spend(remaining: &mut usize, amount: usize) -> bool {
    match remaining.checked_sub(amount) {
      Some(left) => {
        *remaining = left;
        true
      }
      None => {
        *remaining = 0;
        false
      }
    }
  }
}
