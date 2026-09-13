//! Cutting the content column into pages without splitting unsplittable atoms.

use std::sync::Arc;

use takumi_core::{
  geometry::transformed_rect_extents,
  layout::node::Node,
  scene::NodePaint,
  style::{GridPlacement, GridPlacementSpan},
};

use crate::{
  atoms::Atoms,
  bands::Repeatable,
  counters::{
    BoxPages, FlowCounters, has_page_counters, has_target_counters, substitute_target_counters,
  },
  inline::{TextBox, build_inline_map},
  interactive::Interactive,
  options::PdfError,
  page_context::PageContexts,
  tree::{PagedTree, PreparedTree, TreeInputs},
  window::Window,
};

/// An unsplittable box as `(top, bottom)` in content coordinates. Both are
/// finite: they come from resolved layout, which is what lets the cut search
/// sort and bisect them.
pub(crate) type Atom = (f32, f32);

/// One text box's lines in content coordinates, with its `widows` / `orphans`
/// minimums.
pub(crate) struct Paragraph {
  /// Line extents sorted top-down.
  pub lines: Vec<Atom>,
  /// Fewest lines a cut may leave at the bottom of a page (`orphans`).
  pub before: usize,
  /// Fewest lines a cut may push to the top of the next page (`widows`).
  pub after: usize,
}

impl Paragraph {
  fn top(&self) -> f32 {
    self.lines.first().map_or(0.0, |line| line.0)
  }

  fn bottom(&self) -> f32 {
    self.lines.last().map_or(0.0, |line| line.1)
  }

  /// Where a cut through the paragraph must move up to honor its minimums, or
  /// `cut` unchanged. A proposal at or above `floor` (the top of the current
  /// page) drops the constraint instead, like browsers do.
  fn cut_for_minimums(&self, cut: f32, floor: f32) -> f32 {
    let lines = &self.lines;
    // The atom pass keeps the cut off line interiors; the half point absorbs a
    // cut sitting exactly on a line edge.
    let before = lines.partition_point(|(_, bottom)| *bottom <= cut + 0.5);

    if before == 0 || before >= lines.len() {
      return cut;
    }
    let after = lines.len() - before;

    let proposed = if before < self.before {
      lines[0].0
    } else if after < self.after {
      // Blink's resolution: `max(line_count - widows, orphans)`. The orphans
      // floor wins; a floor at the current cut accepts the widow violation.
      let line_number = lines.len().saturating_sub(self.after).max(self.before);

      if line_number >= before {
        return cut;
      }
      lines[line_number].0
    } else {
      return cut;
    };

    if proposed <= floor {
      return cut;
    }
    cut.min(proposed)
  }
}

/// How many pages one render may cut its content into. A document tall enough
/// to pass this is not a document anyone reads; it is a way to spend a
/// renderer's memory.
pub(crate) const MAX_PAGES: usize = 20_000;

/// How many times pagination reruns to settle page counters. A counter widens
/// the line it sits on, which can move a cut, which can renumber the counter.
/// The second pass numbers a laid-out document, the third confirms the numbers
/// did not move, and a document still moving after that keeps the last pass's
/// numbers.
const COUNTER_PASSES: usize = 3;

/// A table's repeatable header rows, in content coordinates. On every page
/// that starts inside the table's body, the band paints again at the top of
/// the window and the body shifts below it.
pub(crate) struct HeaderBand {
  pub(crate) top: f32,
  pub(crate) bottom: f32,
  pub(crate) table_bottom: f32,
  /// Horizontal extent of the table, which the replay clips to so content
  /// beside the table does not repeat with it.
  pub(crate) left: f32,
  pub(crate) right: f32,
}

impl HeaderBand {
  pub(crate) fn height(&self) -> f32 {
    self.bottom - self.top
  }

  /// The band's own extent as a paint window.
  pub(crate) fn window(&self) -> Window {
    Window {
      y: Some((self.top, self.bottom)),
      x: Some((self.left, self.right)),
      lines: None,
    }
  }

  /// Whether a page starting at `y` shows this header again.
  fn repeats_at(&self, y: f32) -> bool {
    self.bottom <= y + 0.5 && y + 0.5 < self.table_bottom
  }

