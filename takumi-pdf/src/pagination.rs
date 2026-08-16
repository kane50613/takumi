//! Cutting the content column into pages without splitting unsplittable atoms.

use std::{cell::RefCell, ops::Range};

use takumi_core::{layout::node::Node, style::Affine, viewport::Viewport};

use crate::{
  counters::{
    has_page_counters, has_target_counters, substitute_flow_page_counters,
    substitute_target_counters,
  },
  emitter::{FontMap, RenderIssues},
  inline::{build_inline_map, collect_text_boxes},
  interactive::{Interactive, collect_interactive},
  options::PdfError,
  tree::{PreparedTree, RepeatedBox, TreeInputs, prepare_paged_tree},
};

/// Unsplittable vertical extents in content coordinates: text lines, images,
/// and transformed subtrees (which cannot be windowed without distortion).
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
}

/// Page start offsets for slicing `total` height into windows of `window`
/// height. Each cut moves up to the top of any atom straddling it, repeated
/// until no atom straddles (a raised cut can land inside another atom). An
/// atom taller than the window can never fit a page, so it does not push cuts
/// at all — matching browsers, where `break-inside: avoid` is dropped for
/// boxes taller than the fragmentainer.
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

/// The content laid out and cut into pages, with the `fixed` boxes that repeat
/// on every page and the interactive targets the pages draw from.
pub(crate) struct Paginated {
  pub(crate) content: PreparedTree,
  pub(crate) repeated: Vec<RepeatedBox>,
  pub(crate) starts: Vec<f32>,
  pub(crate) interactive: Interactive,
}

/// What a pagination pass lays out against: the content column, the page area a
/// repeated box positions in, and the height one page shows.
pub(crate) struct PageGeometry {
  pub(crate) viewport: Viewport,
  pub(crate) page_area: Viewport,
  pub(crate) window_height: f32,
}

/// Lays the content out and cuts it into pages, refilling the counters it holds
/// until the numbers stop moving. A counter inside a repeated box is left to
/// that box, which lays out again per page with its own numbers.
pub(crate) fn paginate(
  mut node: Node,
  inputs: &TreeInputs<'_>,
  fonts: &mut FontMap,
  issues: &RefCell<RenderIssues>,
  document_lang: Option<&str>,
  geometry: &PageGeometry,
) -> Result<Paginated, PdfError> {
  if !has_page_counters(&node) && !has_target_counters(&node) {
    return paginate_once(node, inputs, fonts, issues, document_lang, geometry);
  }
  let mut previous: Vec<String> = Vec::new();
  let mut passes = 0;

  loop {
    let paginated = paginate_once(node.clone(), inputs, fonts, issues, document_lang, geometry)?;
    let pages = paginated.starts.len();
    let mut written = Vec::new();

    passes += 1;
    if pages < MAX_PAGES {
      substitute_counters(&mut node, &paginated, &mut written);
    }
    if written == previous {
      return Ok(paginated);
    }
    previous = written;
    // The numbers are still moving, and this was the last pass allowed. They
    // are laid out once more so the page shows the numbers it was cut with.
    if passes == COUNTER_PASSES {
      return paginate_once(node, inputs, fonts, issues, document_lang, geometry);
    }
  }
}

/// Fills both kinds of counter from one laid-out pass, collecting what it wrote
/// so the caller can see whether the numbers moved.
fn substitute_counters(node: &mut Node, paginated: &Paginated, written: &mut Vec<String>) {
  let pages = paginated.starts.len();
  let page_at = |top: &f32| {
    paginated
      .starts
      .partition_point(|start| start <= top)
      .max(1)
  };
  // `fill_root` wraps the tree in a page-wide box, which takes preorder 0.
  let page_of = paginated
    .interactive
    .boxes
    .iter()
    .filter_map(|(index, top)| Some((index.checked_sub(1)?, page_at(top))))
    .collect();
  let repeated: Vec<Range<usize>> = paginated
    .repeated
    .iter()
    .filter_map(|repeat| repeat.template.as_ref())
    .map(|template| {
      let range = template.source_range();

      range.start.saturating_sub(1)..range.end.saturating_sub(1)
    })
    .collect();

  substitute_flow_page_counters(node, &page_of, pages, &repeated, &mut 0, None, written);

  let anchors = &paginated.interactive.anchors;
  let target_page = |id: &str| anchors.get(id).map(|anchor| page_at(&anchor.top));

  substitute_target_counters(node, None, &target_page, written);
}

