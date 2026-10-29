use std::{array, cell::RefCell};

use tiny_skia::{PixmapRef, PremultipliedColorU8};

use crate::{
  BackgroundTile, BilinearRows, BoxRows, ColorTile, SampledBitmapView,
  blend::premultiplied_from_pixel,
  canvas::checked_area,
  style::{Affine, ImageScalingAlgorithm},
};

#[derive(Clone, Copy)]
pub(crate) struct SamplingFootprint {
  pub(crate) x: f32,
  pub(crate) y: f32,
}

impl SamplingFootprint {
  pub(crate) fn new(x: f32, y: f32) -> Self {
    Self {
      x: x.max(0.0),
      y: y.max(0.0),
    }
  }

  /// The source pixels one destination pixel covers under `transform`.
  pub(crate) fn of(transform: Affine) -> Self {
    Self::new(
      transform.a.hypot(transform.b),
      transform.c.hypot(transform.d),
    )
  }

  pub(crate) fn is_minifying(self) -> bool {
    self.x > 1.0 || self.y > 1.0
  }

  pub(crate) fn box_span_x(self) -> f32 {
    self.x.max(1.0)
  }

  pub(crate) fn box_span_y(self) -> f32 {
    self.y.max(1.0)
  }
}

#[derive(Clone, Copy)]
pub(crate) enum PaintSource<'a> {
  Pixmap(PixmapRef<'a>),
  BackgroundTile(&'a BackgroundTile),
  ColorTile(&'a ColorTile),
}

impl<'a> PaintSource<'a> {
  pub(crate) fn width(self) -> u32 {
    match self {
      Self::Pixmap(pixmap) => pixmap.width(),
      Self::BackgroundTile(tile) => tile.width(),
      Self::ColorTile(tile) => tile.width(),
    }
  }

  pub(crate) fn height(self) -> u32 {
    match self {
      Self::Pixmap(pixmap) => pixmap.height(),
      Self::BackgroundTile(tile) => tile.height(),
      Self::ColorTile(tile) => tile.height(),
    }
  }

  pub(crate) fn get_pixel(self, x: u32, y: u32) -> PremultipliedColorU8 {
    match self {
      Self::Pixmap(pixmap) => {
        let width = pixmap.width();
        let height = pixmap.height();
        if x >= width || y >= height {
          return PremultipliedColorU8::TRANSPARENT;
        }
        let index = (y * width + x) as usize;
        pixmap.pixels()[index]
      }
      Self::BackgroundTile(tile) => tile.get_pixel(x, y),
      Self::ColorTile(tile) => tile.get_pixel(x, y),
    }
  }

  /// The form pixel loops sample from, resolved once. Loops reading more than one pixel resolve first.
  pub(crate) fn resolve(self) -> ResolvedSource<'a> {
    let Self::BackgroundTile(tile) = self else {
      return ResolvedSource::Direct(self);
    };

    match tile {
      BackgroundTile::Pixmap(source) => {
        ResolvedSource::Direct(Self::Pixmap(source.as_ref().as_ref()))
      }
      _ => match tile.sampled_bitmap_view() {
        Some(view) => match view.identity_source() {
          Some(source) => ResolvedSource::Direct(Self::Pixmap(source)),
          None => ResolvedSource::Bitmap(view),
        },
        None => ResolvedSource::Direct(self),
      },
    }
  }

  /// Rows for destination columns `x_start..x_start + width`, resolved once so
  /// every row fill reads pixels the cheapest way the source allows.
  pub(crate) fn rows(self, x_start: u32, width: u32) -> RowSource<'a> {
    if let Some(color) = self.premultiplied_constant() {
      return RowSource::Constant(color);
    }

    match self.resolve() {
      ResolvedSource::Direct(Self::Pixmap(source)) => RowSource::Copy { source, x_start },
      ResolvedSource::Bitmap(view) => {
        if let Some(rows) = view.bilinear_rows(x_start, width) {
          return RowSource::Bilinear(rows);
        }
        if let Some(rows) = view.box_rows(x_start, width) {
          return RowSource::Box(rows);
        }
        RowSource::Sampled {
          source: ResolvedSource::Bitmap(view),
          x_start,
        }
      }
      source => RowSource::Sampled { source, x_start },
    }
  }

  pub(crate) fn as_pixmap_ref(self) -> Option<PixmapRef<'a>> {
    match self.resolve() {
      ResolvedSource::Direct(Self::Pixmap(source)) => Some(source),
      _ => None,
    }
  }

  pub(crate) fn premultiplied_constant(self) -> Option<[u8; 4]> {
    match self {
      Self::ColorTile(tile) => Some(premultiplied_from_pixel(tile.get_pixel(0, 0))),
      Self::BackgroundTile(BackgroundTile::Color(tile)) => {
        Some(premultiplied_from_pixel(tile.get_pixel(0, 0)))
      }
      _ => None,
    }
  }

  pub(crate) fn with_pixmap_ref<R>(self, f: impl FnOnce(PixmapRef<'_>) -> R) -> Option<R> {
    if let Some(source) = self.as_pixmap_ref() {
      return Some(f(source));
    }

    let width = self.width();
    let height = self.height();
    let source_len = checked_area(width, height, 4)?;
    let mut premultiplied = vec![0; source_len];
    let rows = self.rows(0, width);
    let pixels: &mut [[u8; 4]] = bytemuck::cast_slice_mut(&mut premultiplied);
    for (y, row) in pixels.chunks_exact_mut(width as usize).enumerate() {
      rows.fill(y as u32, row);
    }

    PixmapRef::from_bytes(&premultiplied, width, height).map(f)
  }

  pub(crate) fn supports_rounded_fill_fast_path(self) -> bool {
    matches!(self, Self::Pixmap(_))
  }
}

