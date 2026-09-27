use std::{collections::HashMap, mem, rc::Rc, sync::Arc};

pub use takumi_core::layout::measure::{MeasuredNode, MeasuredTextRun};
use takumi_core::{
  scene::Scene,
  style::{ComputedStyle, Lang},
};
use typed_builder::TypedBuilder;

use crate::{
  AnimationFrame, Bitmap, Canvas, DitheringAlgorithm, Error, Fonts, RenderContext, Result,
  layout::{
    node::Node,
    tree::{LayoutResults, RenderNode},
  },
  resources::{font::FontsSnapshot, image::ImageSource},
  stacking_context::paint_scene,
  style::{FontFamily, SizingContext, StyleSheet},
  viewport::Viewport,
};

#[derive(Clone, TypedBuilder)]
/// Options for rendering a node, built with [`RenderOptions::builder`].
pub struct RenderOptions<'g> {
  /// The viewport to render the node in.
  pub(crate) viewport: Viewport,
  /// The font context.
  pub(crate) fonts: &'g Fonts,
  /// The node to render.
  pub(crate) node: Node,
  /// Whether to draw debug borders.
  #[builder(default = false)]
  pub(crate) draw_debug_border: bool,
  /// Pre-decoded images keyed by `src`, resolved when a node references that URL.
  #[builder(default)]
  pub(crate) images: HashMap<Arc<str>, ImageSource>,
  /// CSS stylesheets to apply before layout/rendering.
  #[builder(default)]
  pub(crate) stylesheet: Arc<StyleSheet>,
  /// Global animation time in milliseconds.
  #[builder(default = 0)]
  pub(crate) time_ms: u64,
  /// Dithers gradient fills before they quantize to 8-bit.
  #[builder(default)]
  pub(crate) dithering: DitheringAlgorithm,
  /// Per-render font fallback chain (family names in order). `None` uses all
  /// registered families in registration order.
  #[builder(default)]
  pub(crate) font_families: Option<FontFamily>,
  /// Default BCP-47 language tag applied to the root, inherited by nodes without
  /// their own `lang`. Drives locale-aware shaping and line-breaking.
  #[builder(default)]
  pub(crate) lang: Option<Lang>,
}

impl<'g> RenderOptions<'g> {
  /// Returns a reference to the viewport.
  pub fn viewport(&self) -> &Viewport {
    &self.viewport
  }

  /// Returns a reference to the root node.
  pub fn node(&self) -> &Node {
    &self.node
  }

  /// Returns the font context.
  pub fn fonts(&self) -> &'g Fonts {
    self.fonts
  }

  /// Returns the CSS stylesheet applied before layout.
  pub fn stylesheet(&self) -> &Arc<StyleSheet> {
    &self.stylesheet
  }

  /// Returns the pre-decoded images keyed by `src`.
  pub fn images(&self) -> &HashMap<Arc<str>, ImageSource> {
    &self.images
  }
}

#[derive(Clone, TypedBuilder)]
/// A single scene in a sequential animation timeline.
pub struct SequentialScene<'g> {
  /// Render options used when this scene is active.
  pub(crate) options: RenderOptions<'g>,
  /// Duration of this scene in milliseconds.
  pub(crate) duration_ms: u32,
}

impl RenderOptions<'_> {
  fn fonts_snapshot(&self) -> FontsSnapshot {
    self
      .fonts
      .snapshot_with_fallbacks(self.font_families.as_ref())
  }

  /// Builds the render context for these options at `time_ms`, from a font
  /// snapshot and image table the caller may share across frames.
  fn render_context(
    &self,
    fonts: FontsSnapshot,
    images: Rc<HashMap<Arc<str>, ImageSource>>,
    time_ms: u64,
  ) -> RenderContext {
    RenderContext::builder()
      .fonts(fonts)
      .sizing(SizingContext::builder().viewport(self.viewport).build())
      .images(images)
      .stylesheet(self.stylesheet.clone())
      .time_ms(time_ms)
      .draw_debug_border(self.draw_debug_border)
      .dither_gradients(self.dithering != DitheringAlgorithm::None)
      .style(Box::new(ComputedStyle::root(
        self.lang,
        self.font_families.clone(),
      )))
      .build()
  }
}

