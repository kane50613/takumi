//! The seam between deciding what to paint and painting it.

mod background;
mod border;
mod content;
mod decoration;
mod outline;
mod replaced;
mod shadow;
mod text;

pub use self::{
  background::{BackgroundClipArea, BoxBackground},
  border::BoxBorderPainter,
  content::OwnContent,
  outline::PendingOutline,
  replaced::ReplacedContent,
  shadow::ShadowShape,
  text::{GlyphDevice, GlyphFill, InlineLines},
};

use crate::{
  context::RenderContext,
  geometry::{ComputedLayout, PathCommand, Point, Rect, Size},
  layout::{
    border::{BorderDash, BorderProperties},
    clip::push_ellipse,
    decoration::{ClipBox, OutlineGeometry},
  },
  shadow::SizedShadow,
  style::{Affine, BackgroundImage, BoxShadow, Color, FillRule, Overflow, SpacePair},
};

/// A distance far enough out that an edge placed there never shows, for a clip that is unbounded on
/// some side.
pub const UNBOUNDED: f32 = 1.0e6;

/// A closed shape to fill, in the coordinate space of the box that owns it.
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
      Self::Path { commands: path, .. } => commands.extend_from_slice(path),
    }
    commands
  }

  /// How to decide what lies inside the shape.
  pub fn rule(&self) -> FillRule {
    match self {
      Self::Path { rule, .. } => *rule,
      _ => FillRule::NonZero,
    }
  }

  /// The ring between the outer and inner edges of `border` on a `size` box.
  pub fn border_ring(border: &BorderProperties, size: Size<f32>) -> Self {
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

  /// The padding box's edges on each axis that clips, effectively unbounded on the others.
  pub fn overflow_clip_edges(self, clip_x: bool, clip_y: bool) -> Rect<f32> {
    let Self {
      layout,
      origin: Point { x, y },
    } = self;
    let (left, right) = if clip_x {
      let padding_left = x + layout.border.left;
      let padding_right = (x + layout.size.width - layout.border.right).max(padding_left);
      (padding_left, padding_right)
    } else {
      (x - UNBOUNDED, x + layout.size.width + UNBOUNDED)
    };
    let (top, bottom) = if clip_y {
      let padding_top = y + layout.border.top;
      let padding_bottom = (y + layout.size.height - layout.border.bottom).max(padding_top);
      (padding_top, padding_bottom)
    } else {
      (y - UNBOUNDED, y + layout.size.height + UNBOUNDED)
    };

    Rect {
      left,
      top,
      right,
      bottom,
    }
  }
}

/// What a box's `overflow` clips its content to.
pub enum OverflowClip {
  /// The rounded padding box. A corner radius clips both axes, whatever each axis asks for.
  Rounded(ClipBox),
  /// The padding box on each axis that clips, the other axis left unbounded.
  Axes {
    /// Whether the horizontal axis clips.
    x: bool,
    /// Whether the vertical axis clips.
    y: bool,
  },
}

impl OverflowClip {
  /// What the box at `layout` clips its content to, or `None` when it clips nothing.
  pub fn of(context: &RenderContext, layout: ComputedLayout) -> Option<Self> {
    let overflow = context.style.resolve_overflows();

    if !overflow.should_clip_content() {
      return None;
    }

    let border = BorderProperties::from_context(context, layout.size, layout.border);

    if !border.is_zero() {
      return Some(Self::Rounded(ClipBox::padding_box(border, layout)));
    }

    Some(Self::Axes {
      x: overflow.x != Overflow::Visible,
      y: overflow.y != Overflow::Visible,
    })
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

  /// Fills `shape` under `transform`, with a single colour.
  fn fill_shape(&mut self, shape: &FillShape, color: Color, transform: Affine);

  /// Strokes `shape` under `transform`.
  fn stroke_shape(&mut self, _shape: &FillShape, _stroke: &StrokeStyle, _transform: Affine) {}

  /// Clips later draws to `shape` under `transform`, until the matching [`PaintDevice::pop_clip`].
  fn push_clip(&mut self, shape: &FillShape, transform: Affine);

  /// Clips later draws to everything outside `shape` under `transform`, until the matching
  /// [`PaintDevice::pop_clip`].
  fn push_clip_out(&mut self, shape: &FillShape, transform: Affine);

  /// Removes the most recent clip.
  fn pop_clip(&mut self);

  /// Draws what follows into a layer that composites at `opacity` on the matching
  /// [`PaintDevice::end_layer`].
  fn begin_layer(&mut self, opacity: f32);

  /// Composites the most recent layer.
  fn end_layer(&mut self);

  /// Runs `paint` into a layer at `opacity`, without the layer when the paint is opaque and not
  /// at all when it is invisible.
  fn with_opacity(&mut self, opacity: f32, paint: impl FnOnce(&mut Self))
  where
    Self: Sized,
  {
    if opacity <= 0.0 {
      return;
    }
    if opacity >= 1.0 {
      return paint(self);
    }

    self.begin_layer(opacity);
    paint(self);
    self.end_layer();
  }

  /// Fills `shape` moved by `shadow`'s offset, in its colour, blurred by a Gaussian whose standard
  /// deviation is half its blur radius, as a CSS shadow blurs.
  fn fill_shadow(&mut self, shape: &ShadowShape, shadow: &SizedShadow, transform: Affine);
}

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
}

impl<'c> BoxPainter<'c> {
  /// Prepares the box at `layout` for painting.
  pub fn new(context: &'c RenderContext, layout: ComputedLayout) -> Self {
    Self {
      context,
      layout,
      border: BorderProperties::from_context(context, layout.size, layout.border),
    }
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

  /// The area the box clips its background to, per `background-clip`.
  pub fn background_clip(&self) -> BackgroundClipArea {
    BackgroundClipArea::new(self.context, self.layout, self.border)
  }

  /// The box's background, resolved.
  pub fn background(&self) -> BoxBackground<'c> {
    BoxBackground::new(self.context, self.layout, self.border)
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
    let Some(shape) = self.background_clip().shape(self.layout.size) else {
      return;
    };

    device.set_role(PaintRole::Background);
    device.fill_shape(&shape, color, Affine::translation(origin.x, origin.y));
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
  pub fn paint_border<D: PaintDevice>(&self, origin: Point<f32>, device: &mut D) {
    device.set_role(PaintRole::Border);
    BoxBorderPainter::new(&self.border, self.layout.size).paint(origin, device);
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
    OutlineGeometry::painted(self.context, self.layout.size)
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

    (background && self.background_clip().shape(self.layout.size).is_some())
      || self.border.has_visible_sides()
      || !shadows.inset.is_empty()
      || !shadows.outer.is_empty()
      || (style.outline_color.resolve(current_color).0[3] != 0 && self.outline().is_some())
  }
}
