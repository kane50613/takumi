//! Cutting the content column into pages without splitting unsplittable atoms.

/// Unsplittable vertical extents in content coordinates: text lines, images,
/// and transformed subtrees (which cannot be windowed without distortion).
/// An unsplittable box as `(top, bottom)` in content coordinates. Both are
/// finite: they come from resolved layout, which is what lets the cut search
/// sort and bisect them.
pub(crate) type Atom = (f32, f32);

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

pub(crate) fn page_starts(
  atoms: &mut [Atom],
  forced: &mut Vec<f32>,
  total: f32,
  window: f32,
) -> Vec<f32> {
  atoms.sort_by(|a, b| a.0.total_cmp(&b.0));
  forced.retain(|cut| *cut > 1.0 && *cut < total - 1.0);
  forced.sort_by(f32::total_cmp);

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
