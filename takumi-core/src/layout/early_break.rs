//! Blink's [`EarlyBreak`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/early_break.h).

use std::rc::Rc;

use crate::{geometry::NodeId, layout::break_appeal::BreakAppeal};

/// Blink's `EarlyBreak`: a good breakpoint seen earlier, to go back and break at when the space
/// runs out somewhere worse.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EarlyBreak {
  pub(crate) target: EarlyBreakTarget,
  pub(crate) break_appeal: BreakAppeal,
}

/// Blink's `EarlyBreak::BreakType` with the line number or block it names.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum EarlyBreakTarget {
  /// Before the line of this number.
  Line(usize),
  /// Before the block, or inside it at the breakpoint given.
  Block {
    node: NodeId,
    break_inside: Option<Rc<EarlyBreak>>,
  },
}

impl EarlyBreak {
  /// Blink's `EarlyBreak(BlockNode, BreakAppeal, const EarlyBreak*)`.
  pub(crate) fn block(
    node: NodeId,
    break_appeal: BreakAppeal,
    break_inside: Option<Rc<EarlyBreak>>,
  ) -> Self {
    Self {
      target: EarlyBreakTarget::Block { node, break_inside },
      break_appeal,
    }
  }

  /// Blink's `EarlyBreak(int, BreakAppeal)`.
  pub(crate) fn line(line_number: usize, break_appeal: BreakAppeal) -> Self {
    Self {
      target: EarlyBreakTarget::Line(line_number),
      break_appeal,
    }
  }

  /// Blink's `IsBreakBefore`.
  pub(crate) fn is_break_before(&self) -> bool {
    !matches!(
      self.target,
      EarlyBreakTarget::Block {
        break_inside: Some(_),
        ..
      }
    )
  }
}
