use std::sync::Arc;

use arc_swap::ArcSwap;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use takumi_bindings_common::{
  default_fonts,
  font_cache::FontCache,
  input::{ListedFont, ListedFonts},
  options::RendererOptions,
  source_cache::SourceCache,
};
use takumi_core::{Fonts, resources::image::ResourceCache};

use crate::{
  NodeSource, RegisteredFamily, deserialize_with_tracing,
  load_font_task::LoadFontTask,
  map_error,
  measure_task::MeasureTask,
  needs_task::{AnimationNeedsTask, NeedsTask},
  parse_font_input, read_options,
  render_animation_task::{AnimationInput, RenderAnimationTask},
  render_job::RenderJob,
  render_task::RenderTask,
  svg_render_task::SvgRenderTask,
};

/// Represents a single run of text in a measured node.
#[napi(object)]
pub struct MeasuredTextRun {
  /// The text content of the run.
  pub text: String,
  /// The inline x-coordinate of the run.
  pub x: f64,
  /// The inline y-coordinate of the run.
  pub y: f64,
  /// The width of the run.
  pub width: f64,
  /// The height of the run.
  pub height: f64,
}

impl From<takumi_raster::MeasuredTextRun> for MeasuredTextRun {
  fn from(run: takumi_raster::MeasuredTextRun) -> Self {
    Self {
      text: run.text,
      x: run.x as f64,
      y: run.y as f64,
      width: run.width as f64,
      height: run.height as f64,
    }
  }
}

/// Represents a node that has been measured, including its layout information.
#[napi(object)]
pub struct MeasuredNode {
  /// The measured width of the node.
  pub width: f64,
  /// The measured height of the node.
  pub height: f64,
  /// The transformation matrix of the node.
  #[napi(ts_type = "[number, number, number, number, number, number]")]
  pub transform: Vec<f64>,
  /// The children of the node.
  pub children: Vec<MeasuredNode>,
  /// The text runs within the node.
  pub runs: Vec<MeasuredTextRun>,
}

impl From<takumi_raster::MeasuredNode> for MeasuredNode {
  fn from(node: takumi_raster::MeasuredNode) -> Self {
    Self {
      width: node.width as f64,
      height: node.height as f64,
      transform: node.transform.iter().map(|&x| x as f64).collect(),
      children: node.children.into_iter().map(Into::into).collect(),
      runs: node.runs.into_iter().map(Into::into).collect(),
    }
  }
}

/// The images a node tree looks up by URL and the listed fonts the render loads.
#[napi(object)]
pub struct RenderNeeds {
  /// The image URLs the tree looks up, except `data:` URIs and inline SVG, sorted.
  pub images: Vec<String>,
  /// The ids of the listed fonts the render loads, sorted.
  pub fonts: Vec<u32>,
}

impl RenderNeeds {
  /// What `needs` reports, with the ids of the `listed` fonts the render loads.
  pub(crate) fn new(needs: takumi_raster::RenderNeeds, listed: Option<&ListedFonts>) -> Self {
    Self {
      fonts: listed.map_or_else(Vec::new, |listed| {
        listed
          .to_load(&needs.faces)
          .into_iter()
          .map(|id| id as u32)
          .collect()
      }),
      images: needs.images.into_iter().collect(),
    }
  }
}

/// The main renderer for Takumi image rendering engine (Node.js version).
#[napi]
pub struct Renderer {
  pub(crate) state: Arc<RendererState>,
}

/// The registered fonts and what registered them, published together.
#[derive(Clone)]
pub(crate) struct FontState {
  fonts: Arc<Fonts>,
  cache: FontCache,
}

pub(crate) struct RendererState {
  /// Read without waiting through `font_state.load()`, so renders never block. Every change runs
  /// on the JS thread that owns the renderer, so changes never race, and renders in flight keep
  /// the state they started with.
  font_state: ArcSwap<FontState>,
  pub(crate) resource_cache: ResourceCache,
  pub(crate) sources: SourceCache,
}

impl RendererState {
  /// Applies `change` to a copy of the fonts and publishes it. Call it on the JS thread only.
  pub(crate) fn update_fonts<T>(
    &self,
    change: impl FnOnce(&mut Fonts, &mut FontCache) -> Result<T>,
  ) -> Result<T> {
    let mut state = FontState::clone(&self.font_state.load());
    let changed = change(Arc::make_mut(&mut state.fonts), &mut state.cache)?;

    self.font_state.store(Arc::new(state));

    Ok(changed)
  }

