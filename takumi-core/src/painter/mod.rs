//! The seam between deciding what to paint and painting it.

mod background;
mod border;
mod box_side;
mod content;
mod decoration;
mod outline;
mod replaced;
mod shadow;
mod snapped_box;
mod text;
mod text_clip;

pub use self::{
  background::{BackgroundClipArea, BoxBackground},
  border::BoxBorderPainter,
  content::OwnContent,
  outline::PendingOutline,
  replaced::ReplacedContent,
  shadow::ShadowShape,
  snapped_box::SnappedBox,
  text::{GlyphDevice, GlyphFill, InlineLines, StripBackground},
  text_clip::TextClip,
};

use crate::{
  context::RenderContext,
  geometry::{ComputedLayout, PathCommand, Point, Rect, Size},
  layout::{
    border::{BorderDash, BorderProperties},
    clip::push_ellipse,
    decoration::{ClipBox, ContourOrigin, OutlineGeometry},
  },
  shadow::SizedShadow,
  style::{Affine, BackgroundImage, BoxShadow, Color, FillRule, Overflow, SpacePair},
};

/// How far a layer's paint can reach: a rectangle of `size` at the origin under `transform`, as
/// Skia's `saveLayer` bounds.
#[derive(Debug, Clone, Copy)]
pub struct LayerBounds {
  /// The rectangle's size.
  pub size: Size<f32>,
  /// Where the rectangle sits.
  pub transform: Affine,
}

/// A distance far enough out that an edge placed there never shows, for a clip that is unbounded on
/// some side.
pub const UNBOUNDED: f32 = 1.0e6;

/// A closed shape to fill, in the coordinate space of the box that owns it.
#[derive(Debug, Clone)]
pub enum FillShape {
  /// An axis-aligned rectangle at the box origin.
  Rect(Size<f32>),
  /// A rectangle whose corners come from `border`.
  RoundedRect {
    /// The corner geometry.
    border: BorderProperties,
    /// The rectangle's size.
    size: Size<f32>,
    /// Where the rectangle sits inside the box.
    offset: Point<f32>,
  },
  /// An axis-aligned ellipse.
  Ellipse {
    /// The centre.
    center: Point<f32>,
    /// The horizontal and vertical radii.
    radius: SpacePair<f32>,
  },
  /// A region inset from a border box whose corners are not all round, its corners following the
  /// border box's.
  Contoured(ClipBox),
  /// Anything else.
  Path {
    /// The path.
    commands: Vec<PathCommand>,
    /// How to decide what lies inside the path.
    rule: FillRule,
  },
}

impl FillShape {
  /// The shape as path commands, for a backend that only draws paths.
  pub fn to_commands(&self) -> Vec<PathCommand> {
    let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);

    match self {
      Self::Rect(size) => {
        BorderProperties::default().append_mask_commands(&mut commands, *size, Point::ZERO);
      }
      Self::RoundedRect {
        border,
        size,
        offset,
      } => border.append_mask_commands(&mut commands, *size, *offset),
      Self::Ellipse { center, radius } => push_ellipse(&mut commands, *center, *radius),
      Self::Contoured(clip) => clip.append_contour(&mut commands),
      Self::Path { commands: path, .. } => commands.extend_from_slice(path),
    }
    commands
  }

  /// How to decide what lies inside the shape.
  pub fn rule(&self) -> FillRule {
    match self {
      Self::Path { rule, .. } => *rule,
      Self::Contoured(_) => FillRule::EvenOdd,
      _ => FillRule::NonZero,
    }
  }

  /// Closed polygons of `N` corners each, filled nonzero.
  pub(crate) fn polygons<const N: usize>(
    polygons: impl IntoIterator<Item = [Point<f32>; N]>,
  ) -> Self {
    let commands = polygons
      .into_iter()
      .flat_map(|corners| {
        corners
          .into_iter()
          .enumerate()
          .map(|(index, corner)| {
            if index == 0 {
              PathCommand::MoveTo(corner)
            } else {
              PathCommand::LineTo(corner)
            }
          })
          .chain([PathCommand::Close])
      })
      .collect();

    Self::Path {
      commands,
      rule: FillRule::NonZero,
    }
  }

  /// The ring between the outer and inner edges of `border` on a `size` box.
  pub(crate) fn border_ring(border: &BorderProperties, size: Size<f32>) -> Self {
    let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);

    border.append_border_ring_commands(&mut commands, size);
    Self::Path {
      commands,
      rule: FillRule::EvenOdd,
    }
  }
}

