//! The painted document: nodes in document order, the drawables each paints, and the steps a
//! renderer takes in paint order. Every CSS value is resolved; lengths are device pixels.

use serde::Serialize;

/// `[r, g, b, a]`, each `0..=255`, in sRGB.
pub type Rgba = [u8; 4];

/// `[a, b, c, d, e, f]`, the order `CanvasRenderingContext2D.setTransform` takes.
pub type Matrix = [f32; 6];

/// A point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PaintPoint {
  /// The horizontal coordinate.
  pub x: f32,
  /// The vertical coordinate.
  pub y: f32,
}

/// An axis-aligned rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PaintRect {
  /// The left edge.
  pub x: f32,
  /// The top edge.
  pub y: f32,
  /// The width.
  pub width: f32,
  /// The height.
  pub height: f32,
}

/// Horizontal and vertical radius of each corner.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CornerRadii {
  /// The top-left corner.
  pub top_left: PaintPoint,
  /// The top-right corner.
  pub top_right: PaintPoint,
  /// The bottom-right corner.
  pub bottom_right: PaintPoint,
  /// The bottom-left corner.
  pub bottom_left: PaintPoint,
}

/// A region to fill, stroke, or clip to.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Shape {
  /// A rectangle.
  Rect {
    /// The rectangle.
    rect: PaintRect,
  },
  /// A rectangle with elliptical corners.
  RoundedRect {
    /// The rectangle.
    rect: PaintRect,
    /// The corner radii.
    radii: CornerRadii,
  },
  /// SVG path data, the string `new Path2D()` takes.
  Path {
    /// The path data.
    d: String,
    /// How to decide what lies inside.
    #[serde(rename = "fillRule")]
    fill_rule: FillRuleName,
  },
}

/// A fill rule, named as Canvas and SVG name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FillRuleName {
  /// Nonzero winding.
  Nonzero,
  /// Even-odd.
  Evenodd,
}

/// A colour stop, `offset` in `0..=1`, interpolated in sRGB.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ColorStop {
  /// Where the stop sits.
  pub offset: f32,
  /// The stop's colour.
  pub color: Rgba,
}

/// A decoded image.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImageSource {
  /// The `src` the document referenced, or a `data:` URL for inline image data.
  pub src: String,
  /// The intrinsic width.
  pub width: f32,
  /// The intrinsic height.
  pub height: f32,
}

/// How an image samples its pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Sampling {
  /// Smooth interpolation.
  Smooth,
  /// Nearest-neighbour sampling.
  Pixelated,
}

/// What a shape or glyphs are filled with.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Paint {
  /// A solid colour.
  Color {
    /// The colour.
    color: Rgba,
  },
  /// A linear gradient from `start` to `end`.
  LinearGradient {
    /// Where offset 0 sits.
    start: PaintPoint,
    /// Where offset 1 sits.
    end: PaintPoint,
    /// The stops, unrolled over the area a repeating gradient covers.
    stops: Vec<ColorStop>,
  },
  /// A radial gradient, elliptical when the radii differ.
  #[serde(rename_all = "camelCase")]
  RadialGradient {
    /// The centre.
    center: PaintPoint,
    /// Where offset 1 sits horizontally.
    radius_x: f32,
    /// Where offset 1 sits vertically.
    radius_y: f32,
    /// The stops, unrolled over the area a repeating gradient covers.
    stops: Vec<ColorStop>,
  },
  /// A conic gradient.
  #[serde(rename_all = "camelCase")]
  ConicGradient {
    /// The centre.
    center: PaintPoint,
    /// Where offset 0 sits, in radians clockwise from the positive x axis.
    start_angle: f32,
    /// The stops.
    stops: Vec<ColorStop>,
  },
  /// An image stretched over the shape's bounds.
  Image {
    /// The image.
    image: ImageSource,
    /// How it samples.
    sampling: Sampling,
  },
  /// A tile repeated at every `x` and `y` pair.
  #[serde(rename_all = "camelCase")]
  Pattern {
    /// The tile's paint, in the tile's own space.
    tile: Box<Paint>,
    /// The tile's width.
    tile_width: f32,
    /// The tile's height.
    tile_height: f32,
    /// Each tile's left edge.
    x: Vec<f32>,
    /// Each tile's top edge.
    y: Vec<f32>,
  },
}