  /// Every table header band eligible to repeat: css-tables-3 §repeated-headers
  /// admits a header of at most a quarter of the fragmentainer.
  fn collect(tree: &PreparedTree, window: f32) -> Vec<Self> {
    let mut bands = Vec::new();

    tree.for_each_paint(|paint| Self::collect_paint(tree, paint, &mut bands));
    bands.retain(|band| band.height() > 0.0 && band.height() <= window / 4.0);
    bands.sort_by(|a, b| a.top.total_cmp(&b.top));
    bands
  }

  fn collect_paint(tree: &PreparedTree, paint: &NodePaint, bands: &mut Vec<Self>) {
    let Some(node) = tree.root.node_at_path(&paint.path) else {
      return;
    };
    let Some((start, end)) = node.table_header_lines else {
      return;
    };
    // Cell extents are measured in the table's own space, so anything beyond a
    // translation would disagree with the transformed band; such a table is a
    // monolithic atom anyway.
    let transform = paint.transform;

    if (transform.a - 1.0).abs() > 1e-3
      || (transform.d - 1.0).abs() > 1e-3
      || transform.b.abs() > 1e-3
      || transform.c.abs() > 1e-3
    {
      return;
    }
    let Ok(layout) = tree.results.layout(paint.node_id) else {
      return;
    };
    let Some((table_left, table_top, table_right, table_bottom)) = transformed_rect_extents(
      takumi_core::geometry::Point { x: 0.0, y: 0.0 },
      layout.size,
      paint.transform,
    ) else {
      return;
    };
    let Some(rows) = node.children.as_deref() else {
      return;
    };
    let Ok(children) = tree.results.box_children(paint.node_id) else {
      return;
    };
    let content_top = table_top + layout.border.top + layout.padding.top;
    let mut header_top = f32::MAX;
    let mut header_bottom = f32::MIN;
    let mut body_top = f32::MAX;

    for ordered in children.iter() {
      let Some(child) = rows.get(ordered.render_index) else {
        continue;
      };
      let GridPlacement::Line(line) = child.context.style.grid_row_start else {
        continue;
      };
      let Ok(cell) = tree.results.layout(ordered.node_id) else {
        continue;
      };

      if line >= start && line < end {
        // A header cell whose rowspan reaches into the body would replay body
        // area with the band; such a table does not repeat.
        let GridPlacement::Span(GridPlacementSpan::Span(rowspan)) =
          child.context.style.grid_row_end
        else {
          return;
        };

        if line.saturating_add(rowspan as i16) > end {
          return;
        }
        header_top = header_top.min(content_top + cell.location.y);
        header_bottom = header_bottom.max(content_top + cell.location.y + cell.size.height);
      } else if line >= end {
        body_top = body_top.min(content_top + cell.location.y);
      }
    }

    // The band starts at the header cells, not the table edge: a top caption
    // sits between the two and must not repeat. It runs to the first body row,
    // so the `border-spacing` strip repeats with it, as Blink reserves it.
    let band_bottom = if body_top < f32::MAX {
      body_top.max(header_bottom)
    } else {
      header_bottom
    };

    if header_top < f32::MAX && band_bottom > header_top {
      bands.push(Self {
        top: header_top,
        bottom: band_bottom,
        table_bottom,
        left: table_left,
        right: table_right,
      });
    }
  }

  /// The bands a page starting at `y` replays, each with the window offset its
  /// strip paints at, and the window height they take together. Bands whose
  /// source ranges overlap vertically sit side by side, so they share one
  /// strip instead of stacking; only bands below one another (nested
  /// continuations) stack.
  pub(crate) fn replays(headers: &[Self], y: f32, window: f32) -> (f32, Vec<(f32, usize)>) {
    let mut slots = Vec::new();
    let mut offset = 0.0_f32;
    let mut strip: Option<(f32, f32)> = None;

    for (index, band) in headers.iter().enumerate() {
      if !band.repeats_at(y) {
        continue;
      }
      match &mut strip {
        Some((bottom, height)) if band.top < *bottom => {
          slots.push((offset, index));
          *bottom = bottom.max(band.bottom);
          *height = height.max(band.height());
        }
        _ => {
          if let Some((_, height)) = strip.take() {
            offset += height;
          }
          slots.push((offset, index));
          strip = Some((band.bottom, band.height()));
        }
      }
    }
    let reserved = offset + strip.map_or(0.0, |(_, height)| height);

    // A band stack this tall starves the page; the headers stop repeating
    // rather than squeezing the body out.
    if reserved > window / 2.0 {
      (0.0, Vec::new())
    } else {
      (reserved, slots)
    }
  }
}