impl From<ClipBox> for FillShape {
  fn from(clip: ClipBox) -> Self {
    if clip.follows_origin() {
      return Self::Contoured(clip);
    }

    Self::RoundedRect {
      border: clip.border,
      size: clip.size,
      offset: clip.offset,
    }
  }
}

/// A box's layout, its border-box top-left at `origin`.
#[derive(Debug, Clone, Copy)]
pub struct BoxFrame {
  /// The box's layout.
  pub layout: ComputedLayout,
  /// The border-box top-left.
  pub origin: Point<f32>,
}

impl BoxFrame {
  /// Places `layout` at `origin`.
  pub fn new(layout: ComputedLayout, origin: Point<f32>) -> Self {
    Self { layout, origin }
  }

  /// Moves the origin by `offset`.
  pub fn shifted(self, offset: Point<f32>) -> Self {
    Self {
      origin: self.origin + offset,
      ..self
    }
  }

  /// The translation to the origin.
  pub fn translation(self) -> Affine {
    Affine::translation(self.origin.x, self.origin.y)
  }

  /// Moves a border-box-relative transform to the origin's space.
  pub fn place(self, transform: Affine) -> Affine {
    Affine {
      x: transform.x + self.origin.x,
      y: transform.y + self.origin.y,
      ..transform
    }
  }
}

/// What a box's `overflow` clips its content to, pixel-snapped as Blink's `ToSnappedClipRect`
/// and `PixelSnappedContouredInnerBorder` snap an overflow clip.
pub enum OverflowClip {
  /// The rounded padding box. A corner radius clips both axes, whatever each axis asks for.
  Rounded(Box<ClipBox>),
  /// The padding box on each axis that clips, the other axis left unbounded, relative to the
  /// border box.
  Axes(Rect<f32>),
}

impl OverflowClip {
  /// The clip as a shape in the border box, with where the shape's origin sits.
  pub fn shape(self) -> (FillShape, Point<f32>) {
    match self {
      Self::Rounded(clip) => ((*clip).into(), Point::ZERO),
      Self::Axes(edges) => (
        FillShape::Rect(Size {
          width: edges.right - edges.left,
          height: edges.bottom - edges.top,
        }),
        edges.top_left(),
      ),
    }
  }

  /// What the box at `layout`, its border box at `paint_offset`, clips its content to, or `None`
  /// when it clips nothing.
  pub fn of(
    context: &RenderContext,
    layout: ComputedLayout,
    paint_offset: Point<f32>,
  ) -> Option<Self> {
    let overflow = context.style.resolve_overflows();

    if !overflow.should_clip_content() {
      return None;
    }

    let border = BorderProperties::from_context(context, layout.size, layout.border);
    let snapped = SnappedBox::new(paint_offset, layout.size);

    if !border.is_zero() {
      let (offset, size) = snapped.contoured_inset(layout.border, false);
      let padding_box = ClipBox::padding_box(border, layout);

      return Some(Self::Rounded(Box::new(ClipBox {
        offset: offset + snapped.offset(),
        size,
        origin: padding_box.origin.map(|origin| ContourOrigin {
          size: snapped.size(),
          offset: snapped.offset(),
          ..origin
        }),
        ..padding_box
      })));
    }

    let (offset, size) = snapped.inset(layout.border);
    let offset = offset + snapped.offset();
    let (left, right) = if overflow.x == Overflow::Visible {
      (-UNBOUNDED, layout.size.width + UNBOUNDED)
    } else {
      (offset.x, offset.x + size.width)
    };
    let (top, bottom) = if overflow.y == Overflow::Visible {
      (-UNBOUNDED, layout.size.height + UNBOUNDED)
    } else {
      (offset.y, offset.y + size.height)
    };

    Some(Self::Axes(Rect {
      left,
      top,
      right,
      bottom,
    }))
  }
}

/// What a draw paints for its box, so a device that records draws can name them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintRole {
  /// `background-color` and `background-image`.
  Background,
  /// `border`.
  Border,
  /// `box-shadow`.
  BoxShadow,
  /// `outline`.
  Outline,
  /// A replaced element's image.
  Image,
  /// Glyphs.
  Text,
  /// `text-shadow`.
  TextShadow,
  /// `text-decoration` lines.
  TextDecoration,
  /// An inline element's background.
  InlineBackground,
}

/// What a backend has to be able to do for the shared painting code to drive it.
pub trait PaintDevice {
  /// Names what the draws that follow paint. Only a device that records draws needs it.
  fn set_role(&mut self, _role: PaintRole) {}

  /// Maps the space draws are placed in onto the output's pixels.
  fn transform(&self) -> Affine;