/// How to stroke a shape.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stroke {
  /// The line width.
  pub width: f32,
  /// Alternating dash and gap lengths; absent for a solid line.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub dash: Option<Vec<f32>>,
  /// The cap at each end of a dash.
  pub cap: LineCapName,
  /// The join between segments.
  pub join: LineJoinName,
}

/// A line cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LineCapName {
  /// Flat at the end.
  Butt,
  /// Rounded past the end.
  Round,
}

/// A line join.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LineJoinName {
  /// Mitred.
  Miter,
  /// Rounded.
  Round,
  /// Bevelled.
  Bevel,
}

/// What a drawable is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
  /// `background-color` and `background-image`.
  Background,
  /// `border`.
  Border,
  /// `box-shadow`.
  BoxShadow,
  /// `outline`.
  Outline,
  /// A replaced image.
  Image,
  /// Glyphs.
  Text,
  /// `text-shadow`.
  TextShadow,
  /// `-webkit-text-stroke`.
  TextStroke,
  /// `text-decoration` lines.
  TextDecoration,
  /// An inline element's background.
  InlineBackground,
}

/// Which side of a shadow's box it shows on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ShadowSide {
  /// Outside the box, as an outer box shadow shows.
  Outside,
  /// Inside the box, as an inset box shadow shows.
  Inside,
}

/// Something to draw, in the owning node's local space.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Drawable {
  /// A filled shape.
  #[serde(rename_all = "camelCase")]
  Fill {
    /// What it is for.
    role: Role,
    /// The shape.
    shape: Shape,
    /// What fills it.
    paint: Paint,
    /// How it blends with what the enclosing group already holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    blend_mode: Option<String>,
    /// Shapes it is clipped to, all of them at once.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    clips: Vec<Shape>,
  },
  /// A stroked shape.
  Stroke {
    /// What it is for.
    role: Role,
    /// The shape.
    shape: Shape,
    /// How it is stroked.
    stroke: Stroke,
    /// What fills the stroke.
    paint: Paint,
    /// Shapes it is clipped to, all of them at once.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    clips: Vec<Shape>,
  },
  /// A blurred copy of `shape`, moved by `offset`, visible on one side of `box`.
  Shadow {
    /// What it is for.
    role: Role,
    /// The shape that casts it.
    shape: Shape,
    /// Strokes the shape instead of filling it, as a dashed or wavy text decoration casts.
    #[serde(skip_serializing_if = "Option::is_none")]
    stroke: Option<Stroke>,
    /// How far it moves.
    offset: PaintPoint,
    /// The Gaussian's standard deviation.
    blur: f32,
    /// The shadow's colour.
    color: Rgba,
    /// Which side of `box` it shows on.
    visible: ShadowSide,
    /// The shape it shows outside or inside of.
    #[serde(rename = "box")]
    region: Shape,
  },
  /// A run's glyphs filled with `paint`.
  Glyphs {
    /// What it is for.
    role: Role,
    /// The run's index in its text node.
    run: usize,
    /// What fills the glyphs.
    paint: Paint,
    /// How far the glyphs move.
    offset: PaintPoint,
    /// The standard deviation of a text shadow's blur.
    blur: f32,
    /// Strokes the outlines instead of filling them.
    #[serde(skip_serializing_if = "Option::is_none")]
    stroke: Option<Stroke>,
  },
  /// An image drawn into `rect`, clipped to `clip`.
  Image {
    /// What it is for.
    role: Role,
    /// The image.
    image: ImageSource,
    /// Where it draws.
    rect: PaintRect,
    /// What clips it.
    clip: Shape,
    /// How it samples.
    sampling: Sampling,
  },
  /// `content` kept only where `mask` covers, as a `DstIn` layer keeps it.
  Masked {
    /// What it is for.
    role: Role,
    /// Drawables whose alpha masks the content.
    mask: Vec<Drawable>,
    /// The masked drawables.
    content: Vec<Drawable>,
  },
}

