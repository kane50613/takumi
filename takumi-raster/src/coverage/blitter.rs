//! The A8 coverage target and the additive blitters the analytic scan converter accumulates
//! partial coverage through, ported from `SkA8_Coverage_Blitter` (`src/core/SkBlitter_A8.cpp`),
//! the `SkBlitter` defaults it inherits (`src/core/SkBlitter.cpp`), and `MaskAdditiveBlitter`,
//! `RunBasedAdditiveBlitter` and `SafeRLEAdditiveBlitter` (`src/core/SkScan_AAAPath.cpp`).

/// An integer rect, `SkIRect`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct IntBounds {
  pub(super) left: i32,
  pub(super) top: i32,
  pub(super) right: i32,
  pub(super) bottom: i32,
}

impl IntBounds {
  pub(super) const fn width(self) -> i32 {
    self.right - self.left
  }

  pub(super) const fn height(self) -> i32 {
    self.bottom - self.top
  }

  pub(super) const fn contains(self, other: Self) -> bool {
    self.left <= other.left
      && self.top <= other.top
      && self.right >= other.right
      && self.bottom >= other.bottom
  }

  pub(super) fn union(self, other: Self) -> Self {
    Self {
      left: self.left.min(other.left),
      top: self.top.min(other.top),
      right: self.right.max(other.right),
      bottom: self.bottom.max(other.bottom),
    }
  }
}

/// `SkA8_Coverage_Blitter` over a `width`×`height` mask, which stores coverage instead of blending.
///
/// Writes outside the mask are dropped: the scan converter runs against a clip that also covers
/// the path's control points, as a device-sized clip would, and only the mask is kept.
pub(super) struct Coverage {
  pub(super) data: Vec<u8>,
  width: i32,
  height: i32,
}

impl Coverage {
  pub(super) fn new(width: u32, height: u32) -> Self {
    Self {
      data: vec![0; width as usize * height as usize],
      width: width as i32,
      height: height as i32,
    }
  }

  pub(super) const fn size(&self) -> (i32, i32) {
    (self.width, self.height)
  }

  /// The mask row `y` from `x` up to `len` pixels, cut to the mask.
  fn span(&mut self, x: i32, y: i32, len: i32) -> (&mut [u8], usize) {
    if y < 0 || y >= self.height || len <= 0 {
      return (&mut [], 0);
    }

    let start = x.max(0);
    let end = (x + len).min(self.width);

    if start >= end {
      return (&mut [], 0);
    }

    let row = y as usize * self.width as usize;

    (
      &mut self.data[row + start as usize..row + end as usize],
      (start - x) as usize,
    )
  }

  /// `SkA8_Coverage_Blitter::blitAntiH`, storing each non-zero alpha of a run.
  pub(super) fn blit_anti_h(&mut self, x: i32, y: i32, alphas: &[u8]) {
    let (span, skip) = self.span(x, y, alphas.len() as i32);

    for (pixel, alpha) in span.iter_mut().zip(&alphas[skip..]) {
      if *alpha != 0 {
        *pixel = *alpha;
      }
    }
  }

  /// `SkA8_Coverage_Blitter::blitH`.
  pub(super) fn blit_h(&mut self, x: i32, y: i32, width: i32) {
    self.span(x, y, width).0.fill(0xFF);
  }