  /// Fills `shape` under `transform`, with a single colour.
  fn fill_shape(&mut self, shape: &FillShape, color: Color, transform: Affine);

  /// Strokes `shape` under `transform`.
  fn stroke_shape(&mut self, _shape: &FillShape, _stroke: &StrokeStyle, _transform: Affine) {}

  /// Clips later draws to `shape` under `transform`, until the matching [`PaintDevice::pop_clip`].
  fn push_clip(&mut self, shape: &FillShape, transform: Affine);

  /// Clips later draws to everything outside `shape` under `transform`, until the matching
  /// [`PaintDevice::pop_clip`].
  fn push_clip_out(&mut self, shape: &FillShape, transform: Affine);

  /// Clips later draws to `shape` under `transform` without antialiasing its edges, as a Skia clip
  /// with antialiasing off keeps only the pixels whose centres fall inside, until the matching
  /// [`PaintDevice::pop_clip`].
  fn push_aliased_clip(&mut self, shape: &FillShape, transform: Affine);

  /// Clips later draws to everything outside `shape` under `transform`, keeping only the pixels
  /// whose centres fall outside, until the matching [`PaintDevice::pop_clip`].
  fn push_aliased_clip_out(&mut self, shape: &FillShape, transform: Affine);

  /// Removes the most recent clip.
  fn pop_clip(&mut self);

  /// Paints `content` only where `border` paints on a box of `size` at `origin`, as a `DstIn`
  /// layer keeps it, for `background-clip: border-area`.
  fn with_border_mask(
    &mut self,
    border: &BorderProperties,
    size: Size<f32>,
    origin: Point<f32>,
    content: impl FnOnce(&mut Self),
  ) where
    Self: Sized;

  /// Draws what follows into a layer that composites at `opacity` on the matching
  /// [`PaintDevice::end_layer`], reaching no further than `bounds` when given.
  fn begin_layer(&mut self, opacity: f32, bounds: Option<LayerBounds>);

  /// Composites the most recent layer.
  fn end_layer(&mut self);

  /// Fills `shape` moved by `shadow`'s offset, in its colour, blurred by a Gaussian whose standard
  /// deviation is half its blur radius, as a CSS shadow blurs.
  fn fill_shadow(&mut self, shape: &ShadowShape, shadow: &SizedShadow, transform: Affine);
}

/// Opacity layers for every device, trait objects included.
pub(crate) trait OpacityLayer: PaintDevice {
  /// Runs `paint` into a layer at `opacity` reaching no further than `bounds`, without the layer
  /// when the paint is opaque and not at all when it is invisible.
  fn with_opacity(
    &mut self,
    opacity: f32,
    bounds: Option<LayerBounds>,
    paint: impl FnOnce(&mut Self),
  ) {
    if opacity <= 0.0 {
      return;
    }
    if opacity >= 1.0 {
      return paint(self);
    }

    self.begin_layer(opacity, bounds);
    paint(self);
    self.end_layer();
  }
}

impl<D: PaintDevice + ?Sized> OpacityLayer for D {}

/// How to stroke a shape.
pub struct StrokeStyle {
  /// The stroke colour.
  pub color: Color,
  /// The stroke width.
  pub width: f32,
  /// Dash and gap lengths, when the stroke is dashed or dotted.
  pub dash: Option<[f32; 2]>,
  /// Whether the dashes have round caps, which is how `dotted` draws.
  pub round_cap: bool,
}

impl StrokeStyle {
  /// A border or outline stroke in `color`, dashed as `dash` says.
  pub fn border(color: Color, width: f32, dash: Option<BorderDash>) -> Self {
    Self {
      color,
      width,
      dash: dash.map(|dash| dash.intervals),
      round_cap: dash.is_some_and(|dash| dash.round_cap),
    }
  }
}

/// A box's `box-shadow` layers, split by where they fall.
#[derive(Default, Clone)]
pub struct BoxShadows {
  /// Shadows inside the box.
  pub inset: Vec<SizedShadow>,
  /// Shadows outside it.
  pub outer: Vec<SizedShadow>,
}

/// Everything a backend needs to paint one box, decided once.
pub struct BoxPainter<'c> {
  context: &'c RenderContext,
  layout: ComputedLayout,
  border: BorderProperties,
  /// The border box its decorations paint in.
  snapped: SnappedBox,
}

impl<'c> BoxPainter<'c> {
  /// Prepares the box at `layout` for painting.
  pub fn new(context: &'c RenderContext, layout: ComputedLayout) -> Self {
    Self {
      context,
      layout,
      border: BorderProperties::from_context(context, layout.size, layout.border),
      snapped: SnappedBox::new(context.box_paint_offset(layout), layout.size),
    }
  }