/// A [`PaintSource`] after [`PaintSource::resolve`].
#[derive(Clone, Copy)]
pub(crate) enum ResolvedSource<'a> {
  Direct(PaintSource<'a>),
  Bitmap(SampledBitmapView<'a>),
}

impl ResolvedSource<'_> {
  pub(crate) fn width(self) -> u32 {
    match self {
      Self::Direct(source) => source.width(),
      Self::Bitmap(view) => view.size().width,
    }
  }

  pub(crate) fn height(self) -> u32 {
    match self {
      Self::Direct(source) => source.height(),
      Self::Bitmap(view) => view.size().height,
    }
  }

  pub(crate) fn get_pixel(self, x: u32, y: u32) -> PremultipliedColorU8 {
    match self {
      Self::Direct(source) => source.get_pixel(x, y),
      Self::Bitmap(view) => view.sample(x, y),
    }
  }
}

/// A paint source read one destination row at a time.
pub(crate) enum RowSource<'a> {
  Copy {
    source: PixmapRef<'a>,
    x_start: u32,
  },
  Bilinear(BilinearRows<'a>),
  Box(BoxRows<'a>),
  Constant([u8; 4]),
  Sampled {
    source: ResolvedSource<'a>,
    x_start: u32,
  },
}

impl RowSource<'_> {
  /// Fills `dst` with source row `y`, one pixel per destination column.
  pub(crate) fn fill(&self, y: u32, dst: &mut [[u8; 4]]) {
    match self {
      Self::Copy { source, x_start } => {
        let start = y as usize * source.width() as usize + *x_start as usize;
        dst.copy_from_slice(bytemuck::cast_slice(
          &source.pixels()[start..start + dst.len()],
        ));
      }
      Self::Bilinear(rows) => rows.fill(y, dst),
      Self::Box(rows) => rows.fill(y, dst),
      Self::Constant(color) => dst.fill(*color),
      Self::Sampled { source, x_start } => {
        for (i, pixel) in dst.iter_mut().enumerate() {
          *pixel = premultiplied_from_pixel(source.get_pixel(x_start + i as u32, y));
        }
      }
    }
  }
}

