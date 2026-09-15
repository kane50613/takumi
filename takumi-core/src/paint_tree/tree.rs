//! The serializable paint tree.

use serde::Serialize;

use crate::style::Color;

/// A color as `[r, g, b, a]`, each `0..=255`.
pub type Rgba = [u8; 4];

/// A rectangle in the owning node's border-box space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PaintRect {
  /// Left edge.
  pub x: f32,
  /// Top edge.
  pub y: f32,
  /// Width.
  pub width: f32,
  /// Height.
  pub height: f32,
}

/// Corner radii as `[x, y]` pairs: top-left, top-right, bottom-right, bottom-left.
pub type Radii = [[f32; 2]; 4];

/// Everything the backends paint for a node tree, in paint order.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaintTree {
  /// Canvas width in device pixels.
  pub width: f32,
  /// Canvas height in device pixels.
  pub height: f32,
  /// Font instances the runs reference by index.
  pub fonts: Vec<PaintFont>,
  /// The root node.
  pub root: PaintNode,
}

/// A font instance a run was shaped with.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaintFont {
  /// The family the face was registered under; `None` for a face the registry cannot name.
  pub family: Option<String>,
  /// Index of the face within its collection.
  pub face_index: u32,
  /// Weight class, `wght` applied when the face is variable.
  pub weight: f32,
  /// CSS `font-style` of the face: `normal`, `italic`, or `oblique`.
  pub style: String,
  /// Width as a percentage of normal.
  pub width: f32,
  /// Variation coordinates the run was shaped at.
  pub variations: Vec<PaintVariation>,
  /// Stroke width in px for synthetic bold.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub synthetic_bold_width: Option<f32>,
  /// Synthetic oblique angle in degrees.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub synthetic_oblique_angle: Option<f32>,
}

/// One variation axis setting.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaintVariation {
  /// Four-character axis tag such as `wght`.
  pub tag: String,
  /// The coordinate in user space.
  pub value: f32,
}

/// One painted box: a compositing group whose `opacity`, `clip`, and blend apply to everything
/// inside it. Paints in order: `box_decoration`, `image`, `inline_backgrounds`, `runs`,
/// `children`, then `box_decoration.outline`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaintNode {
  /// The node this box came from; `None` for an anonymous box.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub source: Option<PaintSource>,
  /// Border-box width in device pixels.
  pub width: f32,
  /// Border-box height in device pixels.
  pub height: f32,
  /// Absolute transform placing the border box on the canvas, as `[a, b, c, d, e, f]`.
  pub transform: [f32; 6],
  /// Group opacity, `0..=1`.
  pub opacity: f32,
  /// `mix-blend-mode` other than `normal`.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub blend_mode: Option<String>,
  /// Whether `isolation: isolate` applies.
  pub isolate: bool,
  /// Overflow clip applied to the children.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub clip: Option<PaintClip>,
  /// Box decorations, when the box paints any.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub box_decoration: Option<PaintBoxDecoration>,
  /// Replaced image content.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub image: Option<PaintImage>,
  /// `text-shadow` layers under every run, later-listed shadows lowest.
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub text_shadows: Vec<PaintShadow>,
  /// Inline-span backgrounds, one rounded rect per line, outer spans first.
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub inline_backgrounds: Vec<PaintInlineBackground>,
  /// Shaped text runs in visual order.
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub runs: Vec<PaintTextRun>,
  /// Effects the tree carries as CSS text instead of resolving.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub unresolved_effects: Option<PaintUnresolvedEffects>,
  /// Boxes painted after this one, in paint order.
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub children: Vec<PaintNode>,
}

/// Where a box came from in the input tree.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaintSource {
  /// Child-index path from the input root.
  pub path: Vec<usize>,
  /// The node's `id`.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub id: Option<String>,
  /// The node's tag name.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub tag_name: Option<String>,
  /// The node's class name.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub class_name: Option<String>,
}

/// The rounded padding box children clip to, and which axes clip.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaintClip {
  /// The clip rectangle.
  pub rect: PaintRect,
  /// The rectangle's corner radii.
  pub radii: Radii,
  /// Whether the horizontal axis clips.
  pub x: bool,
  /// Whether the vertical axis clips.
  pub y: bool,
}

/// A box's decorations.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaintBoxDecoration {
  /// The background.
  pub background: PaintBackground,
  /// The border.
  pub border: PaintBorder,
  /// `box-shadow` layers.
  pub shadows: PaintBoxShadows,
  /// The outline, painted after the children.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub outline: Option<PaintOutline>,
}

/// A box's background.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaintBackground {
  /// `background-color`, when visible.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub color: Option<Rgba>,
  /// `background-clip`; `text` means the background fills the glyphs instead of the box.
  pub clip: String,
  /// Image layers, bottom to top.
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub layers: Vec<PaintBackgroundLayer>,
}

/// One `background-image` layer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaintBackgroundLayer {
  /// What the layer fills each tile with.
  pub fill: PaintFill,
  /// Where the tiles land.
  pub tiles: PaintTiles,
  /// `background-blend-mode` for the layer.
  pub blend_mode: String,
}

/// Tile placement of a background layer in border-box space.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaintTiles {
  /// Left edges of the tiles.
  pub xs: Vec<i32>,
  /// Top edges of the tiles.
  pub ys: Vec<i32>,
  /// Tile width.
  pub width: u32,
  /// Tile height.
  pub height: u32,
}

