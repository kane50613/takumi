//! The table box tree: Blink's anonymous table wrappers, spans, and `TableGroupedChildren`.

use std::mem::take;

use crate::{
  context::RenderContext,
  layout::tree::{NodeOrigin, RenderNode},
  style::{BorderCollapse, ComputedStyle, Display},
};

/// Blink's `kMaxColSpan` (`core/html/table_constants.h`).
const MAX_COLSPAN: u32 = 1000;

/// Blink's `kMaxRowSpan`.
pub(super) const MAX_ROWSPAN: u32 = 65534;

/// The part of a table a box with some `display` plays, as Blink sorts its layout objects.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TableRole {
  Table,
  Section,
  Row,
  ColumnGroup,
  Column,
}

impl TableRole {
  fn of(display: Display) -> Option<Self> {
    match display {
      Display::Table | Display::InlineTable => Some(Self::Table),
      Display::TableHeaderGroup | Display::TableRowGroup | Display::TableFooterGroup => {
        Some(Self::Section)
      }
      Display::TableRow => Some(Self::Row),
      Display::TableColumnGroup => Some(Self::ColumnGroup),
      Display::TableColumn => Some(Self::Column),
      _ => None,
    }
  }

  /// Blink's `CanBeDirectChild` of `LayoutTable`, `LayoutTableSection`, and `LayoutTableRow`, and
  /// `LayoutTableColumn::IsChildAllowed`.
  fn accepts(self, display: Display) -> bool {
    match self {
      Self::Table => {
        display == Display::TableCaption || display.is_table_column() || display.is_table_section()
      }
      Self::Section => display == Display::TableRow,
      Self::Row => display == Display::TableCell,
      Self::ColumnGroup => display == Display::TableColumn,
      Self::Column => false,
    }
  }

  /// The anonymous box Blink's `CreateAnonymousChild` wraps children this role does not accept
  /// in, or `None` when it drops them instead.
  fn wrapper(self) -> Option<Display> {
    match self {
      Self::Table => Some(Display::TableRowGroup),
      Self::Section => Some(Display::TableRow),
      Self::Row => Some(Display::TableCell),
      Self::ColumnGroup | Self::Column => None,
    }
  }
}

impl RenderNode {
  /// Whether a box `context` styles places its children as table parts: a table, row group, row,
  /// column group, or column.
  pub(in crate::layout) fn holds_table_parts(context: &RenderContext) -> bool {
    TableRole::of(context.style.base_data.display).is_some()
  }

  /// `children` of a table part, each in a parent that accepts it, as Blink's
  /// `LayoutBlock::AddChild` wraps them; `None` for any other display.
  pub(in crate::layout) fn fix_up_table_children(
    context: &RenderContext,
    children: Vec<RenderNode>,
  ) -> Option<Vec<RenderNode>> {
    let role = TableRole::of(context.style.base_data.display)?;
    let mut fixed = Vec::with_capacity(children.len());
    let mut run = Vec::new();

    for child in children {
      // Blink's `CanHaveWhitespaceChildren`: white space a table part holds renders only after
      // text, approximated as inside a run already wrapped, or when it preserves its spaces.
      if child.is_collapsible_whitespace_only_text_node() && run.is_empty() {
        continue;
      }

      if role.accepts(child.context.style.base_data.display) {
        Self::flush_wrapped_run(context, role, &mut run, &mut fixed);
        fixed.push(child);
      } else if role.wrapper().is_some() {
        run.push(child);
      }
    }

    Self::flush_wrapped_run(context, role, &mut run, &mut fixed);

    if role == TableRole::Table
      && context.style.inherited_data.border_collapse == BorderCollapse::Collapse
    {
      let sections = fixed
        .iter_mut()
        .filter(|child| child.context.style.base_data.display.is_table_section());

      for section in sections {
        for row in section.children.as_deref_mut().into_iter().flatten() {
          for cell in row.children.as_deref_mut().into_iter().flatten() {
            cell.context.collapsed_borders = true;
          }
        }
      }
    }

    Some(fixed)
  }