fn paginate_once(
  node: Node,
  inputs: &TreeInputs<'_>,
  fonts: &mut FontMap,
  issues: &RefCell<RenderIssues>,
  document_lang: Option<&str>,
  geometry: &PageGeometry,
) -> Result<Paginated, PdfError> {
  let (content, repeated) =
    prepare_paged_tree(inputs, node, geometry.viewport, geometry.page_area)?;
  let text_boxes = collect_text_boxes(&content);
  let inline_map = build_inline_map(&text_boxes)?;
  let mut atoms = Vec::new();
  let mut forced = Vec::new();
  let mut paragraphs = Vec::new();

  content
    .emitter(fonts, Some(&inline_map), None, issues, document_lang)
    .collect_atoms(
      0,
      Affine::IDENTITY,
      &mut atoms,
      &mut forced,
      &mut paragraphs,
    )?;

  let starts = page_starts(
    &mut atoms,
    &mut forced,
    &paragraphs,
    content.height,
    geometry.window_height,
  );
  let interactive = collect_interactive(&content);

  Ok(Paginated {
    content,
    repeated,
    starts,
    interactive,
  })
}

pub(crate) fn page_starts(
  atoms: &mut [Atom],
  forced: &mut Vec<f32>,
  paragraphs: &[Paragraph],
  total: f32,
  window: f32,
) -> Vec<f32> {
  atoms.sort_by(|a, b| a.0.total_cmp(&b.0));
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
    let limit = y0 + window;

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
      // `atoms` is sorted by top, and an atom that fits the window can only
      // straddle the cut if it starts within one window of it, so the scan
      // walks back from the cut and stops there instead of reading every atom.
      let straddling = atoms.partition_point(|(top, _)| *top < cut);
      let mut pushed_up = cut;

      for &(top, bottom) in atoms[..straddling].iter().rev() {
        if top <= cut - window {
          break;
        }
        if bottom > cut && bottom - top <= window {
          pushed_up = pushed_up.min(top);
        }
      }

      let straddling = by_top.partition_point(|paragraph| paragraph.top() < pushed_up);

      for i in (0..straddling).rev() {
        if prefix_max_bottom[i] <= pushed_up {
          break;
        }
        pushed_up = pushed_up.min(widow_orphan_cut(by_top[i], pushed_up, y0 + 1.0));
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

/// Where a cut through `paragraph` must move up to honor its minimums, or
/// `cut` unchanged. A proposal at or above `floor` (the top of the current
/// page) drops the constraint instead, like browsers do.
fn widow_orphan_cut(paragraph: &Paragraph, cut: f32, floor: f32) -> f32 {
  let lines = &paragraph.lines;
  // The atom pass keeps the cut off line interiors; the half point absorbs a
  // cut sitting exactly on a line edge.
  let before = lines.partition_point(|(_, bottom)| *bottom <= cut + 0.5);

  if before == 0 || before >= lines.len() {
    return cut;
  }
  let after = lines.len() - before;

  let proposed = if before < paragraph.before {
    lines[0].0
  } else if after < paragraph.after {
    // Blink's resolution: `max(line_count - widows, orphans)`. The orphans
    // floor wins; a floor at the current cut accepts the widow violation.
    let line_number = (lines.len() - paragraph.after).max(paragraph.before);

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