impl<'a> From<PixmapRef<'a>> for ResolvedSource<'a> {
  fn from(value: PixmapRef<'a>) -> Self {
    Self::Direct(PaintSource::Pixmap(value))
  }
}

impl<'a> From<PixmapRef<'a>> for PaintSource<'a> {
  fn from(value: PixmapRef<'a>) -> Self {
    Self::Pixmap(value)
  }
}

impl<'a> From<&'a tiny_skia::Pixmap> for PaintSource<'a> {
  fn from(value: &'a tiny_skia::Pixmap) -> Self {
    Self::Pixmap(value.as_ref())
  }
}

impl<'a> From<&'a BackgroundTile> for PaintSource<'a> {
  fn from(value: &'a BackgroundTile) -> Self {
    Self::BackgroundTile(value)
  }
}

impl<'a> From<&'a ColorTile> for PaintSource<'a> {
  fn from(value: &'a ColorTile) -> Self {
    Self::ColorTile(value)
  }
}

#[inline(always)]
pub(super) fn sample_paint_source(
  source: ResolvedSource<'_>,
  algorithm: ImageScalingAlgorithm,
  x: f32,
  y: f32,
  footprint: SamplingFootprint,
) -> Option<[u8; 4]> {
  interpolate_with_footprint(source, algorithm, x, y, footprint).map(premultiplied_from_pixel)
}

/// One axis of a bilinear lookup, casts included, so every sampler matches byte for byte.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct BilinearAxis {
  floor: usize,
  ceil: usize,
  ratio: u32,
}

/// The two source rows a vertical axis reads.
#[derive(Clone, Copy)]
pub(crate) struct BilinearRow<'p> {
  axis: BilinearAxis,
  top: &'p [PremultipliedColorU8],
  bottom: &'p [PremultipliedColorU8],
}

/// Bilinear magnification one row at a time, each source row lerped once, matching
/// [`BilinearAxis::mix`].
pub(crate) struct BilinearColumns {
  columns: Vec<BilinearAxis>,
  lerped: RefCell<LerpedRows>,
}

/// The last two source rows lerped across the columns, each by its row index.
#[derive(Default)]
struct LerpedRows([(Option<usize>, Vec<[u16; 4]>); 2]);

impl BilinearColumns {
  pub(crate) fn new(columns: Vec<BilinearAxis>) -> Self {
    Self {
      columns,
      lerped: RefCell::default(),
    }
  }

  /// [`BilinearAxis::mix`] of each column at source rows `row` into `out`.
  pub(crate) fn fill(&self, source: PixmapRef<'_>, row: BilinearAxis, out: &mut [[u8; 4]]) {
    let mut lerped = self.lerped.borrow_mut();
    let top = lerped.slot(source, &self.columns, row.floor, row.ceil);
    let bottom = lerped.slot(source, &self.columns, row.ceil, row.floor);
    let [top, bottom] = [top, bottom].map(|slot| &lerped.0[slot].1);
    let weight = row.ratio;

    // A row's lerp peaks at 255 * 256, inside 16 bits; the column lerp needs 32.
    for ((out, top), bottom) in out.iter_mut().zip(top).zip(bottom) {
      *out = array::from_fn(|channel| {
        ((u32::from(top[channel]) * (256 - weight) + u32::from(bottom[channel]) * weight) >> 16)
          as u8
      });
    }
  }
}