/// The content laid out and cut into pages, with the `fixed` boxes that repeat
/// on every page and the interactive targets the pages draw from.
pub(crate) struct Paginated {
  pub(crate) content: PreparedTree,
  pub(crate) repeated: Vec<Repeatable>,
  pub(crate) starts: Vec<f32>,
  pub(crate) interactive: Interactive,
  pub(crate) headers: Vec<HeaderBand>,
  pages: Vec<PageGeometry>,
}

/// What a page takes from its group: the page name, the window height, and
/// the column x its content starts at.
struct PageGeometry {
  name: Option<Arc<str>>,
  window: f32,
  left: f32,
}

/// The column extent a page group covers, and where its page-area box sits.
pub(crate) struct PageGroup {
  pub(crate) name: Arc<str>,
  pub(crate) left: f32,
  pub(crate) top: f32,
  pub(crate) bottom: f32,
}

/// The page groups of a column, in column order, with the extents of the
/// boxes outside every group that paint something, which tell whether a page
/// starting above a group holds anything else.
pub(crate) struct PageGroups {
  groups: Vec<PageGroup>,
  outside: Vec<Atom>,
}

impl PageGroups {
  /// `groups` come in column order.
  pub(crate) fn new(groups: Vec<PageGroup>, outside: Vec<Atom>) -> Self {
    Self { groups, outside }
  }

  /// The group a page starting at `y` draws on: the group `y` falls in, or
  /// the next group when only its own leading space lies between, as when a
  /// cover carries a top margin.
  pub(crate) fn at(&self, y: f32) -> Option<&PageGroup> {
    let group = self.groups.iter().find(|group| y + 0.5 < group.bottom)?;

    (group.top <= y + 0.5 || !self.shows_between(y, group)).then_some(group)
  }

  /// Whether a box outside every group shows between `y` and the group's top.
  fn shows_between(&self, y: f32, group: &PageGroup) -> bool {
    self
      .outside
      .iter()
      .any(|&(top, bottom)| bottom > y + 0.5 && top < group.top - 0.5)
  }
}

/// One page's window into the content column.
pub(crate) struct PageSlice {
  /// 0-based; display page numbers are `index + 1`.
  pub(crate) index: usize,
  /// The column x that lands at the page's left margin.
  pub(crate) left: f32,
  pub(crate) start: f32,
  /// Where the next page begins. The last page never runs out.
  pub(crate) end: f32,
  pub(crate) paint_height: f32,
  /// Window height repeated table headers take above the content.
  pub(crate) reserved: f32,
  /// The header bands this page replays, with each band's window offset.
  pub(crate) replays: Vec<(f32, usize)>,
}

impl Paginated {
  /// Lays the content out and cuts it into pages, refilling the counters it
  /// holds until the numbers stop moving. A counter inside a repeated box is
  /// left to that box, which lays out again per page with its own numbers.
  pub(crate) fn build(
    mut node: Node,
    inputs: &TreeInputs<'_>,
    contexts: &mut PageContexts,
  ) -> Result<Self, PdfError> {
    if !has_page_counters(&node) && !has_target_counters(&node) {
      return Self::build_once(node, inputs, contexts);
    }
    let mut previous: Vec<String> = Vec::new();
    let mut passes = 0;

    loop {
      let paginated = Self::build_once(node.clone(), inputs, contexts)?;
      let pages = paginated.starts.len();
      let mut written = Vec::new();

      passes += 1;
      if pages < MAX_PAGES {
        paginated.substitute_counters(&mut node, &mut written);
      }
      if written == previous {
        return Ok(paginated);
      }
      previous = written;
      // The numbers are still moving, and this was the last pass allowed. They
      // are laid out once more so the page shows the numbers it was cut with.
      if passes == COUNTER_PASSES {
        return Self::build_once(node, inputs, contexts);
      }
    }
  }

