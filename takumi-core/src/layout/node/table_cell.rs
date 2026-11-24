//! The style the `nowrap`, `width`, and `height` attributes of a `<td>` or `<th>` map to: Blink's
//! `HTMLTableCellElement::CollectStyleForPresentationAttribute`
//! (`third_party/blink/renderer/core/html/html_table_cell_element.cc`).

use crate::{
  layout::{
    node::{Node, html_dimension::parse_dimension_value},
    table::TABLE_CELL_TAGS,
  },
  style::{Length, Style, StyleDeclaration, TextWrapMode, WhiteSpaceCollapse},
};

impl Node {
  /// Pushes a table cell's `nowrap`, `width`, and `height` attributes into its presentation
  /// attributes' style.
  pub(super) fn push_table_cell_presentation(&self, presentation: &mut Option<Box<Style>>) {
    let is_table_cell = self.tag_name().is_some_and(|tag| {
      TABLE_CELL_TAGS
        .iter()
        .any(|cell| tag.eq_ignore_ascii_case(cell))
    });

    if !is_table_cell {
      return;
    }

    let mut declarations = Vec::new();

    if self.attribute("nowrap").is_some() {
      declarations.push(StyleDeclaration::white_space_collapse(
        WhiteSpaceCollapse::Collapse,
      ));
      declarations.push(StyleDeclaration::text_wrap_mode(TextWrapMode::NoWrap));
    }

    // Blink's `AddHTMLLengthToStyle` with `kAllowPercentageValues` and `kDontAllowZeroValues`.
    for (name, declaration) in [
      (
        "width",
        StyleDeclaration::width as fn(Length) -> StyleDeclaration,
      ),
      ("height", StyleDeclaration::height),
    ] {
      if let Some(length) = self
        .attribute(name)
        .and_then(parse_dimension_value)
        .filter(|length| !length.is_zero())
      {
        declarations.push(declaration(length));
      }
    }

    if declarations.is_empty() {
      return;
    }

    let style = presentation.get_or_insert_default();

    for declaration in declarations {
      style.push(declaration, false);
    }
  }
}