/// Measures the layout of a node.
pub fn measure<'g>(mut options: RenderOptions<'g>) -> Result<MeasuredNode> {
  let images = Rc::new(mem::take(&mut options.images));
  let render_context = options.render_context(options.fonts_snapshot(), images, options.time_ms);
  let mut root = RenderNode::from_node(&render_context, options.node);
  let layout_results = LayoutResults::compute(&root, options.viewport.into());

  MeasuredNode::of(&mut root, &layout_results, options.viewport.size.into())
}

/// Renders a node to an image.
pub fn render<'g>(mut options: RenderOptions<'g>) -> Result<Bitmap> {
  let images = Rc::new(mem::take(&mut options.images));
  let render_context = options.render_context(options.fonts_snapshot(), images, options.time_ms);

  render_with_context(render_context, options.node, options.viewport)
}

/// Rasterizes `node` under an already-built [`RenderContext`]. The context
/// carries the font snapshot, images, and stylesheet, so animation frames share
/// one snapshot instead of re-snapshotting per frame.
fn render_with_context(
  render_context: RenderContext,
  node: Node,
  viewport: Viewport,
) -> Result<Bitmap> {
  let mut scene = Scene::lay_out(RenderNode::from_node(&render_context, node), viewport, true)?;
  let size = scene.size.map(|length| length.round() as u32);

  if size.width == 0 || size.height == 0 {
    return Err(Error::InvalidViewport);
  }

  let mut canvas = Canvas::try_new(size).ok_or(Error::InvalidViewport)?;

  paint_scene(&mut scene, &mut canvas)?;

  let image = canvas.into_inner()?;

  Ok(Bitmap::from_rgba(image))
}

/// A scene with its per-frame-invariant render state precomputed: the font
/// snapshot and the shared image table do not change between frames of the
/// same scene, so they are built once and cheaply cloned per frame instead of
/// rebuilt (and the whole option tree deep-cloned) each time.
///
/// This is the seam where wider per-frame layout reuse would later live.
pub(crate) struct PreparedScene<'a, 'g> {
  scene: &'a SequentialScene<'g>,
  fonts: FontsSnapshot,
  images: Rc<HashMap<Arc<str>, ImageSource>>,
}

impl<'a, 'g> PreparedScene<'a, 'g> {
  fn new(scene: &'a SequentialScene<'g>) -> Self {
    Self {
      fonts: scene.options.fonts_snapshot(),
      images: Rc::new(scene.options.images.clone()),
      scene,
    }
  }

  fn render_at_time(&self, time_ms: u64) -> Result<Bitmap> {
    let options = &self.scene.options;
    let render_context = options.render_context(self.fonts.clone(), self.images.clone(), time_ms);

    render_with_context(render_context, options.node.clone(), options.viewport)
  }
}

/// Precomputes the per-frame-invariant state for every scene in a timeline.
pub(crate) fn prepare_scenes<'a, 'g>(
  scenes: &'a [SequentialScene<'g>],
) -> Vec<PreparedScene<'a, 'g>> {
  scenes.iter().map(PreparedScene::new).collect()
}

fn resolve_prepared_at_time<'p, 'a, 'g>(
  prepared: &'p [PreparedScene<'a, 'g>],
  time_ms: u64,
) -> Option<(&'p PreparedScene<'a, 'g>, u64)> {
  resolve_at_time(
    prepared.len(),
    |index| prepared[index].scene.duration_ms,
    time_ms,
  )
  .map(|(index, local_time_ms)| (&prepared[index], local_time_ms))
}

/// A frame's start offset and displayed duration, both in milliseconds. Computed
/// from the timeline and frame rate without rendering any pixels.
#[derive(Clone, Copy)]
pub(crate) struct FrameSpan {
  pub(crate) start_ms: u64,
  pub(crate) duration_ms: u32,
}

/// The frame schedule for a timeline at a fixed frame rate: one [`FrameSpan`] per
/// visible frame, dropping any that round to a zero-millisecond duration.
pub(crate) fn frame_spans<'g>(scenes: &[SequentialScene<'g>], fps: u32) -> Vec<FrameSpan> {
  if scenes.is_empty() || fps == 0 {
    return Vec::new();
  }

  let total_duration_ms = total_sequence_duration(scenes);
  if total_duration_ms == 0 {
    return Vec::new();
  }

  let frame_count = total_duration_ms
    .saturating_mul(u64::from(fps))
    .div_ceil(1000);

  (0..frame_count)
    .filter_map(|frame_index| {
      let start_ms = frame_index * 1000 / u64::from(fps);
      let end_ms = ((frame_index + 1) * 1000 / u64::from(fps)).min(total_duration_ms);
      let duration_ms = end_ms.saturating_sub(start_ms);
      (duration_ms != 0).then_some(FrameSpan {
        start_ms,
        duration_ms: duration_ms as u32,
      })
    })
    .collect()
}