  fn build_once(
    node: Node,
    inputs: &TreeInputs<'_>,
    contexts: &mut PageContexts,
  ) -> Result<Self, PdfError> {
    let PagedTree {
      content,
      repeated,
      groups,
    } = inputs.prepare_paged(node, contexts)?;
    let text_boxes = TextBox::collect(&content);
    let inline_map = build_inline_map(&text_boxes)?;
    let mut atoms = content.atom_collector(Some(&inline_map)).collect()?;
    let headers = HeaderBand::collect(&content, contexts.unnamed().frame.window_height);

    // A repeating header is monolithic: a cut through it would show a partial
    // header once and the full band again on the next page.
    atoms
      .extents
      .extend(headers.iter().map(|band| (band.top, band.bottom)));
    let geometry = |y: f32| {
      let group = groups.at(y);
      let name = group.map(|group| group.name.clone());
      let window = contexts.get(name.as_deref()).frame.window_height;

      PageGeometry {
        name,
        window,
        left: group.map_or(0.0, |group| group.left),
      }
    };
    let window_at = |y: f32| geometry(y).window;
    let starts = atoms.page_starts(&headers, content.height, &window_at);
    let pages = starts.iter().map(|&start| geometry(start)).collect();
    let interactive = Interactive::collect(&content);

    Ok(Self {
      content,
      repeated,
      starts,
      interactive,
      headers,
      pages,
    })
  }

  /// The name of the page `index` draws on, or `None` for the unnamed page.
  pub(crate) fn page_name(&self, index: usize) -> Option<&str> {
    self.pages.get(index).and_then(|page| page.name.as_deref())
  }

  /// Fills both kinds of counter from this laid-out pass, collecting what it
  /// wrote so the caller can see whether the numbers moved.
  fn substitute_counters(&self, node: &mut Node, written: &mut Vec<String>) {
    let pages = self.starts.len();
    let page_at = |top: &f32| self.page_index(*top) + 1;
    // `fill_root` wraps the tree in a page-wide box, which takes preorder 0.
    let page_of = self
      .interactive
      .extents
      .iter()
      .filter_map(|(order, extent)| {
        let pages = BoxPages {
          start: page_at(&extent.top),
          next: extent.flow_bottom.as_ref().map(page_at),
        };

        Some((order.checked_sub(1)?, pages))
      })
      .collect();
    let repeated: Vec<_> = self
      .repeated
      .iter()
      .filter_map(Repeatable::source_orders)
      .map(|orders| orders.start.saturating_sub(1)..orders.end.saturating_sub(1))
      .collect();

    FlowCounters::new(&page_of, pages, &repeated).substitute(node, None, written);

    let anchors = &self.interactive.anchors;
    let target_page = |id: &str| anchors.get(id).map(|anchor| page_at(&anchor.top));

    substitute_target_counters(node, None, &target_page, written);
  }

  /// 0-based index of the page `top` falls on.
  pub(crate) fn page_index(&self, top: f32) -> usize {
    self
      .starts
      .partition_point(|start| *start <= top)
      .saturating_sub(1)
  }

  /// Window height repeated headers take on the page starting at `start`.
  pub(crate) fn reserved_at(&self, start: f32) -> f32 {
    let window = self.pages[self.page_index(start)].window;

    HeaderBand::replays(&self.headers, start, window).0
  }

  pub(crate) fn pages(&self) -> impl Iterator<Item = PageSlice> + '_ {
    self.starts.iter().enumerate().map(|(index, &start)| {
      let end = self.starts.get(index + 1).copied().unwrap_or(f32::INFINITY);
      let PageGeometry { window, left, .. } = self.pages[index];
      let (reserved, replays) = HeaderBand::replays(&self.headers, start, window);

      PageSlice {
        index,
        left,
        start,
        end,
        paint_height: (end - start).min(window - reserved),
        reserved,
        replays,
      }
    })
  }
}

