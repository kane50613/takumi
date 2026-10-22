//! CSS tables, laid out by a port of Blink's
//! [table layout](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/table/)
//! under the notice in LICENSE-CHROMIUM.

mod borders;
mod columns;
mod fragment;
mod layout;
mod parts;
mod rows;

pub(crate) use self::{
  borders::{TableBorders, style_rank},
  columns::ColumnConstraints,
  layout::{TableLayoutAlgorithm, TableStructure},
};
pub use self::{fragment::TableFragment, parts::TableSectionKind};

#[cfg(test)]
mod tests;