impl LerpedRows {
  /// The slot holding source row `index` lerped across `columns`, lerping it into the slot that
  /// does not hold row `keep` when neither has it.
  fn slot(
    &mut self,
    source: PixmapRef<'_>,
    columns: &[BilinearAxis],
    index: usize,
    keep: usize,
  ) -> usize {
    if let Some(slot) = self.0.iter().position(|(row, _)| *row == Some(index)) {
      return slot;
    }

    let slot = usize::from(self.0[0].0 == Some(keep) && index != keep);
    let stride = source.width() as usize;
    let pixels = &source.pixels()[index * stride..][..stride];
    let (row, lerped) = &mut self.0[slot];

    *row = Some(index);
    lerped.clear();
    lerped.extend(columns.iter().map(|column| {
      let left = premultiplied_from_pixel(pixels[column.floor]);
      let right = premultiplied_from_pixel(pixels[column.ceil]);
      let ratio = column.ratio as u16;

      array::from_fn(|channel| {
        u16::from(left[channel]) * (256 - ratio) + u16::from(right[channel]) * ratio
      })
    }));

    slot
  }
}

impl BilinearAxis {
  pub(crate) fn new(coordinate: f32, extent: u32) -> Self {
    let last = extent.saturating_sub(1);
    let clamped = (coordinate - 0.5).clamp(0.0, last as f32);
    let floor = clamped.floor() as u32;

    Self {
      floor: floor as usize,
      ceil: (floor + 1).min(last) as usize,
      ratio: ((clamped - floor as f32) * 256.0) as u32,
    }
  }

  pub(crate) fn rows(self, source: PixmapRef<'_>) -> BilinearRow<'_> {
    let stride = source.width() as usize;
    let pixels = source.pixels();

    BilinearRow {
      axis: self,
      top: &pixels[self.floor * stride..][..stride],
      bottom: &pixels[self.ceil * stride..][..stride],
    }
  }

  /// 8.8 fixed-point weights for the taps `[floor, ceil] x [top, bottom]`.
  #[inline(always)]
  fn weights(self, row: Self) -> [u32; 4] {
    let u_opposite = 256 - self.ratio;
    let v_opposite = 256 - row.ratio;

    [
      u_opposite * v_opposite,
      self.ratio * v_opposite,
      u_opposite * row.ratio,
      self.ratio * row.ratio,
    ]
  }

  /// Legal premultiplied taps average to a legal premultiplied colour; measured 4% faster
  /// on a full-canvas image when force-inlined.
  #[inline(always)]
  pub(crate) fn mix(self, row: BilinearRow<'_>) -> [u8; 4] {
    mix_taps(
      [
        row.top[self.floor],
        row.top[self.ceil],
        row.bottom[self.floor],
        row.bottom[self.ceil],
      ],
      self.weights(row.axis),
    )
  }
}

#[inline(always)]
fn mix_taps(taps: [PremultipliedColorU8; 4], weights: [u32; 4]) -> [u8; 4] {
  let channel = |channel: fn(PremultipliedColorU8) -> u8| {
    let sum: u32 = taps
      .iter()
      .zip(weights)
      .map(|(tap, weight)| channel(*tap) as u32 * weight)
      .sum();

    (sum >> 16) as u8
  };

  [
    channel(PremultipliedColorU8::red),
    channel(PremultipliedColorU8::green),
    channel(PremultipliedColorU8::blue),
    channel(PremultipliedColorU8::alpha),
  ]
}

/// An axis-aligned, non-minifying scale: column taps derived once, row taps once per row.
pub(crate) struct ScaledRows<'a> {
  source: PixmapRef<'a>,
  transform: Affine,
  x_start: f32,
  columns: ScaledColumns,
}

/// What each destination column of a [`ScaledRows`] reads: two bilinear taps when the draw
/// magnifies, a box span when it minifies.
enum ScaledColumns {
  /// A whole-pixel shift, whose taps all land on pixel centres: column `i` reads source column
  /// `start + i`.
  Shift {
    start: usize,
  },
  Bilinear(BilinearColumns),
  Box {
    columns: Vec<BoxAxis>,
    span_y: f32,
  },
}