/// Page start offsets for slicing `total` height into pages, each as tall as
/// `window_at` says for the column offset it starts at. Each cut moves up to
/// the top of any atom straddling it, repeated
/// until no atom straddles (a raised cut can land inside another atom). An
/// atom taller than the window can never fit a page, so it does not push cuts
/// at all — matching browsers, where `break-inside: avoid` is dropped for
/// boxes taller than the fragmentainer.
///
/// A forced cut with no content on its page above it is dropped, per
/// css-break-3 §forced-breaks. The column ends at its last content box.
///
/// Layout snaps box edges to whole pixels while the window is fractional, so
/// a box sized to the page can end up to [`ContentEdges::TOLERANCE`] past the window.
/// A cut within that distance of a content edge lands on the edge instead of
/// leaving a sub-pixel sliver on either page.
impl Atoms {
  pub(crate) fn page_starts(
    mut self,
    headers: &[HeaderBand],
    total: f32,
    window_at: &dyn Fn(f32) -> f32,
  ) -> Vec<f32> {
    let Self {
      extents,
      forced,
      paragraphs,
      content,
    } = &mut self;
    let overlaps = |top: f32, bottom: f32| {
      content
        .iter()
        .any(|&(box_top, box_bottom)| box_top < bottom - 0.5 && box_bottom > top + 0.5)
    };
    let total = content
      .iter()
      .fold(0.0_f32, |bottom, atom| bottom.max(atom.1))
      .min(total);
    let edges = ContentEdges::new(content);

    extents.sort_by(|a, b| a.0.total_cmp(&b.0));
    forced.retain(|cut| *cut < total - 1.0);
    forced.sort_by(f32::total_cmp);

    // The prefix max of bottoms lets the back-scan stop early even when a
    // paragraph spans several pages.
    let mut by_top: Vec<&Paragraph> = paragraphs.iter().collect();

    by_top.sort_by(|a, b| a.top().total_cmp(&b.top()));

    let mut prefix_max_bottom = Vec::with_capacity(by_top.len());
    let mut running = f32::MIN;

    for paragraph in &by_top {
      running = running.max(paragraph.bottom());
      prefix_max_bottom.push(running);
    }
    let mut starts = vec![0.0_f32];
    let mut y0 = 0.0_f32;

    loop {
      let window = window_at(y0);
      let limit = edges.snap(
        y0 + window - HeaderBand::replays(headers, y0, window).0,
        y0 + 1.0,
      );

      if let Some(cut) = forced
        .iter()
        .copied()
        .find(|cut| *cut > y0 + 1.0 && overlaps(y0, *cut))
        && cut <= limit
      {
        starts.push(cut);
        y0 = cut;
        continue;
      }
      if limit >= total {
        break;
      }
      let mut cut = limit;

      loop {
        // `extents` is sorted by top, and an atom that fits the window can only
        // straddle the cut if it starts within one window of it, so the scan
        // walks back from the cut and stops there instead of reading every atom.
        let straddling = extents.partition_point(|(top, _)| *top < cut);
        let mut pushed_up = cut;

        for &(top, bottom) in extents[..straddling].iter().rev() {
          if top <= cut - window {
            break;
          }
          // An atom moves to the next page only when it fits the capacity that
          // page actually offers under its repeated headers.
          let next = window_at(top);

          if bottom > cut && bottom - top <= next - HeaderBand::replays(headers, top, next).0 {
            pushed_up = pushed_up.min(top);
          }
        }

        let straddling = by_top.partition_point(|paragraph| paragraph.top() < pushed_up);

        for i in (0..straddling).rev() {
          if prefix_max_bottom[i] <= pushed_up {
            break;
          }
          pushed_up = pushed_up.min(by_top[i].cut_for_minimums(pushed_up, y0 + 1.0));
        }

        if pushed_up >= cut {
          break;
        }
        if pushed_up <= y0 + 1.0 {
          cut = limit;
          break;
        }
        cut = pushed_up;
      }

      // The page cap is what bounds this loop; a cut that did not move would
      // spin forever without it, so refuse that too rather than lean on the cap.
      if cut <= y0 {
        break;
      }
      starts.push(cut);
      y0 = cut;

      if starts.len() >= MAX_PAGES {
        break;
      }
    }
    starts
  }
}

/// The tops and bottoms of the content boxes, sorted, which a cut lands on
/// when it falls within a pixel of one.
struct ContentEdges(Vec<f32>);

impl ContentEdges {
  /// How far a snapped box edge can sit from the fractional cut it belongs
  /// on: half a pixel from snapping the edge, half from snapping the cut
  /// before it.
  const TOLERANCE: f32 = 1.0;

  fn new(content: &[Atom]) -> Self {
    let mut edges: Vec<f32> = content
      .iter()
      .flat_map(|&(top, bottom)| [top, bottom])
      .collect();

    edges.sort_by(f32::total_cmp);
    Self(edges)
  }

  /// The edge closest to `limit` within [`Self::TOLERANCE`] and above
  /// `floor`, or `limit` itself.
  fn snap(&self, limit: f32, floor: f32) -> f32 {
    let after = self.0.partition_point(|edge| *edge < limit);
    let before = after.checked_sub(1).map(|index| self.0[index]);

    [before, self.0.get(after).copied()]
      .into_iter()
      .flatten()
      .filter(|edge| (edge - limit).abs() <= Self::TOLERANCE && *edge > floor)
      .min_by(|a, b| (a - limit).abs().total_cmp(&(b - limit).abs()))
      .unwrap_or(limit)
  }
}