impl Drawable {
  /// What the drawable is for.
  pub fn role(&self) -> Role {
    match self {
      Self::Fill { role, .. }
      | Self::Stroke { role, .. }
      | Self::Shadow { role, .. }
      | Self::Glyphs { role, .. }
      | Self::Image { role, .. }
      | Self::Masked { role, .. } => *role,
    }
  }
}

/// A filter a group runs over its layer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum PaintFilter {
  /// A Gaussian blur.
  Blur {
    /// The standard deviation.
    radius: f32,
  },
  /// A 4×5 row-major colour matrix, as SVG `feColorMatrix type="matrix"` takes.
  ColorMatrix {
    /// The matrix.
    matrix: Vec<f32>,
  },
  /// A drop shadow.
  DropShadow {
    /// How far the shadow moves.
    offset: PaintPoint,
    /// The Gaussian's standard deviation.
    blur: f32,
    /// The shadow's colour.
    color: Rgba,
  },
  /// A filter takumi cannot resolve, such as `url(#svg-filter)`.
  Unsupported {
    /// The filter as written.
    css: String,
  },
}

/// How a node composites.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Effects {
  /// The group's opacity.
  pub opacity: f32,
  /// How the group blends with its backdrop.
  pub blend_mode: String,
  /// Whether the group isolates its descendants' blending.
  pub isolation: bool,
  /// Filters run over the group's layer.
  pub filters: Vec<PaintFilter>,
  /// Filters run over what is already drawn behind the node, before it draws.
  pub backdrop_filters: Vec<PaintFilter>,
  /// The shape the filtered backdrop shows through: the node's border box.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub backdrop_clip: Option<Shape>,
  /// What clips the group.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub clip: Option<Shape>,
  /// Drawables whose alpha masks the group.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub mask: Option<Vec<Drawable>>,
}

/// The element a node belongs to.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementInfo {
  /// Its `id`.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub id: Option<String>,
  /// Its tag name.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub tag_name: Option<String>,
  /// Its class name.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub class_name: Option<String>,
  /// Child-index path from the input root.
  pub path: Vec<usize>,
}

/// A glyph id and where it sits from the run's baseline start.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PaintGlyph {
  /// The glyph id in the run's font.
  pub id: u32,
  /// Horizontal offset from the baseline start.
  pub x: f32,
  /// Vertical offset from the baseline start.
  pub y: f32,
}

/// Text shaped with one font and size.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRun {
  /// The final text, after `text-transform` and any inserted ellipsis.
  pub text: String,
  /// The inline element the run came from; absent for the block's own text.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub element: Option<ElementInfo>,
  /// Start of the baseline, in the text node's local space.
  pub x: f32,
  /// Start of the baseline, in the text node's local space.
  pub y: f32,
  /// The run's advance.
  pub width: f32,
  /// Index of the line the run sits on, from 0.
  pub line: usize,
  /// The font's ascent.
  pub ascent: f32,
  /// The font's descent.
  pub descent: f32,
  /// Index into the document's fonts.
  pub font: usize,
  /// The font size.
  pub font_size: f32,
  /// The used line height.
  pub line_height: f32,
  /// Letter spacing, already applied to the glyph positions.
  pub letter_spacing: f32,
  /// Glyph ids positioned from the baseline start.
  pub glyphs: Vec<PaintGlyph>,
  /// The outlines of the glyphs the run's colour fills, as SVG path data positioned from the
  /// baseline start. Colour font layers arrive as their own drawables, and bitmap glyphs such as
  /// some emoji are left out.
  pub outline: String,
  /// Present when `text-fit` scales the line; applies on top of the node's transform.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub transform: Option<Matrix>,
}