  /// The pixel-snapped border box the box's decorations paint in.
  pub fn snapped(&self) -> &SnappedBox {
    &self.snapped
  }

  /// Prepares a fragment of the box that paints its own decorations, which is what
  /// `box-decoration-break: clone` asks for.
  pub fn fragment(context: &'c RenderContext, layout: ComputedLayout, size: Size<f32>) -> Self {
    Self::new(context, ComputedLayout { size, ..layout })
  }

  /// The context the box paints in.
  pub fn context(&self) -> &'c RenderContext {
    self.context
  }

  /// The box's border geometry, corners included.
  pub fn border(&self) -> &BorderProperties {
    &self.border
  }

  /// The area the box clips its background to, per `background-clip`, relative to the snapped
  /// border box.
  pub fn background_clip(&self) -> BackgroundClipArea {
    BackgroundClipArea::new(self.context, self.layout, self.border, &self.snapped)
  }

  /// The box's background, resolved.
  pub fn background(&self) -> BoxBackground<'c> {
    BoxBackground::new(
      self.context,
      self.layout,
      self.border,
      self.context.box_paint_offset(self.layout),
    )
  }

  /// Paints `background-color`.
  pub fn background_color<D: PaintDevice>(&self, origin: Point<f32>, device: &mut D) {
    let color = self
      .context
      .style
      .background_color
      .resolve(self.context.current_color);

    if color.0[3] == 0 {
      return;
    }
    let clip = self.background_clip();
    let size = self.snapped.size();
    let Some(shape) = clip.shape(size) else {
      return;
    };
    let origin = origin + self.snapped.offset();
    let at = Affine::translation(origin.x, origin.y);

    device.set_role(PaintRole::Background);
    match clip.border_mask() {
      Some(mask) => device.with_border_mask(&mask, size, origin, |device| {
        device.fill_shape(&FillShape::Rect(size), color, at);
      }),
      None => device.fill_shape(&shape, color, at),
    }
  }

  /// The box's `box-shadow` layers, resolved and split into the ones that fall inside the box and
  /// the ones outside it.
  pub fn shadows(&self) -> BoxShadows {
    let Some(shadows) = self.context.style.box_shadow.as_deref() else {
      return BoxShadows::default();
    };
    let resolve = |shadow: &BoxShadow| {
      SizedShadow::from_box_shadow(
        *shadow,
        &self.context.sizing,
        self.context.current_color,
        self.layout.size,
      )
    };
    let visible = |shadow: &SizedShadow| shadow.color.0[3] != 0;

    BoxShadows {
      inset: shadows
        .iter()
        .filter(|shadow| shadow.inset)
        .map(resolve)
        .filter(visible)
        .collect(),
      outer: shadows
        .iter()
        .filter(|shadow| !shadow.inset)
        .map(resolve)
        .filter(visible)
        .collect(),
    }
  }

  /// Paints the box's `border` at `origin`.
  pub fn paint_border(&self, origin: Point<f32>, device: &mut dyn PaintDevice) {
    device.set_role(PaintRole::Border);
    BoxBorderPainter::new(&self.border, self.snapped.size())
      .paint(origin + self.snapped.offset(), device);
  }

  /// The `clip-path` shape the box and its descendants clip to, or `None` when it has none or the
  /// shape cannot resolve.
  pub fn clip_path(&self) -> Option<FillShape> {
    let style = &self.context.style;

    style
      .clip_path
      .as_ref()?
      .fill_shape(self.context, self.layout.size, style.clip_rule)
  }

  /// The outline the box paints, or `None` when it paints none.
  pub fn outline(&self) -> Option<OutlineGeometry> {
    if !self.context.style.is_visible() {
      return None;
    }

    OutlineGeometry::painted(self.context, self.snapped.size())
  }

  /// Whether the box paints a background, border, shadow or outline.
  pub fn paints_decorations(&self) -> bool {
    let style = &self.context.style;
    let current_color = self.context.current_color;
    let background = style.background_color.resolve(current_color).0[3] != 0
      || style
        .background_image
        .as_deref()
        .is_some_and(|images| images.iter().any(BackgroundImage::paints));
    let shadows = self.shadows();

    (background && self.background_clip().shape(self.snapped.size()).is_some())
      || self.border.has_visible_sides()
      || !shadows.inset.is_empty()
      || !shadows.outer.is_empty()
      || (style.outline_color.resolve(current_color).0[3] != 0 && self.outline().is_some())
  }
}
