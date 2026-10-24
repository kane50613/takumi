//! Lays an SVG document out to drawing ops, after Blink's `LayoutSVGRoot`: elements become
//! containers, shapes, and pictures with their transforms, clips, masks, and paint resolved, and
//! the painter writes the result as ops.

mod approx_eq;
mod budget;
mod clip_mask;
mod image;
mod marker;
mod object;
mod paint;
mod paint_server;
mod painter;
mod shape;
mod switch;
mod use_element;
mod view_box;

use std::{collections::HashMap, rc::Rc};

use self::{
  approx_eq::ApproxEq,
  budget::Budget,
  object::{
    Container, Fill, LayoutClip, LayoutDocument, LayoutMask, LayoutObject, Paint, PictureContent,
    Stroke,
  },
  painter::SvgPainter,
  view_box::PreserveAspectRatio,
};
use super::{
  SvgDocument,
  element::SvgElementKind,
  length::{LengthAxis, SvgLengthContext},
  style::StyleResolver,
};
use crate::{
  context::RenderContext,
  geometry::{Rect, Size},
  resources::{font::FontsSnapshot, image::SvgOp, svg_size::ViewBox},
  style::{
    Affine, BackgroundImage, BasicShape, BlendMode, Color, ColorInput, ComputedStyle, Display,
    FromCssStr, Isolation, Length, SizingContext, SvgResource,
  },
};

/// An element with the style it resolved to.
#[derive(Clone)]
struct Styled {
  index: usize,
  kind: SvgElementKind,
  context: RenderContext,
}

impl Styled {
  fn style(&self) -> &ComputedStyle {
    &self.context.style
  }

  fn sizing(&self) -> &SizingContext {
    &self.context.sizing
  }
}

/// What a subtree is laid out within.
#[derive(Clone)]
struct Scope {
  /// The clip path whose children are being laid out, which fills them black and strokes them not.
  parent_clip_path: Option<usize>,
  /// The markers being laid out, which a marker may not recurse into.
  parent_markers: Vec<usize>,
  /// The fill and stroke of the `use` element or marked shape, for `context-fill` and
  /// `context-stroke`.
  context_paints: Option<(Option<Fill>, Option<Stroke>)>,
  /// The nearest viewport, which percentages are of.
  view_box: Rect<f32>,
  /// The size a `use` element gives the `svg` it references.
  use_size: (Option<f32>, Option<f32>),
}

/// Lays one document out.
pub(super) struct SvgLayout<'a> {
  document: &'a SvgDocument,
  styles: StyleResolver<'a>,
  /// The clip paths and masks laid out, by element and the viewport their percentages resolved
  /// against.
  clip_paths: HashMap<(usize, [u32; 4]), Rc<LayoutClip>>,
  masks: HashMap<(usize, [u32; 4]), Rc<LayoutMask>>,
  /// Whether each clip path or mask, or one it chains to, is in bounding box units.
  bounding_box_units: HashMap<(clip_mask::Resource, usize), bool>,
  /// The paint servers laid out, by element and the viewport their percentages resolved against.
  paint_servers: HashMap<(usize, [u32; 4]), Paint>,
  /// The images decoded so far by the element that names them.
  images: HashMap<usize, Option<(PictureContent, Size<f32>)>>,
  /// The `use` elements and resources being laid out, which may not contain themselves.
  active: Vec<usize>,
  /// How many SVG images the one being laid out sits in.
  image_depth: usize,
  /// How many more element instances may be laid out.
  budget: Budget,
  fonts: Option<&'a FontsSnapshot>,
  /// Set when the document holds something this pipeline does not draw.
  unsupported: bool,
}

impl<'a> SvgLayout<'a> {
  pub(super) fn new(
    document: &'a SvgDocument,
    current_color: Color,
    fonts: Option<&'a FontsSnapshot>,
  ) -> Self {
    Self::nested(document, current_color, fonts, 0)
  }

  fn nested(
    document: &'a SvgDocument,
    current_color: Color,
    fonts: Option<&'a FontsSnapshot>,
    image_depth: usize,
  ) -> Self {
    Self {
      document,
      styles: StyleResolver::new(document, current_color, fonts),
      clip_paths: HashMap::new(),
      masks: HashMap::new(),
      bounding_box_units: HashMap::new(),
      paint_servers: HashMap::new(),
      images: HashMap::new(),
      active: Vec::new(),
      image_depth,
      budget: Budget::default(),
      fonts,
      unsupported: false,
    }
  }