  /// `SkA8_Coverage_Blitter::blitV`.
  pub(super) fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: u8) {
    if alpha == 0 {
      return;
    }
    for row in y..y + height {
      self.span(x, row, 1).0.fill(alpha);
    }
  }

  /// `SkA8_Coverage_Blitter::blitRect`.
  pub(super) fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
    for row in y..y + height {
      self.blit_h(x, row, width);
    }
  }

  /// `SkBlitter::blitAntiRect`.
  pub(super) fn blit_anti_rect(
    &mut self,
    mut x: i32,
    y: i32,
    width: i32,
    height: i32,
    left_alpha: u8,
    right_alpha: u8,
  ) {
    if left_alpha > 0 {
      self.blit_v(x, y, height, left_alpha);
    }
    x += 1;
    if width > 0 {
      self.blit_rect(x, y, width, height);
      x += width;
    }
    if right_alpha > 0 {
      self.blit_v(x, y, height, right_alpha);
    }
  }

  /// `SkBlitter::blitFatAntiRect` for a rect at least three pixels wide.
  pub(super) fn blit_fat_anti_rect(&mut self, rect: tiny_skia::Rect) {
    let bounds = round_out(rect);

    if bounds.height() == 0 {
      return;
    }

    let width = bounds.width() as usize;
    let partial_left = bounds.left as f32 + 1.0 - rect.left();
    let partial_right = rect.right() - (bounds.right as f32 - 1.0);
    let mut partial_top = bounds.top as f32 + 1.0 - rect.top();
    let partial_bottom = rect.bottom() - (bounds.bottom as f32 - 1.0);

    if bounds.height() == 1 {
      partial_top = rect.bottom() - rect.top();
    }

    let mut alphas = vec![0; width];
    let mut edge_row = |coverage: &mut Self, y: i32, partial: f32| {
      alphas.fill(scalar_to_alpha(partial));
      alphas[0] = scalar_to_alpha(partial_left * partial);
      alphas[width - 1] = scalar_to_alpha(partial_right * partial);
      coverage.blit_anti_h(bounds.left, y, &alphas);
    };

    edge_row(self, bounds.top, partial_top);
    if bounds.height() > 2 {
      self.blit_anti_rect(
        bounds.left,
        bounds.top + 1,
        bounds.width() - 2,
        bounds.height() - 2,
        scalar_to_alpha(partial_left),
        scalar_to_alpha(partial_right),
      );
    }
    if bounds.height() > 1 {
      edge_row(self, bounds.bottom - 1, partial_bottom);
    }
  }
}

/// `SkRect::roundOut`.
pub(super) fn round_out(rect: tiny_skia::Rect) -> IntBounds {
  IntBounds {
    left: rect.left().floor() as i32,
    top: rect.top().floor() as i32,
    right: rect.right().ceil() as i32,
    bottom: rect.bottom().ceil() as i32,
  }
}

/// `scalar_to_alpha` in `SkBlitter.cpp`.
fn scalar_to_alpha(value: f32) -> u8 {
  snap_alpha((value * 255.0) as u8)
}

/// Snaps alphas within 8 of 0 or 255 to them, as `RunBasedAdditiveBlitter::snapAlpha` does.
const fn snap_alpha(alpha: u8) -> u8 {
  if alpha > 247 {
    0xFF
  } else if alpha < 8 {
    0
  } else {
    alpha
  }
}

/// `add_alpha`, whose `SkAlphaRuns::CatchOverflow` maps 256 to 255.
pub(super) fn add_alpha(alpha: &mut u8, delta: u8) {
  let sum = i32::from(*alpha) + i32::from(delta);

  *alpha = (sum - (sum >> 8)) as u8;
}

/// `safely_add_alpha`.
pub(super) fn safely_add_alpha(alpha: &mut u8, delta: u8) {
  *alpha = alpha.saturating_add(delta);
}

/// `MaskAdditiveBlitter`: accumulates a small path into its own mask, then copies it out.
pub(super) struct MaskBlitter {
  bounds: IntBounds,
  /// One spare byte at either end takes the writes precision error puts one pixel outside.
  storage: Vec<u8>,
}

impl MaskBlitter {
  /// `kMAX_WIDTH`.
  const MAX_WIDTH: i32 = 32;
  /// `kMAX_STORAGE`.
  const MAX_STORAGE: i64 = 1024;

  /// `MaskAdditiveBlitter::CanHandleRect`.
  pub(super) fn can_handle(bounds: IntBounds) -> bool {
    let width = bounds.width();

    width <= Self::MAX_WIDTH
      && i64::from((width + 3) & !3) * i64::from(bounds.height()) <= Self::MAX_STORAGE
  }