impl<'a> ScaledRows<'a> {
  /// `x_start` and the `y` given to `fill` are in the transform's input space.
  pub(crate) fn new(
    source: ResolvedSource<'a>,
    transform: Affine,
    algorithm: ImageScalingAlgorithm,
    x_start: f32,
    width: usize,
  ) -> Option<Self> {
    let ResolvedSource::Direct(PaintSource::Pixmap(source)) = source else {
      return None;
    };

    if transform.b != 0.0
      || transform.c != 0.0
      || matches!(algorithm, ImageScalingAlgorithm::Pixelated)
    {
      return None;
    }

    let footprint = SamplingFootprint::of(transform);
    let (mut sample_x, _) = transform.transform_point(x_start, 0.0);
    let start = sample_x - 0.5;

    if transform.a == 1.0
      && transform.d == 1.0
      && start.fract() == 0.0
      && start >= 0.0
      && start as usize + width <= source.width() as usize
    {
      return Some(Self {
        source,
        transform,
        x_start,
        columns: ScaledColumns::Shift {
          start: start as usize,
        },
      });
    }

    let mut advance = || {
      let x = sample_x;
      sample_x += transform.a;
      x
    };
    let columns = if footprint.is_minifying() {
      ScaledColumns::Box {
        columns: (0..width)
          .map(|_| BoxAxis::new(advance(), footprint.box_span_x(), source.width()))
          .collect(),
        span_y: footprint.box_span_y(),
      }
    } else {
      ScaledColumns::Bilinear(BilinearColumns::new(
        (0..width)
          .map(|_| BilinearAxis::new(advance(), source.width()))
          .collect(),
      ))
    };

    Some(Self {
      source,
      transform,
      x_start,
      columns,
    })
  }

  pub(crate) fn fill(&self, y: f32, out: &mut [[u8; 4]]) {
    let (_, sample_y) = self.transform.transform_point(self.x_start, y);

    match &self.columns {
      ScaledColumns::Shift { start } => {
        let row = BilinearAxis::new(sample_y, self.source.height());
        let stride = self.source.width() as usize;
        let source: &[[u8; 4]] = bytemuck::cast_slice(self.source.pixels());

        if row.ratio == 0 {
          out.copy_from_slice(&source[row.floor * stride + start..][..out.len()]);
          return;
        }

        let rows = row.rows(self.source);

        for (index, dst) in out.iter_mut().enumerate() {
          *dst = BilinearAxis::new((start + index) as f32 + 0.5, stride as u32).mix(rows);
        }
      }
      ScaledColumns::Bilinear(columns) => {
        columns.fill(
          self.source,
          BilinearAxis::new(sample_y, self.source.height()),
          out,
        );
      }
      ScaledColumns::Box { columns, span_y } => {
        BoxAxis::new(sample_y, *span_y, self.source.height()).fill_row(self.source, columns, out);
      }
    }
  }
}

pub(crate) fn interpolate_with_footprint(
  image: ResolvedSource<'_>,
  algorithm: ImageScalingAlgorithm,
  x: f32,
  y: f32,
  footprint: SamplingFootprint,
) -> Option<PremultipliedColorU8> {
  if matches!(algorithm, ImageScalingAlgorithm::Pixelated) {
    return interpolate_nearest(image, x, y);
  }

  if footprint.is_minifying() {
    return interpolate_box(image, x, y, footprint);
  }

  interpolate_bilinear(image, x, y)
}

#[inline(always)]
fn interpolate_nearest(image: ResolvedSource<'_>, x: f32, y: f32) -> Option<PremultipliedColorU8> {
  let w = image.width();
  let h = image.height();
  if w == 0 || h == 0 {
    return None;
  }

  let px = x.floor().max(0.0) as u32;
  let px = px.min(w.saturating_sub(1));
  let py = y.floor().max(0.0) as u32;
  let py = py.min(h.saturating_sub(1));

  Some(image.get_pixel(px, py))
}