/// A font variation axis setting.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaintVariation {
  /// The axis tag.
  pub tag: String,
  /// The axis value.
  pub value: f32,
}

/// A font face at one set of variation coordinates.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaintFont {
  /// The family, when the registry can name it.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub family: Option<String>,
  /// The weight.
  pub weight: f32,
  /// `normal`, `italic`, or `oblique`.
  pub style: &'static str,
  /// `font-stretch` as a percentage.
  pub stretch: f32,
  /// Variation coordinates.
  pub variation_settings: Vec<PaintVariation>,
  /// Index of the face within its collection.
  pub face_index: u32,
}

/// What a node is, and what only that kind carries.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NodeKind {
  /// A CSS box.
  #[serde(rename_all = "camelCase")]
  Box {
    /// The box inset by border and padding, in local space.
    content_box: PaintRect,
    /// Drawn after the descendants.
    outline: Vec<Drawable>,
    /// Present when the box composites as a group.
    #[serde(skip_serializing_if = "Option::is_none")]
    effects: Option<Box<Effects>>,
    /// Clips the descendants, from `overflow`.
    #[serde(skip_serializing_if = "Option::is_none")]
    overflow_clip: Option<Shape>,
  },
  /// One paragraph of laid-out text.
  #[serde(rename_all = "camelCase")]
  Text {
    /// `start` and `end` resolved against the direction.
    text_align: &'static str,
    /// Runs in visual order.
    runs: Vec<TextRun>,
  },
  /// A replaced image, in its content box's space.
  Image {
    /// The image.
    image: ImageSource,
  },
}

/// A node placed on the canvas.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaintNode {
  /// The node's index in the document's node list.
  pub id: usize,
  /// The parent box's index.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parent: Option<usize>,
  /// The element the node belongs to.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub element: Option<ElementInfo>,
  /// Maps local space onto the canvas.
  pub transform: Matrix,
  /// The local width.
  pub width: f32,
  /// The local height.
  pub height: f32,
  /// The node's rectangle on the canvas.
  pub bounds: PaintRect,
  /// What the node draws itself, bottom to top.
  pub drawables: Vec<Drawable>,
  /// The children's indices, in document order.
  pub children: Vec<usize>,
  /// What kind of node this is.
  #[serde(flatten)]
  pub kind: NodeKind,
}

/// Which of a node's drawable lists a draw step draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DrawPart {
  /// The node's `drawables`.
  Drawables,
  /// A box's `outline`.
  Outline,
}

/// One step of painting the document, bottom to top.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum PaintStep {
  /// Draw one of a node's drawable lists under its transform.
  Draw {
    /// The node's index.
    node: usize,
    /// Which list.
    part: DrawPart,
  },
  /// Start a layer that composites per the box's effects at the matching end.
  BeginGroup {
    /// The box's index.
    node: usize,
  },
  /// Composite the box's layer.
  EndGroup {
    /// The box's index.
    node: usize,
  },
  /// Clip the enclosed steps to the box's overflow clip.
  BeginClip {
    /// The box's index.
    node: usize,
  },
  /// End the box's overflow clip.
  EndClip {
    /// The box's index.
    node: usize,
  },
}

/// A painted document.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaintDocument {
  /// The canvas width.
  pub width: f32,
  /// The canvas height.
  pub height: f32,
  /// Every node; the first is the root.
  pub nodes: Vec<PaintNode>,
  /// Every font the runs use.
  pub fonts: Vec<PaintFont>,
  /// The steps a renderer takes, in paint order.
  pub steps: Vec<PaintStep>,
}