  pub(super) fn new(bounds: IntBounds) -> Self {
    Self {
      bounds,
      storage: vec![0; (bounds.width() * bounds.height()) as usize + 2],
    }
  }

  /// `MaskAdditiveBlitter::getRow`: the storage index of pixel `(0, y)`, possibly negative.
  pub(super) const fn row(&self, y: i32) -> isize {
    1 + ((y - self.bounds.top) * self.bounds.width() - self.bounds.left) as isize
  }

  pub(super) fn at(&mut self, row: isize, x: i32) -> &mut u8 {
    &mut self.storage[(row + x as isize) as usize]
  }

  /// `MaskAdditiveBlitter::blitV`, which stores rather than adds.
  fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: u8) {
    if alpha == 0 {
      return;
    }
    for y in y..y + height {
      let row = self.row(y);

      *self.at(row, x) = alpha;
    }
  }

  /// `MaskAdditiveBlitter::blitAntiRect`.
  fn blit_anti_rect(
    &mut self,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    left_alpha: u8,
    right_alpha: u8,
  ) {
    self.blit_v(x, y, height, left_alpha);
    self.blit_v(x + 1 + width, y, height, right_alpha);
    for y in y..y + height {
      let row = self.row(y);

      for offset in 0..width {
        *self.at(row, x + 1 + offset) = 0xFF;
      }
    }
  }

  /// `blitMask(fMask, fClipRect)` from `~MaskAdditiveBlitter`.
  fn finish(&self, coverage: &mut Coverage) {
    let width = self.bounds.width() as usize;

    for (index, row) in self.storage[1..]
      .chunks_exact(width.max(1))
      .take(self.bounds.height() as usize)
      .enumerate()
    {
      let (span, skip) = coverage.span(
        self.bounds.left,
        self.bounds.top + index as i32,
        width as i32,
      );

      span.copy_from_slice(&row[skip..skip + span.len()]);
    }
  }
}

/// How a [`RunBlitter`] sums overlapping alphas.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum AlphaSum {
  /// `RunBasedAdditiveBlitter`'s wrapping sum, for a fill that never exceeds 255.
  Wrap,
  /// `SafeRLEAdditiveBlitter`'s clamped sum.
  Saturate,
}

/// `RunBasedAdditiveBlitter`, or `SafeRLEAdditiveBlitter` under [`AlphaSum::Saturate`].
pub(super) struct RunBlitter {
  left: i32,
  top: i32,
  current_y: i32,
  /// The row being accumulated, `SkAlphaRuns` expanded to one alpha a pixel.
  row: Vec<u8>,
  dirty: (usize, usize),
  sum: AlphaSum,
}

impl RunBlitter {
  pub(super) fn new(bounds: IntBounds, sum: AlphaSum) -> Self {
    Self {
      left: bounds.left,
      top: bounds.top,
      current_y: bounds.top - 1,
      row: vec![0; bounds.width().max(0) as usize],
      dirty: (usize::MAX, 0),
      sum,
    }
  }

  /// `RunBasedAdditiveBlitter::flush`: stores the row's snapped alphas.
  fn flush(&mut self, coverage: &mut Coverage) {
    if self.current_y < self.top {
      return;
    }

    let (start, end) = self.dirty;

    if start < end {
      for alpha in &mut self.row[start..end] {
        *alpha = snap_alpha(*alpha);
      }
      coverage.blit_anti_h(
        self.left + start as i32,
        self.current_y,
        &self.row[start..end],
      );
      self.row[start..end].fill(0);
    }
    self.dirty = (usize::MAX, 0);
    self.current_y = self.top - 1;
  }

  /// `RunBasedAdditiveBlitter::checkY`.
  fn check_y(&mut self, y: i32, coverage: &mut Coverage) {
    if y != self.current_y {
      self.flush(coverage);
      self.current_y = y;
    }
  }