#[inline(always)]
fn interpolate_bilinear(image: ResolvedSource<'_>, x: f32, y: f32) -> Option<PremultipliedColorU8> {
  let w = image.width();
  let h = image.height();

  if w == 0 || h == 0 {
    return None;
  }

  let column = BilinearAxis::new(x, w);
  let row = BilinearAxis::new(y, h);
  let tap = |x: usize, y: usize| image.get_pixel(x as u32, y as u32);
  let p00 = tap(column.floor, row.floor);

  if (column.floor == column.ceil && row.floor == row.ceil) || (column.ratio == 0 && row.ratio == 0)
  {
    return Some(p00);
  }

  let [r, g, b, a] = mix_taps(
    [
      p00,
      tap(column.ceil, row.floor),
      tap(column.floor, row.ceil),
      tap(column.ceil, row.ceil),
    ],
    column.weights(row),
  );

  PremultipliedColorU8::from_rgba(r, g, b, a)
}

fn interpolate_box(
  image: ResolvedSource<'_>,
  x: f32,
  y: f32,
  footprint: SamplingFootprint,
) -> Option<PremultipliedColorU8> {
  let width = image.width();
  let height = image.height();
  if width == 0 || height == 0 {
    return None;
  }

  box_average(
    BoxAxis::new(x, footprint.box_span_x(), width),
    BoxAxis::new(y, footprint.box_span_y(), height),
    |source_x, source_y| image.get_pixel(source_x, source_y),
  )
}

/// One axis of a box filter: the source span a destination pixel covers, casts included, so
/// every sampler matches byte for byte.
#[derive(Clone, Copy)]
pub(crate) struct BoxAxis {
  low: f32,
  high: f32,
  start: u32,
  end: u32,
}

impl BoxAxis {
  pub(crate) fn new(coordinate: f32, span: f32, extent: u32) -> Self {
    let size = extent as f32;
    let center = if span >= size {
      size * 0.5
    } else {
      coordinate.clamp(span * 0.5, size - span * 0.5)
    };
    let low = (center - span * 0.5).clamp(0.0, size);
    let high = (center + span * 0.5).clamp(0.0, size);

    Self {
      low,
      high,
      start: low.floor() as u32,
      end: high.ceil().min(size) as u32,
    }
  }

  /// How much of source pixel `index` the span covers.
  #[inline(always)]
  fn weight(self, index: u32) -> f32 {
    let edge = index as f32;

    (self.high.min(edge + 1.0) - self.low.max(edge)).max(0.0)
  }

  /// Box-averages `source` across this row span under each of `columns`, into `out`.
  pub(crate) fn fill_row(self, source: PixmapRef<'_>, columns: &[BoxAxis], out: &mut [[u8; 4]]) {
    let stride = source.width() as usize;
    let pixels = source.pixels();

    for (dst, &column) in out.iter_mut().zip(columns) {
      let average = box_average(column, self, |x, y| {
        pixels[y as usize * stride + x as usize]
      });

      *dst = premultiplied_from_pixel(average.unwrap_or(PremultipliedColorU8::TRANSPARENT));
    }
  }
}

/// The box average of the source pixels `columns` and `rows` cover, read through `pixel`.
#[inline(always)]
fn box_average(
  columns: BoxAxis,
  rows: BoxAxis,
  pixel: impl Fn(u32, u32) -> PremultipliedColorU8,
) -> Option<PremultipliedColorU8> {
  let mut sum = [0.0; 4];
  let mut total_weight = 0.0;

  for source_y in rows.start..rows.end {
    let y_weight = rows.weight(source_y);
    if y_weight == 0.0 {
      continue;
    }

    for source_x in columns.start..columns.end {
      let x_weight = columns.weight(source_x);
      if x_weight == 0.0 {
        continue;
      }

      let weight = x_weight * y_weight;
      let pixel = pixel(source_x, source_y);
      sum[0] += pixel.red() as f32 * weight;
      sum[1] += pixel.green() as f32 * weight;
      sum[2] += pixel.blue() as f32 * weight;
      sum[3] += pixel.alpha() as f32 * weight;
      total_weight += weight;
    }
  }

  if total_weight == 0.0 {
    return None;
  }

  PremultipliedColorU8::from_rgba(
    (sum[0] / total_weight).round() as u8,
    (sum[1] / total_weight).round() as u8,
    (sum[2] / total_weight).round() as u8,
    (sum[3] / total_weight).round() as u8,
  )
}