/// Renders one frame of the timeline for the given [`FrameSpan`].
pub(crate) fn render_frame(
  prepared: &[PreparedScene<'_, '_>],
  span: FrameSpan,
) -> Result<AnimationFrame> {
  let Some((scene, local_time_ms)) = resolve_prepared_at_time(prepared, span.start_ms) else {
    return Err(Error::InvalidViewport);
  };

  let image = scene.render_at_time(local_time_ms)?;
  Ok(AnimationFrame::new(image, span.duration_ms))
}

/// Renders all frames for a sequential animation timeline at a fixed frame rate.
///
/// Holds every frame in memory. To bound memory, stream straight into an encoder
/// with [`write_animation`](crate::write_animation) instead.
pub fn render_animation<'g>(
  scenes: &[SequentialScene<'g>],
  fps: u32,
) -> Result<Vec<AnimationFrame>> {
  let prepared = prepare_scenes(scenes);

  frame_spans(scenes, fps)
    .into_iter()
    .map(|span| render_frame(&prepared, span))
    .collect()
}

fn total_sequence_duration<'g>(scenes: &[SequentialScene<'g>]) -> u64 {
  scenes
    .iter()
    .map(|scene| u64::from(scene.duration_ms))
    .sum::<u64>()
}

/// Resolves which scene of a `count`-long timeline is active at `time_ms` and the
/// time offset within it, using `duration_ms(index)` for each scene's length.
/// Times past the end clamp to the last scene's final millisecond.
fn resolve_at_time(
  count: usize,
  duration_ms: impl Fn(usize) -> u32,
  time_ms: u64,
) -> Option<(usize, u64)> {
  if count == 0 {
    return None;
  }

  let total_ms = (0..count)
    .map(|index| u64::from(duration_ms(index)))
    .sum::<u64>();
  let clamped_time_ms = time_ms.min(total_ms.saturating_sub(1));
  let mut elapsed_ms = 0_u64;

  for index in 0..count {
    let next_elapsed_ms = elapsed_ms + u64::from(duration_ms(index));
    if clamped_time_ms < next_elapsed_ms {
      return Some((index, clamped_time_ms - elapsed_ms));
    }
    elapsed_ms = next_elapsed_ms;
  }

  let last = count - 1;
  Some((last, u64::from(duration_ms(last).saturating_sub(1))))
}

#[cfg(test)]
fn resolve_scene_at_time<'a, 'g>(
  scenes: &'a [SequentialScene<'g>],
  time_ms: u64,
) -> Option<(&'a SequentialScene<'g>, u64)> {
  resolve_at_time(scenes.len(), |index| scenes[index].duration_ms, time_ms)
    .map(|(index, local_time_ms)| (&scenes[index], local_time_ms))
}

#[cfg(test)]
mod tests {
  use image::Rgba;

  use super::{RenderOptions, SequentialScene, render, render_animation, resolve_scene_at_time};
  use crate::{
    Fonts,
    layout::node::Node,
    measure,
    style::{
      AnimationFillMode, AnimationTime, AnimationTimingFunction, Color, ColorInput, Display,
      FromCssStr, KeyframeRule, KeyframesRule, Length, Length::Px, Position, Style,
      StyleDeclaration, StyleSheet,
    },
    viewport::Viewport,
  };