  fn add(&mut self, start: usize, deltas: impl Iterator<Item = u8>) {
    let mut end = start;
    let row = self.row[start..].iter_mut().zip(deltas);

    match self.sum {
      AlphaSum::Wrap => {
        for (alpha, delta) in row {
          add_alpha(alpha, delta);
          end += 1;
        }
      }
      AlphaSum::Saturate => {
        for (alpha, delta) in row {
          safely_add_alpha(alpha, delta);
          end += 1;
        }
      }
    }

    self.dirty = (self.dirty.0.min(start), self.dirty.1.max(end));
  }
}

/// `AdditiveBlitter`: where partial coverage accumulates before it reaches the mask.
pub(super) enum Additive {
  Mask(MaskBlitter),
  Runs(RunBlitter),
}

impl Additive {
  /// `blitAntiH(x, y, alpha)`.
  pub(super) fn blit_anti_h(&mut self, coverage: &mut Coverage, x: i32, y: i32, alpha: u8) {
    self.blit_anti_h_len(coverage, x, y, 1, alpha);
  }

  /// `blitAntiH(x, y, width, alpha)`.
  pub(super) fn blit_anti_h_len(
    &mut self,
    coverage: &mut Coverage,
    x: i32,
    y: i32,
    width: i32,
    alpha: u8,
  ) {
    match self {
      Self::Mask(mask) => {
        let row = mask.row(y);

        for offset in 0..width {
          add_alpha(mask.at(row, x + offset), alpha);
        }
      }
      Self::Runs(runs) => {
        runs.check_y(y, coverage);

        let x = x - runs.left;

        if x >= 0 && x + width <= runs.row.len() as i32 {
          runs.add(x as usize, (0..width).map(|_| alpha));
        }
      }
    }
  }

  /// `blitAntiH(x, y, antialias, len)`, which only the run blitters take.
  pub(super) fn blit_anti_h_alphas(
    &mut self,
    coverage: &mut Coverage,
    x: i32,
    y: i32,
    alphas: &[u8],
  ) {
    let Self::Runs(runs) = self else {
      return;
    };

    runs.check_y(y, coverage);

    let mut x = x - runs.left;
    let mut alphas = alphas;

    if x < 0 {
      alphas = &alphas[(-x as usize).min(alphas.len())..];
      x = 0;
    }

    let len = alphas.len().min(runs.row.len().saturating_sub(x as usize));

    runs.add(x as usize, alphas[..len].iter().copied());
  }

  /// `flush_if_y_changed`.
  pub(super) fn flush_if_y_changed(&mut self, coverage: &mut Coverage, y: i32, next_y: i32) {
    if let Self::Runs(runs) = self
      && (y >> 16) != (next_y >> 16)
    {
      runs.flush(coverage);
    }
  }

  /// `getRealBlitter()->blitV`: the mask blitter stores into its own mask.
  pub(super) fn real_blit_v(
    &mut self,
    coverage: &mut Coverage,
    x: i32,
    y: i32,
    height: i32,
    alpha: u8,
  ) {
    match self {
      Self::Mask(mask) => mask.blit_v(x, y, height, alpha),
      Self::Runs(_) => coverage.blit_v(x, y, height, alpha),
    }
  }

  /// `getRealBlitter()->blitAntiRect`.
  #[allow(clippy::too_many_arguments)]
  pub(super) fn real_blit_anti_rect(
    &mut self,
    coverage: &mut Coverage,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    left_alpha: u8,
    right_alpha: u8,
  ) {
    match self {
      Self::Mask(mask) => mask.blit_anti_rect(x, y, width, height, left_alpha, right_alpha),
      Self::Runs(_) => coverage.blit_anti_rect(x, y, width, height, left_alpha, right_alpha),
    }
  }

  /// Writes what is still accumulated into `coverage`, as the blitters' destructors do.
  pub(super) fn finish(&mut self, coverage: &mut Coverage) {
    match self {
      Self::Mask(mask) => mask.finish(coverage),
      Self::Runs(runs) => runs.flush(coverage),
    }
  }
}