  fn flush_wrapped_run(
    context: &RenderContext,
    role: TableRole,
    run: &mut Vec<RenderNode>,
    fixed: &mut Vec<RenderNode>,
  ) {
    let Some(display) = role.wrapper().filter(|_| {
      run
        .iter()
        .any(|child| !child.is_collapsible_whitespace_only_text_node())
    }) else {
      run.clear();
      return;
    };
    let mut wrapper = Self::anonymous_table_box(context, display, Vec::new());

    wrapper.children = Some(Self::normalize_children(
      &mut wrapper.context,
      take(run).into_boxed_slice(),
      false,
    ));
    fixed.push(wrapper);
  }

  /// Wraps each run of table parts outside a table in an anonymous table, as Blink's
  /// `LayoutObject::AddChild` does.
  pub(in crate::layout) fn wrap_orphan_table_parts(
    parent: &RenderContext,
    children: Box<[RenderNode]>,
  ) -> Box<[RenderNode]> {
    let is_part = |child: &RenderNode| child.context.style.base_data.display.is_table_part();

    if TableRole::of(parent.style.base_data.display).is_some() || !children.iter().any(is_part) {
      return children;
    }

    let display = if parent.style.base_data.display.is_inline() {
      Display::InlineTable
    } else {
      Display::Table
    };
    let mut wrapped = Vec::with_capacity(children.len());
    let mut run = Vec::new();
    // Collapsible white space between two parts belongs to neither, and goes.
    let mut gap = Vec::new();
    let flush = |run: &mut Vec<RenderNode>, wrapped: &mut Vec<RenderNode>| {
      if run.is_empty() {
        return;
      }
      let mut table = Self::anonymous_table_box(parent, display, Vec::new());

      table.children = Some(Self::normalize_children(
        &mut table.context,
        take(run).into_boxed_slice(),
        false,
      ));
      wrapped.push(table);
    };

    for child in children {
      if is_part(&child) {
        gap.clear();
        run.push(child);
      } else if !run.is_empty() && child.is_collapsible_whitespace_only_text_node() {
        gap.push(child);
      } else {
        flush(&mut run, &mut wrapped);
        wrapped.append(&mut gap);
        wrapped.push(child);
      }
    }

    flush(&mut run, &mut wrapped);
    wrapped.append(&mut gap);

    wrapped.into_boxed_slice()
  }

  /// A `display` box generated inside `parent` around `children`, styled only by what it
  /// inherits, as Blink's `CreateAnonymousStyleBuilderWithDisplay` styles it.
  fn anonymous_table_box(
    parent: &RenderContext,
    display: Display,
    children: Vec<RenderNode>,
  ) -> RenderNode {
    let mut style = ComputedStyle::from_parent(&parent.style);

    style.base_data_mut().display = display;
    style.make_computed(&parent.sizing);

    RenderNode::new(
      RenderContext::from_parent(parent, style, parent.sizing.clone(), parent.current_color),
      NodeOrigin::Anonymous,
      None,
      Some(children.into_boxed_slice()),
    )
  }

  /// The row groups in layout order, as Blink's `TableGroupedChildren` orders them.
  pub fn table_sections(&self) -> Vec<(usize, TableSectionKind)> {
    if !self.context.style.base_data.display.is_table() {
      return Vec::new();
    }

    let grouped = TableGroupedChildren::of(self);

    grouped
      .sections()
      .map(|(index, is_body)| {
        let kind = if is_body {
          TableSectionKind::Body
        } else if Some(index) == grouped.header {
          TableSectionKind::Header
        } else {
          TableSectionKind::Footer
        };

        (index, kind)
      })
      .collect()
  }

  /// Blink's `LayoutTableCell::ColSpan`: a `td` or `th` element's `colspan` attribute, from 1 to
  /// 1000.
  pub fn table_cell_colspan(&self) -> usize {
    self
      .span_attribute(&["td", "th"], "colspan", 1, MAX_COLSPAN)
      .map_or(1, |span| span as usize)
  }

  /// Blink's `LayoutTableCell::ComputedRowSpan`: a `td` or `th` element's `rowspan` attribute up
  /// to 65534, a zero spanning every row left.
  pub fn table_cell_rowspan(&self) -> usize {
    match self.span_attribute(&["td", "th"], "rowspan", 0, MAX_ROWSPAN) {
      Some(0) => MAX_ROWSPAN as usize,
      Some(span) => span as usize,
      None => 1,
    }
  }