  fn make_scene<'g>(fonts: &'g Fonts, duration_ms: u32) -> SequentialScene<'g> {
    let options = RenderOptions::builder()
      .fonts(fonts)
      .viewport(Viewport::new((10, 10)))
      .node(Node::container([]))
      .build();

    SequentialScene::builder()
      .duration_ms(duration_ms)
      .options(options)
      .build()
  }

  #[test]
  fn animation_frames_dither_gradients_under_the_option() {
    let fonts = Fonts::default();
    let scene = |dithering| {
      let node = Node::container([]).with_style(
        Style::default()
          .with(StyleDeclaration::display(Display::Flex))
          .with(StyleDeclaration::width(Length::Percentage(100.0)))
          .with(StyleDeclaration::height(Length::Percentage(100.0)))
          .with(StyleDeclaration::background_image(Some(
            crate::style::BackgroundImages::from_css_str(
              "linear-gradient(37deg, #101010, #131313)",
            )
            .unwrap(),
          ))),
      );
      let options = RenderOptions::builder()
        .fonts(&fonts)
        .viewport(Viewport::new((64, 64)))
        .dithering(dithering)
        .node(node)
        .build();

      vec![
        SequentialScene::builder()
          .duration_ms(100)
          .options(options)
          .build(),
      ]
    };

    let plain = render_animation(&scene(crate::DitheringAlgorithm::None), 10).unwrap();
    let dithered = render_animation(&scene(crate::DitheringAlgorithm::OrderedBayer), 10).unwrap();

    assert_ne!(
      plain[0].image.as_raw(),
      dithered[0].image.as_raw(),
      "the dithering option must reach animation frames"
    );
  }

  #[test]
  fn resolve_scene_at_time_uses_cumulative_durations() {
    let fonts = Fonts::default();
    let scenes = vec![make_scene(&fonts, 100), make_scene(&fonts, 200)];

    let scene = resolve_scene_at_time(&scenes, 50);
    assert!(scene.is_some());
    let local_time = scene.map_or(0, |(_, local_time)| local_time);
    assert_eq!(local_time, 50);

    let scene = resolve_scene_at_time(&scenes, 150);
    assert!(scene.is_some());
    let local_time = scene.map_or(0, |(_, local_time)| local_time);
    assert_eq!(local_time, 50);
  }

  #[test]
  fn resolve_scene_at_time_clamps_to_last_scene() {
    let fonts = Fonts::default();
    let scenes = vec![make_scene(&fonts, 100), make_scene(&fonts, 200)];

    let scene = resolve_scene_at_time(&scenes, 500);
    assert!(scene.is_some());
    let local_time = scene.map_or(0, |(_, local_time)| local_time);
    assert_eq!(local_time, 199);
  }

  #[test]
  fn render_sequence_animation_returns_no_frames_for_zero_duration_timelines() {
    let fonts = Fonts::default();
    let scenes = vec![make_scene(&fonts, 0)];

    let frames_result = render_animation(&scenes, 30);
    assert!(frames_result.is_ok());
    let frames = frames_result.unwrap_or_default();

    assert!(frames.is_empty());
  }

  #[test]
  fn oversized_viewport_errors_instead_of_silent_1x1() {
    let fonts = Fonts::default();
    // A width whose row byte length (width * 4) overflows u32, so the backing
    // pixmap cannot allocate.
    let options = RenderOptions::builder()
      .fonts(&fonts)
      .viewport(Viewport::new((2_000_000_000, 1)))
      .node(Node::container([]))
      .build();

    assert!(matches!(
      render(options),
      Err(crate::Error::InvalidViewport)
    ));
  }

  #[test]
  fn viewport_over_pixel_budget_errors_before_allocating() {
    let fonts = Fonts::default();
    let options = RenderOptions::builder()
      .fonts(&fonts)
      .viewport(Viewport::new((8193, 8192)))
      .node(Node::container([]))
      .build();

    assert!(matches!(
      render(options),
      Err(crate::Error::InvalidViewport)
    ));
  }

  #[test]
  fn ordinary_viewport_still_renders() {
    let fonts = Fonts::default();
    let options = RenderOptions::builder()
      .fonts(&fonts)
      .viewport(Viewport::new((100, 100)))
      .node(Node::container([]))
      .build();

    let bitmap = render(options).unwrap();
    assert_eq!((bitmap.width(), bitmap.height()), (100, 100));
  }

  #[test]
  fn write_animation_streams_the_same_bytes_as_render_then_encode() -> crate::Result<()> {
    use std::borrow::Cow;

    use crate::{
      AnimatedGifOptions, AnimatedPngOptions, AnimatedWebpOptions, AnimationFormat,
      write_animated_gif, write_animated_png, write_animated_webp, write_animation,
    };

    let fonts = Fonts::default();
    let scenes = vec![make_scene(&fonts, 100), make_scene(&fonts, 100)];
    let fps = 30;
    let frames = render_animation(&scenes, fps)?;
    assert!(!frames.is_empty());

    let mut eager = Vec::new();
    write_animated_gif(
      Cow::Owned(frames.clone()),
      &mut eager,
      AnimatedGifOptions::default(),
    )?;
    let mut streamed = Vec::new();
    write_animation(
      &scenes,
      fps,
      AnimationFormat::Gif(AnimatedGifOptions::default()),
      &mut streamed,
    )?;
    assert_eq!(eager, streamed, "gif");

    let mut eager = Vec::new();
    write_animated_png(&frames, &mut eager, AnimatedPngOptions::default())?;
    let mut streamed = Vec::new();
    write_animation(
      &scenes,
      fps,
      AnimationFormat::Apng(AnimatedPngOptions::default()),
      &mut streamed,
    )?;
    assert_eq!(eager, streamed, "apng");

    let mut eager = Vec::new();
    write_animated_webp(
      Cow::Owned(frames.clone()),
      &mut eager,
      AnimatedWebpOptions::default(),
    )?;
    let mut streamed = Vec::new();
    write_animation(
      &scenes,
      fps,
      AnimationFormat::WebP(AnimatedWebpOptions::default()),
      &mut streamed,
    )?;
    assert_eq!(eager, streamed, "webp");

    Ok(())
  }

  #[test]
  fn write_animation_rejects_frame_rate_above_format_cap() {
    use crate::{AnimatedGifOptions, AnimatedWebpOptions, AnimationFormat, Error, write_animation};

    let fonts = Fonts::default();
    let scenes = vec![make_scene(&fonts, 100)];

    let mut sink = Vec::new();
    let over_webp = write_animation(
      &scenes,
      91,
      AnimationFormat::WebP(AnimatedWebpOptions::default()),
      &mut sink,
    );
    assert!(matches!(
      over_webp,
      Err(Error::AnimationFrameRateTooHigh {
        fps: 91,
        max_fps: 90
      })
    ));
    assert!(sink.is_empty(), "cap must reject before writing bytes");

    let mut sink = Vec::new();
    let over_gif = write_animation(
      &scenes,
      51,
      AnimationFormat::Gif(AnimatedGifOptions::default()),
      &mut sink,
    );
    assert!(matches!(
      over_gif,
      Err(Error::AnimationFrameRateTooHigh {
        fps: 51,
        max_fps: 50
      })
    ));

    let mut sink = Vec::new();
    let at_cap = write_animation(
      &scenes,
      90,
      AnimationFormat::WebP(AnimatedWebpOptions::default()),
      &mut sink,
    );
    assert!(at_cap.is_ok());
    assert!(!sink.is_empty());
  }

  #[test]
  fn render_sequence_animation_uses_per_frame_integer_durations() {
    let fonts = Fonts::default();
    let scenes = vec![make_scene(&fonts, 150)];

    let frames_result = render_animation(&scenes, 30);
    assert!(frames_result.is_ok());
    let frames = frames_result.unwrap_or_default();
    let durations = frames
      .iter()
      .map(|frame| frame.duration_ms)
      .collect::<Vec<_>>();

    assert_eq!(durations, vec![33, 33, 34, 33, 17]);
    assert_eq!(
      durations
        .iter()
        .map(|duration| u64::from(*duration))
        .sum::<u64>(),
      150
    );
  }

  #[test]
  fn measure_layout_supports_structured_keyframes() {
    let fonts = Fonts::default();
    let node = Node::container([]).with_tag_name("div").with_style(
      Style::default()
        .with(StyleDeclaration::width(Px(100.0)))
        .with(StyleDeclaration::animation_name(
          [Some("grow".to_string())].into(),
        ))
        .with(StyleDeclaration::animation_duration(
          [AnimationTime::from_milliseconds(1000.0)].into(),
        ))
        .with(StyleDeclaration::animation_timing_function(
          [AnimationTimingFunction::Linear].into(),
        ))
        .with(StyleDeclaration::animation_fill_mode(
          [AnimationFillMode::Both].into(),
        )),
    );

    let options = RenderOptions::builder()
      .fonts(&fonts)
      .viewport(Viewport::new((200, 100)))
      .node(node)
      .stylesheet(
        StyleSheet::from(vec![KeyframesRule {
          name: "grow".to_string(),
          keyframes: vec![
            KeyframeRule::builder()
              .offsets([0.0])
              .declarations(
                Style::default()
                  .with(StyleDeclaration::width(Px(100.0)))
                  .into(),
              )
              .build(),
            KeyframeRule::builder()
              .offsets([1.0])
              .declarations(
                Style::default()
                  .with(StyleDeclaration::width(Px(200.0)))
                  .into(),
              )
              .build(),
          ],
          media_queries: Vec::new(),
        }])
        .into(),
      )
      .time_ms(500)
      .build();

    let layout_result = measure(options);
    assert!(layout_result.is_ok());
    let layout = match layout_result {
      Ok(layout) => layout,
      Err(_) => return,
    };

    assert_eq!(layout.width, 150.0);
  }

  #[test]
  fn measure_resolves_absolute_against_relative_skipping_static() {
    // root(relative) > mid(static, offset by margin) > abs(absolute).
    // The absolute's containing block is the relative root, not the static
    // middle, so its transform must resolve against the root's origin (0, 0)
    // plus its own insets — independent of the static middle's offset.
    let fonts = Fonts::default();
    let abs = Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::position(Position::Absolute))
        .with(StyleDeclaration::left(Px(40.0)))
        .with(StyleDeclaration::top(Px(30.0)))
        .with(StyleDeclaration::width(Px(10.0)))
        .with(StyleDeclaration::height(Px(10.0))),
    );
    let mid = Node::container([abs]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::position(Position::Static))
        .with(StyleDeclaration::margin_left(Px(50.0)))
        .with(StyleDeclaration::margin_top(Px(50.0)))
        .with(StyleDeclaration::width(Px(100.0)))
        .with(StyleDeclaration::height(Px(100.0))),
    );
    let root = Node::container([mid]).with_style(
      Style::default()
        .with(StyleDeclaration::display(Display::Block))
        .with(StyleDeclaration::position(Position::Relative))
        .with(StyleDeclaration::width(Px(200.0)))
        .with(StyleDeclaration::height(Px(200.0))),
    );

    let options = RenderOptions::builder()
      .fonts(&fonts)
      .viewport(Viewport::new((200, 200)))
      .node(root)
      .build();

    let layout = match measure(options) {
      Ok(layout) => layout,
      Err(_) => return,
    };
    let mid_node = &layout.children[0];
    let abs_node = &mid_node.children[0];

    // mid (static, in-flow) carries the margin offset; abs (absolute) resolves
    // against the relative root, so it sits at its own insets, not mid's offset.
    assert_eq!((mid_node.transform[4], mid_node.transform[5]), (50.0, 50.0));
    assert_eq!((abs_node.transform[4], abs_node.transform[5]), (40.0, 30.0));
  }

  #[test]
  fn absolute_positioned_children_paint_over_in_flow_background() {
    // CSS 2.1 paint order requires positioned descendants with z-index:auto/0
    // to paint above in-flow non-positioned descendants in the same stacking context.
    // Ref: https://www.w3.org/TR/CSS22/zindex.html#painting-order
    let node = Node::container([Node::container([]).with_style(
      Style::default()
        .with(StyleDeclaration::position(Position::Absolute))
        .with(StyleDeclaration::left(Length::Px(0.0)))
        .with(StyleDeclaration::top(Length::Px(0.0)))
        .with(StyleDeclaration::width(Length::Px(128.0)))
        .with(StyleDeclaration::height(Length::Px(128.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::from_rgb(0xff0000),
        ))),
    )])
    .with_style(
      Style::default()
        .with(StyleDeclaration::position(Position::Relative))
        .with(StyleDeclaration::width(Length::Px(256.0)))
        .with(StyleDeclaration::height(Length::Px(256.0)))
        .with(StyleDeclaration::background_color(ColorInput::Value(
          Color::from_rgb(0x0b1020),
        ))),
    );
    let fonts = Fonts::default();
    let options = RenderOptions::builder()
      .fonts(&fonts)
      .viewport(Viewport::new((256, 256)))
      .node(node.clone())
      .build();
    let measured = match measure(options.clone()) {
      Ok(measured) => measured,
      Err(_) => return,
    };
    assert_eq!(measured.children.len(), 1);
    assert_eq!(measured.children[0].width, 128.0);
    assert_eq!(measured.children[0].height, 128.0);

    let rendered = match render(options) {
      Ok(rendered) => rendered.into_rgba(),
      Err(_) => return,
    };

    let top_left = rendered.get_pixel(10, 10);
    let bottom_right = rendered.get_pixel(220, 220);

    assert_eq!(top_left, &Rgba([255, 0, 0, 255]));
    assert_eq!(bottom_right, &Rgba([11, 16, 32, 255]));
  }
}