  /// The ops of the document, none when it holds something this pipeline does not draw.
  pub(super) fn layout(mut self) -> Option<Vec<SvgOp>> {
    let document = self.layout_root()?;

    if self.unsupported {
      return None;
    }

    // A document that outgrows the budget draws nothing.
    Some(SvgPainter::paint(&document.root, &mut self.budget).unwrap_or_default())
  }

  /// The document as Blink's `LayoutSVGRoot` lays it out: the container of everything it draws and
  /// the canvas size.
  fn layout_root(&mut self) -> Option<LayoutDocument> {
    let size = self.document.size;
    let canvas = Size::new(size.width, size.height);
    let parent_context = self.styles.root_context().clone();
    let root = self.styled(0, &parent_context);
    let view_box = self.attribute(0, "viewBox").and_then(ViewBox::parse);
    let view_box_rect = view_box
      .and_then(|view_box| {
        Rect::positive_from_xywh(view_box.x, view_box.y, view_box.width, view_box.height)
      })
      .or_else(|| Rect::positive_from_xywh(0.0, 0.0, size.width, size.height))?;
    let aspect = self
      .attribute(0, "preserveAspectRatio")
      .and_then(PreserveAspectRatio::parse)
      .unwrap_or_default();
    let background = match root.style().background_data.background_color {
      ColorInput::Value(color) if color.0[3] != 0 => Some(color),
      _ => None,
    };
    let scope = Scope::root(view_box_rect);
    let mut tree = Container::new(Affine::IDENTITY);

    if !self.is_visible_element(&root, &scope) {
      return Some(LayoutDocument {
        width: size.width,
        height: size.height,
        root: tree,
      });
    }

    let root_transform = ViewBox {
      x: view_box_rect.x(),
      y: view_box_rect.y(),
      width: view_box_rect.width(),
      height: view_box_rect.height(),
    }
    .to_transform(aspect, canvas);
    let identity = root_transform.to_cols_array() == Affine::IDENTITY.to_cols_array();

    if identity && background.is_none() {
      self.convert_element(0, &parent_context, &scope, &mut tree);
    } else {
      let mut group = Container::new(root_transform);

      group.transform = root_transform;

      if let Some(background) = background {
        group
          .children
          .push(LayoutObject::background(view_box_rect, background)?);
      }

      self.convert_element(0, &parent_context, &scope, &mut group);
      group.finish();
      tree.children.push(LayoutObject::Container(Box::new(group)));
    }

    // A size given in percentages of a viewport nothing gives takes the extent of the content.
    let relative = |name| {
      self
        .length_attribute(0, name)
        .is_none_or(|length| matches!(length, Length::Percentage(_)))
    };
    let (mut width, mut height) = (size.width, size.height);

    if (relative("width") || relative("height")) && view_box.is_none() {
      let extent = tree.absolute_bounds();

      if let Some(bounds) = extent
        && bounds.right > 0.0
        && bounds.bottom > 0.0
      {
        width = bounds.right;
        height = bounds.bottom;
      }
    }

    Some(LayoutDocument {
      width,
      height,
      root: tree,
    })
  }

  /// Runs `f` one level deeper, none past the depth a document may nest to.
  fn deeper<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> Option<R> {
    if !self.budget.enter() {
      return None;
    }

    let result = f(self);

    self.budget.leave();
    Some(result)
  }

  fn styled(&self, index: usize, parent: &RenderContext) -> Styled {
    Styled {
      index,
      kind: self.document.elements[index].kind,
      context: self.styles.resolve(index, parent),
    }
  }

  /// The element styled where it stands in the document.
  fn styled_in_place(&self, index: usize) -> Styled {
    Styled {
      index,
      kind: self.document.elements[index].kind,
      context: self.styles.in_place(index),
    }
  }

