//! CSS tables, laid out by a port of Blink's
//! [table layout](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/table/)
//! under the notice in LICENSE-CHROMIUM.

mod borders;
#[cfg(feature = "fragmentation")]
mod break_token_data;
mod columns;
mod constraint_space_data;
mod fragment;
mod fragmentation;
mod layout;
mod parts;
mod row_layout_algorithm;
mod rows;
mod section_layout_algorithm;

#[cfg(feature = "fragmentation")]
pub(crate) use self::break_token_data::TableBreakTokenData;
pub(crate) use self::{
  borders::{TableBorders, style_rank},
  columns::ColumnConstraints,
  constraint_space_data::TablePartSpace,
  layout::{TableLayoutAlgorithm, TableStructure},
};
pub use self::{fragment::TableFragment, parts::TableSectionKind};

#[cfg(test)]
mod tests;