  /// Blink's `LayoutTableColumn::Span`: a `col` or `colgroup` element's `span` attribute, from 1
  /// to 1000.
  pub(crate) fn table_column_span(&self) -> usize {
    self
      .span_attribute(&["col", "colgroup"], "span", 1, MAX_COLSPAN)
      .map_or(1, |span| span as usize)
  }

  /// The attribute `name` of an element tagged one of `tags`, parsed by HTML's rules for clamped
  /// non-negative integers, or `None` when it is missing or holds no number.
  fn span_attribute(&self, tags: &[&str], name: &str, min: u32, max: u32) -> Option<u32> {
    let node = self.node.as_ref()?;
    let tag = node.tag_name()?;

    if !tags
      .iter()
      .any(|candidate| tag.eq_ignore_ascii_case(candidate))
    {
      return None;
    }

    parse_non_negative_integer(node.attribute(name)?).map(|parsed| parsed.clamp(min, max))
  }

  /// Blink's `IsEmptyTableSection`: a row group without rows.
  pub(super) fn is_empty_table_section(&self) -> bool {
    self.children.as_deref().is_none_or(<[_]>::is_empty)
  }
}

/// HTML's [non-negative integers](https://html.spec.whatwg.org/multipage/common-microsyntaxes.html#rules-for-parsing-non-negative-integers),
/// saturating as Blink's `ParseHTMLClampedNonNegativeInteger`.
fn parse_non_negative_integer(value: &str) -> Option<u32> {
  let value = value.trim_start_matches(['\t', '\n', '\x0C', '\r', ' ']);
  let value = value.strip_prefix('+').unwrap_or(value);
  let digits = value.len() - value.trim_start_matches(|c: char| c.is_ascii_digit()).len();

  if digits == 0 {
    return None;
  }

  Some(value[..digits].bytes().fold(0u32, |total, digit| {
    total
      .saturating_mul(10)
      .saturating_add(u32::from(digit - b'0'))
  }))
}

/// The role a row group plays in its table's layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableSectionKind {
  /// The first `table-header-group`, which a paged table may repeat atop each page.
  Header,
  /// A `table-row-group`, or a header or footer group after the first.
  Body,
  /// The first `table-footer-group`, which a paged table may repeat below each page.
  Footer,
}

/// A table's child indices by role, as Blink's `TableGroupedChildren` sorts them.
pub(crate) struct TableGroupedChildren {
  pub(crate) captions: Vec<usize>,
  pub(crate) columns: Vec<usize>,
  pub(crate) header: Option<usize>,
  pub(crate) bodies: Vec<usize>,
  pub(crate) footer: Option<usize>,
}

impl TableGroupedChildren {
  pub(crate) fn of(table: &RenderNode) -> Self {
    let mut grouped = Self {
      captions: Vec::new(),
      columns: Vec::new(),
      header: None,
      bodies: Vec::new(),
      footer: None,
    };

    for (index, child) in table.children.iter().flatten().enumerate() {
      match child.context.style.base_data.display {
        Display::TableCaption => grouped.captions.push(index),
        Display::TableColumn | Display::TableColumnGroup => grouped.columns.push(index),
        Display::TableHeaderGroup if grouped.header.is_none() => grouped.header = Some(index),
        Display::TableFooterGroup if grouped.footer.is_none() => grouped.footer = Some(index),
        _ => grouped.bodies.push(index),
      }
    }

    grouped
  }

  /// The sections in layout order, each with whether it counts as a body, as Blink's
  /// `TableGroupedChildrenIterator` walks them.
  pub(crate) fn sections(&self) -> impl Iterator<Item = (usize, bool)> + '_ {
    self
      .header
      .map(|header| (header, false))
      .into_iter()
      .chain(self.bodies.iter().map(|&body| (body, true)))
      .chain(self.footer.map(|footer| (footer, false)))
  }
}

#[cfg(test)]
mod tests {
  use super::parse_non_negative_integer;

  #[test]
  fn spans_parse_as_html_reads_non_negative_integers() {
    assert_eq!(parse_non_negative_integer(" 3px"), Some(3));
    assert_eq!(parse_non_negative_integer("+2"), Some(2));
    assert_eq!(parse_non_negative_integer("-2"), None);
    assert_eq!(parse_non_negative_integer("x"), None);
    assert_eq!(parse_non_negative_integer("99999999999999"), Some(u32::MAX));
  }
}
