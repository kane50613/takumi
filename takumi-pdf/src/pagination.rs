//! Cutting the content column into pages without splitting unsplittable atoms.

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
  page::PageFrame,
  tree::{PreparedTree, TreeInputs},
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
  window: f32,
}

/// One page's window into the content column.
pub(crate) struct PageSlice {
  /// 0-based; display page numbers are `index + 1`.
  pub(crate) index: usize,
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
    frame: &PageFrame,
  ) -> Result<Self, PdfError> {
    if !has_page_counters(&node) && !has_target_counters(&node) {
      return Self::build_once(node, inputs, frame);
    }
    let mut previous: Vec<String> = Vec::new();
    let mut passes = 0;

    loop {
      let paginated = Self::build_once(node.clone(), inputs, frame)?;
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
        return Self::build_once(node, inputs, frame);
      }
    }
  }

  fn build_once(node: Node, inputs: &TreeInputs<'_>, frame: &PageFrame) -> Result<Self, PdfError> {
    let (content, repeated) = inputs.prepare_paged(node, frame)?;
    let text_boxes = TextBox::collect(&content);
    let inline_map = build_inline_map(&text_boxes)?;
    let mut atoms = content.atom_collector(Some(&inline_map)).collect()?;
    let headers = HeaderBand::collect(&content, frame.window_height);

    // A repeating header is monolithic: a cut through it would show a partial
    // header once and the full band again on the next page.
    atoms
      .extents
      .extend(headers.iter().map(|band| (band.top, band.bottom)));
    let starts = atoms.page_starts(&headers, content.height, frame.window_height);
    let interactive = Interactive::collect(&content);

    Ok(Self {
      content,
      repeated,
      starts,
      interactive,
      headers,
      window: frame.window_height,
    })
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
    HeaderBand::replays(&self.headers, start, self.window).0
  }

  pub(crate) fn pages(&self) -> impl Iterator<Item = PageSlice> + '_ {
    self.starts.iter().enumerate().map(|(index, &start)| {
      let end = self.starts.get(index + 1).copied().unwrap_or(f32::INFINITY);
      let (reserved, replays) = HeaderBand::replays(&self.headers, start, self.window);

      PageSlice {
        index,
        start,
        end,
        paint_height: (end - start).min(self.window - reserved),
        reserved,
        replays,
      }
    })
  }
}

/// Page start offsets for slicing `total` height into windows of `window`
/// height. Each cut moves up to the top of any atom straddling it, repeated
/// until no atom straddles (a raised cut can land inside another atom). An
/// atom taller than the window can never fit a page, so it does not push cuts
/// at all — matching browsers, where `break-inside: avoid` is dropped for
/// boxes taller than the fragmentainer.
impl Atoms {
  pub(crate) fn page_starts(mut self, headers: &[HeaderBand], total: f32, window: f32) -> Vec<f32> {
    let Self {
      extents,
      forced,
      paragraphs,
    } = &mut self;

    extents.sort_by(|a, b| a.0.total_cmp(&b.0));
    forced.retain(|cut| *cut > 1.0 && *cut < total - 1.0);
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
      let limit = y0 + window - HeaderBand::replays(headers, y0, window).0;

      if let Some(cut) = forced.iter().copied().find(|cut| *cut > y0 + 1.0)
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
          if bottom > cut && bottom - top <= window - HeaderBand::replays(headers, top, window).0 {
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