/// The fill of one background tile.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PaintFill {
  /// A linear gradient resolved against the tile.
  #[serde(rename_all = "camelCase")]
  Linear {
    /// The gradient as CSS.
    css: String,
    /// Whether the gradient repeats.
    repeating: bool,
    /// Direction vector, x.
    dir_x: f32,
    /// Direction vector, y.
    dir_y: f32,
    /// Length of the gradient axis in px.
    axis_length: f32,
    /// Stops in axis px from the axis start.
    stops: Vec<PaintGradientStop>,
  },
  /// A radial gradient resolved against the tile.
  #[serde(rename_all = "camelCase")]
  Radial {
    /// The gradient as CSS.
    css: String,
    /// Whether the gradient repeats.
    repeating: bool,
    /// Center x in tile px.
    cx: f32,
    /// Center y in tile px.
    cy: f32,
    /// Horizontal radius in px.
    radius_x: f32,
    /// Vertical radius in px.
    radius_y: f32,
    /// Stops in px from the center along the radius.
    stops: Vec<PaintGradientStop>,
  },
  /// A conic gradient, carried as CSS.
  Conic {
    /// The gradient as CSS.
    css: String,
  },
  /// An image tile.
  Image {
    /// The image URL, when the source was one.
    #[serde(skip_serializing_if = "Option::is_none")]
    src: Option<String>,
  },
}

/// A resolved gradient stop.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaintGradientStop {
  /// The stop color.
  pub color: Rgba,
  /// The stop position in px.
  pub position: f32,
}

/// A box's border, sides ordered top, right, bottom, left.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaintBorder {
  /// Used side widths.
  pub widths: [f32; 4],
  /// Side colors.
  pub colors: [Rgba; 4],
  /// Side styles as CSS keywords.
  pub styles: [String; 4],
  /// Corner radii.
  pub radii: Radii,
}

/// `box-shadow` layers split by where they fall.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaintBoxShadows {
  /// Shadows inside the box.
  pub inset: Vec<PaintShadow>,
  /// Shadows outside it.
  pub outer: Vec<PaintShadow>,
}

/// A resolved shadow.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaintShadow {
  /// Horizontal offset.
  pub offset_x: f32,
  /// Vertical offset.
  pub offset_y: f32,
  /// Blur radius.
  pub blur: f32,
  /// Spread radius.
  pub spread: f32,
  /// The shadow color.
  pub color: Rgba,
}

/// A box's outline, drawn outside the border box.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaintOutline {
  /// Line width.
  pub width: f32,
  /// Line color.
  pub color: Rgba,
  /// Line style as a CSS keyword.
  pub style: String,
  /// Gap between the border edge and the outline.
  pub offset: f32,
}

/// Replaced image content placed inside its content box.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaintImage {
  /// The image URL, when the source was one.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub src: Option<String>,
  /// The content box the image is placed in and clipped to.
  pub content_box: PaintRect,
  /// Where the whole image draws after `object-fit` and `object-position`.
  pub placement: PaintRect,
}

/// A shaped text run.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaintTextRun {
  /// The run's text.
  pub text: String,
  /// Start of the run's baseline, x.
  pub x: f32,
  /// The run's baseline, y.
  pub y: f32,
  /// Advance of the run.
  pub width: f32,
  /// Typographic ascent above the baseline.
  pub ascent: f32,
  /// Typographic descent below the baseline.
  pub descent: f32,
  /// Index into [`PaintTree::fonts`].
  pub font_index: usize,
  /// Font size the run was shaped at.
  pub font_size: f32,
  /// Fill color.
  pub color: Rgba,
  /// The span's `opacity`.
  pub opacity: f32,
  /// A transform on top of the node's, when text-fit scales the line.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub transform: Option<[f32; 6]>,
  /// Glyphs relative to the run origin.
  pub glyphs: Vec<PaintGlyph>,
  /// Text decoration lines.
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub decorations: Vec<PaintDecoration>,
  /// `-webkit-text-stroke`, when visible.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub stroke: Option<PaintStroke>,
  /// UTF-8 byte range of the text within the node's inline text.
  pub text_byte_range: [usize; 2],
  /// The inline span the run came from.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub span_id: Option<u64>,
}

/// A positioned glyph.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaintGlyph {
  /// Glyph id in the run's font.
  pub id: u32,
  /// Horizontal offset from the run origin.
  pub x: f32,
  /// Vertical offset from the run origin.
  pub y: f32,
}

/// One text decoration line, a rectangle under `transform` in border-box space.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaintDecoration {
  /// `underline`, `overline`, or `line-through`.
  pub line: String,
  /// Placement of the rectangle.
  pub transform: [f32; 6],
  /// Rectangle width.
  pub width: f32,
  /// Rectangle height.
  pub height: f32,
  /// Line color.
  pub color: Rgba,
}

/// A text stroke.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaintStroke {
  /// Stroke color.
  pub color: Rgba,
  /// Stroke width.
  pub width: f32,
}

/// One rounded rect an inline span fills on one line.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaintInlineBackground {
  /// The rectangle.
  pub rect: PaintRect,
  /// Corner radii.
  pub radii: Radii,
  /// Fill color.
  pub color: Rgba,
  /// The span's `opacity`.
  pub opacity: f32,
}

/// Effects exported as CSS text.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaintUnresolvedEffects {
  /// `filter`.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub filter: Option<String>,
  /// `backdrop-filter`.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub backdrop_filter: Option<String>,
  /// `mask-image`.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub mask_image: Option<String>,
  /// `clip-path`.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub clip_path: Option<String>,
}

/// The `[r, g, b, a]` form of a color.
pub(super) fn rgba(color: Color) -> Rgba {
  color.0
}
