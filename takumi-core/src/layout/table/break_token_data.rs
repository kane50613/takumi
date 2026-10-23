//! Blink's [`TableBreakTokenData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/table/table_break_token_data.h),
//! under the notice in LICENSE-CHROMIUM.

use std::rc::Rc;

use crate::{layout::table::rows::Rows, layout_unit::LayoutUnit};

/// What a table resumes from: the rows it computed once, and how far its table box got.
#[derive(PartialEq)]
pub(crate) struct TableBreakTokenData {
  pub(super) rows: Rc<Rows>,
  /// The sections' block size before the table's own block size grew them.
  pub(super) total_table_min_block_size: LayoutUnit,
  /// The block size the table box took in earlier fragments, captions left out.
  pub(super) consumed_table_box_block_size: LayoutUnit,
  /// Whether an earlier fragment got past the top captions into the table box.
  pub(super) has_entered_table_box: bool,
  /// Whether an earlier fragment got past every section.
  pub(super) is_past_table_box: bool,
}