#[cfg(test)]
mod tests {
  use tiny_skia::{Pixmap, PremultipliedColorU8};

  use super::{
    BilinearAxis, BilinearColumns, SamplingFootprint, ScaledColumns, ScaledRows,
    interpolate_bilinear, interpolate_with_footprint, sample_paint_source,
  };
  use crate::style::{Affine, ImageScalingAlgorithm};

  #[test]
  fn a_filled_row_mixes_each_column_alone() {
    let pixmap = noisy_pixmap();
    // Rows that repeat, step within one source row, skip rows, go back, and reach the last row,
    // where both taps read the same source row.
    let sample_ys = [
      0.0, 0.2, 0.7, 0.9, 1.6, 5.3, 5.4, 2.1, 30.5, 60.2, 61.0, 60.9, 13.37,
    ];

    for (case, width) in [1, 7, 8, 9, 23, 97].into_iter().enumerate() {
      let step = 97.0 / width as f32 * 0.83;
      let columns: Vec<BilinearAxis> = (0..width)
        .map(|index| BilinearAxis::new(index as f32 * step + 0.1 * case as f32, 97))
        .collect();
      let filler = BilinearColumns::new(columns.clone());

      for sample_y in sample_ys {
        let row = BilinearAxis::new(sample_y + case as f32 * 0.3, 61);
        let mut filled = vec![[0; 4]; width];

        filler.fill(pixmap.as_ref(), row, &mut filled);
        for (column, filled) in columns.iter().zip(&filled) {
          assert_eq!(
            *filled,
            column.mix(row.rows(pixmap.as_ref())),
            "width {width} at y {sample_y}"
          );
        }
      }
    }
  }

  /// A 97x61 pixmap of legal premultiplied noise.
  fn noisy_pixmap() -> Pixmap {
    let mut pixmap = Pixmap::new(97, 61).unwrap();

    for (index, pixel) in pixmap.pixels_mut().iter_mut().enumerate() {
      let byte = |shift: u32| ((index as u32).wrapping_mul(2_654_435_761) >> shift) as u8;
      let alpha = byte(24);

      *pixel = PremultipliedColorU8::from_rgba(
        byte(3) % (alpha.saturating_add(1)),
        byte(9) % (alpha.saturating_add(1)),
        byte(15) % (alpha.saturating_add(1)),
        alpha,
      )
      .unwrap();
    }
    pixmap
  }

  #[test]
  fn shifted_rows_match_the_per_pixel_sampler() {
    let pixmap = noisy_pixmap();
    let source = pixmap.as_ref().into();
    let width = 25;

    // Row offsets off the pixel centre and rows past either edge still read through bilinear taps.
    for (shift_x, shift_y, row_offset) in [
      (7.0, 3.0, 0.5),
      (7.0, 3.0, 0.75),
      (0.0, -2.0, 0.5),
      (72.0, 50.0, 0.5),
    ] {
      let transform = Affine::translation(shift_x, shift_y);
      let rows =
        ScaledRows::new(source, transform, ImageScalingAlgorithm::Auto, 0.5, width).unwrap();
      let mut row = vec![[0; 4]; width];

      assert!(matches!(rows.columns, ScaledColumns::Shift { .. }));
      for y in 0..20 {
        let y = y as f32 + row_offset;
        let (mut sample_x, sample_y) = transform.transform_point(0.5, y);

        rows.fill(y, &mut row);
        for &pixel in &row {
          let expected = sample_paint_source(
            source,
            ImageScalingAlgorithm::Auto,
            sample_x,
            sample_y,
            SamplingFootprint::of(transform),
          )
          .unwrap_or([0; 4]);

          assert_eq!(pixel, expected, "shift {shift_x},{shift_y} at row {y}");
          sample_x += 1.0;
        }
      }
    }
  }