  /// A render's `fonts` list read against the fonts this renderer holds.
  pub(crate) fn listed(
    &self,
    families: &mut Option<Vec<String>>,
    listed: Option<Vec<ListedFont>>,
  ) -> Option<ListedFonts> {
    let listed = ListedFonts::read(&self.font_state.load().cache, listed?);

    if families.is_none() {
      families.clone_from(&listed.families);
    }

    Some(listed)
  }

  /// The fonts for a render that names `families`, which are marked used.
  pub(crate) fn fonts_for_render(&self, families: Option<&[String]>) -> Arc<Fonts> {
    let state = self.font_state.load();

    state.cache.touch(families.unwrap_or_default());

    Arc::clone(&state.fonts)
  }
}

/// Represents a single scene in a sequential animation timeline.
#[napi(object)]
pub struct AnimationScene<'ctx> {
  /// The node tree to render for this scene, the `{ json }` text of one, or HTML markup.
  #[napi(ts_type = "Node | string | { json: string }")]
  pub node: Either<String, Object<'ctx>>,
  /// The duration of this scene in milliseconds.
  pub duration_ms: u32,
}

#[napi]
impl Renderer {
  /// Creates a new Renderer instance.
  #[napi(
    constructor,
    ts_args_type = "options?: RendererOptions | undefined | null"
  )]
  pub fn new(env: Env, options: Option<Object>) -> Result<Self> {
    crate::pool::register_cleanup(&env);

    let options: RendererOptions = read_options(options)?;

    Ok(Self {
      state: Arc::new(RendererState {
        font_state: ArcSwap::from_pointee(FontState {
          fonts: Arc::new(default_fonts().map_err(map_error)?),
          cache: FontCache::new(options.font_cache_bytes()),
        }),
        resource_cache: match options.cache_bytes() {
          Some(bytes) => ResourceCache::new(bytes),
          None => ResourceCache::default(),
        },
        sources: SourceCache::default(),
      }),
    })
  }

  /// Register font into the renderer, under `key` when its loader gives one, returning the
  /// families produced. It is never released for space.
  #[napi(
    js_name = "registerFont",
    ts_args_type = "fonts: Font, key?: string | null, signal?: AbortSignal",
    ts_return_type = "Promise<RegisteredFamily[]>"
  )]
  pub fn register_font(
    &self,
    env: Env,
    font: Object,
    key: Option<String>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<LoadFontTask>> {
    self.load_font(env, font, key, true, signal)
  }

  /// Registers a font a render's `fonts` list loaded under `key`, released for space once the
  /// font cache is past its budget.
  #[napi(
    js_name = "cacheFont",
    ts_args_type = "fonts: Font, key: string, signal?: AbortSignal",
    ts_return_type = "Promise<RegisteredFamily[]>"
  )]
  pub fn cache_font(
    &self,
    env: Env,
    font: Object,
    key: String,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<LoadFontTask>> {
    self.load_font(env, font, Some(key), false, signal)
  }

  /// The families a registration under `key` added, when the renderer still holds them; `pin`
  /// keeps them from being released for space.
  #[napi(js_name = "cachedFonts")]
  pub fn cached_fonts(&self, key: String, pin: bool) -> Result<Option<Vec<RegisteredFamily>>> {
    let registered = if pin {
      self.state.update_fonts(|_, cache| Ok(cache.pin(&key)))?
    } else {
      self.state.font_state.load().cache.lookup(&key)
    };

    Ok(registered.map(|registered| registered.into_iter().map(Into::into).collect()))
  }

  fn load_font(
    &self,
    env: Env,
    font: Object,
    key: Option<String>,
    pinned: bool,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<LoadFontTask>> {
    let (info, buffer) = parse_font_input(env, font)?;

    Ok(AsyncTask::with_optional_signal(
      LoadFontTask {
        state: Arc::clone(&self.state),
        buffer,
        info,
        key,
        pinned,
      },
      signal,
    ))
  }

  /// Removes every face registered under `family`, each `unicodeRange` subset alike, returning
  /// whether it had any.
  #[napi(js_name = "unregisterFont")]
  pub fn unregister_font(&self, family: String) -> Result<bool> {
    self.state.update_fonts(|fonts, cache| {
      cache.forget(&family);

      Ok(fonts.unregister_family(&family))
    })
  }

  /// Removes every registered font, leaving the bundled fallback font.
  #[napi(js_name = "clearFonts")]
  pub fn clear_fonts(&self) -> Result<()> {
    self.state.update_fonts(|fonts, cache| {
      *fonts = default_fonts().map_err(map_error)?;
      cache.clear();

      Ok(())
    })
  }

  /// Renders a node tree, or HTML, into an image buffer asynchronously.
  #[napi(
    ts_args_type = "source: Node | string | { json: string }, options?: RenderOptions, signal?: AbortSignal",
    ts_return_type = "Promise<Buffer<ArrayBuffer>>"
  )]
  pub fn render(
    &self,
    source: Either<String, Object>,
    options: Option<Object>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<RenderTask>> {
    Ok(AsyncTask::with_optional_signal(
      RenderTask(self.job(source, options)?),
      signal,
    ))
  }

  /// Renders a node tree, or HTML, into an SVG document string asynchronously.
  #[napi(
    ts_args_type = "source: Node | string | { json: string }, options?: SvgRenderOptions, signal?: AbortSignal",
    ts_return_type = "Promise<string>"
  )]
  pub fn render_svg(
    &self,
    source: Either<String, Object>,
    options: Option<Object>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<SvgRenderTask>> {
    Ok(AsyncTask::with_optional_signal(
      SvgRenderTask::new(
        NodeSource::from_js(source)?,
        read_options(options)?,
        Arc::clone(&self.state),
      ),
      signal,
    ))
  }

  /// Measures a node tree, or HTML, and returns layout information asynchronously.
  #[napi(
    ts_args_type = "source: Node | string | { json: string }, options?: RenderOptions, signal?: AbortSignal",
    ts_return_type = "Promise<MeasuredNode>"
  )]
  pub fn measure(
    &self,
    source: Either<String, Object>,
    options: Option<Object>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<MeasureTask>> {
    Ok(AsyncTask::with_optional_signal(
      MeasureTask(self.job(source, options)?),
      signal,
    ))
  }

  /// Reads what a node tree, or HTML, needs fetched before it renders, asynchronously.
  #[napi(
    ts_args_type = "source: Node | string | { json: string }, options?: RenderOptions, fonts?: ListedFont[], signal?: AbortSignal",
    ts_return_type = "Promise<RenderNeeds>"
  )]
  pub fn needs(
    &self,
    source: Either<String, Object>,
    options: Option<Object>,
    fonts: Option<Object>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<NeedsTask>> {
    Ok(AsyncTask::with_optional_signal(
      NeedsTask::new(
        NodeSource::from_js(source)?,
        read_options(options)?,
        fonts.map(deserialize_with_tracing).transpose()?,
        Arc::clone(&self.state),
      ),
      signal,
    ))
  }

  /// Reads what every scene of an animation needs fetched before it renders, asynchronously.
  #[napi(
    js_name = "animationNeeds",
    ts_args_type = "options: RenderAnimationOptions, fonts?: ListedFont[], signal?: AbortSignal",
    ts_return_type = "Promise<RenderNeeds>"
  )]
  pub fn animation_needs(
    &self,
    options: Object,
    fonts: Option<Object>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<AnimationNeedsTask>> {
    Ok(AsyncTask::with_optional_signal(
      AnimationNeedsTask::read(
        options,
        fonts.map(deserialize_with_tracing).transpose()?,
        Arc::clone(&self.state),
      )?,
      signal,
    ))
  }

  /// Renders a sequential scene animation into a buffer asynchronously.
  #[napi(
    ts_args_type = "options: RenderAnimationOptions, signal?: AbortSignal",
    ts_return_type = "Promise<Buffer<ArrayBuffer>>"
  )]
  pub fn render_animation(
    &self,
    options: Object,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<RenderAnimationTask>> {
    Ok(AsyncTask::with_optional_signal(
      RenderAnimationTask(AnimationInput::read(
        options,
        |_| {},
        Arc::clone(&self.state),
      )?),
      signal,
    ))
  }
}

impl Renderer {
  /// A render of `source` under `options`.
  fn job(&self, source: Either<String, Object>, options: Option<Object>) -> Result<RenderJob> {
    Ok(RenderJob::new(
      NodeSource::from_js(source)?,
      read_options(options)?,
      Arc::clone(&self.state),
    ))
  }
}