  fn attribute(&self, index: usize, name: &str) -> Option<&'a str> {
    self.styles.node(index).attribute(name)
  }

  fn length_attribute(&self, index: usize, name: &str) -> Option<Length> {
    Length::from_css_str(self.attribute(index, name)?).ok()
  }

  fn has_attribute(&self, index: usize, name: &str) -> bool {
    self.attribute(index, name).is_some()
  }

  /// The element a `url()` names.
  fn reference(&self, resource: &SvgResource) -> Option<usize> {
    self.document.last_ids.get(resource.fragment()?).copied()
  }

  /// The element an `href` attribute names.
  fn href_target(&self, index: usize) -> Option<usize> {
    let fragment = self.attribute(index, "href")?.trim().strip_prefix('#')?;

    self.document.last_ids.get(fragment).copied()
  }

  /// `length` in user units against the viewport of `scope`.
  fn px(&self, styled: &Styled, scope: &Scope, length: Length, axis: LengthAxis) -> f32 {
    SvgLengthContext::new(scope.view_box).resolve(length, axis, styled.sizing())
  }

  /// A length attribute of the element in user units, `default` where it has none.
  fn attribute_px(
    &self,
    styled: &Styled,
    scope: &Scope,
    name: &str,
    axis: LengthAxis,
    default: Length,
  ) -> f32 {
    let length = self.length_attribute(styled.index, name).unwrap_or(default);

    self.px(styled, scope, length, axis)
  }

  /// Blink's `LayoutObject::StyleRef().Display() != kNone` and the conditional processing of
  /// `SVGTests`, plus a transform that keeps the element drawable.
  fn is_visible_element(&self, styled: &Styled, scope: &Scope) -> bool {
    styled.style().base_data.display != Display::None
      && self
        .authored_transform(styled, scope)
        .is_none_or(Affine::is_renderable)
      && self.passes_conditions(styled.index)
  }

  /// The element's `transform` before its `transform-origin` moves it.
  fn authored_transform(&self, styled: &Styled, scope: &Scope) -> Option<Affine> {
    let transforms = styled.style().svg_data.transform.as_ref()?;

    Some(Affine::from_transforms(
      transforms.iter(),
      styled.sizing(),
      scope.view_box.width(),
      scope.view_box.height(),
    ))
  }

  /// The element's `transform` moved by its `transform-origin`. A transform that keeps no scale
  /// counts as none.
  fn resolve_transform(&self, styled: &Styled, scope: &Scope) -> Affine {
    let transform = self
      .authored_transform(styled, scope)
      .filter(|transform| transform.is_renderable())
      .unwrap_or(Affine::IDENTITY);
    let (x, y) = styled.style().svg_data.transform_origin.to_point(
      styled.sizing(),
      scope.view_box.width(),
      scope.view_box.height(),
    );

    if x == 0.0 && y == 0.0 {
      return transform;
    }

    Affine::translation(x, y) * transform * Affine::translation(-x, -y)
  }

  fn convert_children(
    &mut self,
    index: usize,
    context: &RenderContext,
    scope: &Scope,
    parent: &mut Container,
  ) {
    for child in self.document.element_children(index) {
      self.convert_element(child, context, scope, parent);
    }
  }

  /// Lays out the element at `index`, styled against `parent_context`, into `parent`.
  fn convert_element(
    &mut self,
    index: usize,
    parent_context: &RenderContext,
    scope: &Scope,
    parent: &mut Container,
  ) {
    self.deeper(|layout| layout.lay_out_element(index, parent_context, scope, parent));
  }

  fn lay_out_element(
    &mut self,
    index: usize,
    parent_context: &RenderContext,
    scope: &Scope,
    parent: &mut Container,
  ) {
    if !self.budget.spend_instance() {
      return;
    }

    let kind = self.document.elements[index].kind;

    if !kind.is_graphic()
      && !matches!(
        kind,
        SvgElementKind::G | SvgElementKind::Switch | SvgElementKind::Svg
      )
    {
      return;
    }

    let styled = self.styled(index, parent_context);

    if !self.is_visible_element(&styled, scope) {
      return;
    }

    match kind {
      SvgElementKind::Use => self.convert_use(&styled, scope, parent),
      SvgElementKind::Switch => self.convert_switch(&styled, scope, parent),
      // A nested `svg` makes its own group, so it must not be wrapped in another.
      SvgElementKind::Svg if index != 0 => {
        self.convert_nested_svg(&styled, scope, parent);
      }
      _ => {
        let group = self.convert_group(&styled, scope, false, parent, |layout, group| {
          layout.convert_element_content(&styled, scope, group);
        });

        if let Some(group) = group {
          parent
            .children
            .push(LayoutObject::Container(Box::new(group)));
        }
      }
    }
  }

  fn convert_element_content(&mut self, styled: &Styled, scope: &Scope, parent: &mut Container) {
    match styled.kind {
      kind if kind.is_shape() => {
        if let Some(commands) = self.shape_commands(styled, scope) {
          self.convert_shape(styled, scope, commands, parent);
        }
      }
      SvgElementKind::Image => self.convert_image(styled, scope, parent),
      SvgElementKind::Svg | SvgElementKind::G => {
        self.convert_children(styled.index, &styled.context, scope, parent);
      }
      SvgElementKind::Text => self.unsupported = true,
      _ => {}
    }
  }

  /// A `clipPath`'s children, which are shapes and `use` elements only.
  fn convert_clip_path_children(
    &mut self,
    index: usize,
    context: &RenderContext,
    scope: &Scope,
    parent: &mut Container,
  ) {
    for child in self.document.element_children(index) {
      self.convert_clip_path_child(child, context, scope, parent);
    }
  }

  /// One child of a `clipPath`, a shape, a `use`, or a `text`. A `line` has no area to clip to.
  fn convert_clip_path_child(
    &mut self,
    index: usize,
    context: &RenderContext,
    scope: &Scope,
    parent: &mut Container,
  ) {
    let kind = self.document.elements[index].kind;

    if !kind.is_graphic() || !self.budget.spend_instance() {
      return;
    }

    let styled = self.styled(index, context);

    if !self.is_visible_element(&styled, scope) {
      return;
    }

    if kind == SvgElementKind::Use {
      self.convert_use(&styled, scope, parent);
      return;
    }

    let group = self.convert_group(
      &styled,
      scope,
      false,
      parent,
      |layout, group| match styled.kind {
        SvgElementKind::Line => {}
        SvgElementKind::Text => layout.unsupported = true,
        kind if kind.is_shape() => {
          if let Some(commands) = layout.shape_commands(&styled, scope) {
            layout.convert_shape(&styled, scope, commands, group);
          }
        }
        _ => {}
      },
    );

    if let Some(group) = group {
      parent
        .children
        .push(LayoutObject::Container(Box::new(group)));
    }
  }

  /// Blink's `SVGContainerPainter` setup: the container an element makes, with its transform,
  /// opacity, blend mode, clip, and mask, or none when it makes nothing. A container that adds
  /// nothing gives its children to `parent` instead.
  fn convert_group(
    &mut self,
    styled: &Styled,
    scope: &Scope,
    force: bool,
    parent: &mut Container,
    collect_children: impl FnOnce(&mut Self, &mut Container),
  ) -> Option<Container> {
    let style = styled.style();
    // A `clipPath` child cannot have an opacity.
    let opacity = if scope.parent_clip_path.is_none() {
      style.svg_data.opacity.0.clamp(0.0, 1.0)
    } else {
      1.0
    };
    let transform = self.resolve_transform(styled, scope);
    let blend_mode = style.misc_data.mix_blend_mode;
    let isolate = style.misc_data.isolation == Isolation::Isolate;
    let is_g_or_use = matches!(styled.kind, SvgElementKind::G | SvgElementKind::Use);
    let mut group = Container::new(parent.abs_transform * transform);

    group.transform = transform;
    group.opacity = opacity;
    group.blend_mode = blend_mode;
    group.isolate = isolate;
    collect_children(self, &mut group);

    // The bounding box of the children, which clip paths and masks may be in units of.
    let object_bounding_box = group.object_bounding_box();
    let mut clip_path = None;

    if let Some(BasicShape::Url(resource)) = &style.misc1_data.clip_path
      && let Some(link) = self.reference(resource)
      && !self.active.contains(&link)
    {
      clip_path = Some(self.convert_clip_path(link, scope, object_bounding_box)?);
    }

    let mut mask = None;

    if scope.parent_clip_path.is_none()
      && let Some(link) = self.mask_reference(style)
      && !self.active.contains(&link)
    {
      mask = Some(self.convert_mask(link, scope, object_bounding_box)?);
    }

    let required = !opacity.approx_eq(1.0)
      || clip_path.is_some()
      || mask.is_some()
      || transform.to_cols_array() != Affine::IDENTITY.to_cols_array()
      || blend_mode != BlendMode::Normal
      || isolate
      || is_g_or_use
      || force;

    if !required {
      parent.children.append(&mut group.children);
      return None;
    }

    group.clip_path = clip_path;
    group.mask = mask;
    group.finish();
    Some(group)
  }

  /// The element a `mask` property names: the `mask-image` layer that is a `url()` to a fragment.
  fn mask_reference(&self, style: &ComputedStyle) -> Option<usize> {
    let images = style.misc2_data.mask_image.as_ref()?;
    let BackgroundImage::Url(url) = images.first()? else {
      return None;
    };

    self.document.last_ids.get(url.strip_prefix('#')?).copied()
  }
}

impl Scope {
  fn root(view_box: Rect<f32>) -> Self {
    Self {
      parent_clip_path: None,
      parent_markers: Vec::new(),
      context_paints: None,
      view_box,
      use_size: (None, None),
    }
  }
}