  #[test]
  fn scaled_rows_match_the_per_pixel_sampler() {
    let pixmap = noisy_pixmap();

    for (scale_x, scale_y) in [(3.1, 2.4), (2.7, 0.6), (0.8, 1.9)] {
      let transform = Affine::translation(1.25, -0.5) * Affine::scale(scale_x, scale_y);
      let footprint = SamplingFootprint::of(transform);
      let width = 25;
      let source = pixmap.as_ref().into();
      let rows =
        ScaledRows::new(source, transform, ImageScalingAlgorithm::Auto, 0.5, width).unwrap();
      let mut row = vec![[0; 4]; width];

      for y in 0..20 {
        let (mut sample_x, sample_y) = transform.transform_point(0.5, y as f32 + 0.5);

        rows.fill(y as f32 + 0.5, &mut row);
        for &pixel in &row {
          let expected = sample_paint_source(
            source,
            ImageScalingAlgorithm::Auto,
            sample_x,
            sample_y,
            footprint,
          )
          .unwrap_or([0; 4]);

          assert_eq!(pixel, expected, "scale {scale_x}x{scale_y} row {y}");
          sample_x += transform.a;
        }
      }
    }
  }

  /// Sampling takes pixel-centre coordinates: a draw that lands whole-pixel
  /// must read a texel exactly, not blend two of them.
  #[test]
  fn sampling_a_pixel_centre_reads_that_pixel() -> Result<(), &'static str> {
    let mut pixmap = Pixmap::new(2, 1).ok_or("failed to create pixmap")?;
    let pixels = pixmap.pixels_mut();
    pixels[0] = PremultipliedColorU8::from_rgba(0, 0, 0, 255).ok_or("black")?;
    pixels[1] = PremultipliedColorU8::from_rgba(255, 255, 255, 255).ok_or("white")?;

    let centre = interpolate_bilinear(pixmap.as_ref().into(), 0.5, 0.5).ok_or("sampled")?;
    assert_eq!(centre.red(), 0, "a pixel centre reads the pixel itself");

    let corner = interpolate_bilinear(pixmap.as_ref().into(), 0.0, 0.5).ok_or("sampled")?;
    assert_eq!(corner.red(), 0, "clamped at the left edge");

    let between = interpolate_bilinear(pixmap.as_ref().into(), 1.0, 0.5).ok_or("sampled")?;
    assert!(
      (between.red() as i16 - 128).abs() <= 1,
      "halfway between two pixels blends them"
    );

    Ok(())
  }

  #[test]
  fn minified_sampling_averages_high_frequency_content() -> Result<(), &'static str> {
    let mut pixmap = Pixmap::new(8, 1).ok_or("failed to create pixmap")?;
    for (index, pixel) in pixmap.pixels_mut().iter_mut().enumerate() {
      let value = if index % 2 == 0 { 0 } else { 255 };
      *pixel = PremultipliedColorU8::from_rgba(value, value, value, 255)
        .ok_or("failed to create opaque grayscale pixel")?;
    }

    let sample = interpolate_with_footprint(
      pixmap.as_ref().into(),
      ImageScalingAlgorithm::Auto,
      4.0,
      0.5,
      SamplingFootprint::new(8.0, 1.0),
    )
    .ok_or("failed to sample minified image")?;

    assert!((sample.red() as i16 - 128).abs() <= 1);
    assert!((sample.green() as i16 - 128).abs() <= 1);
    assert!((sample.blue() as i16 - 128).abs() <= 1);
    assert_eq!(sample.alpha(), 255);
    Ok(())
  }
}
