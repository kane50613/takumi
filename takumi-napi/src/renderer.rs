use std::sync::Arc;

use arc_swap::ArcSwap;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use takumi_bindings_common::{
  default_fonts,
  input::{ListedFont, ListedFonts},
  registrations::{FontRegistrations, RenderLeases, Retention},
  renderer::{CacheOptions, CacheStats, RendererOptions},
  source_cache::SourceCache,
};
use takumi_core::{Cache, Fonts};

use crate::{
  JsBytes, NodeSource, RegisteredFamily, deserialize_with_tracing,
  load_font_file_task::LoadFontFileTask,
  load_font_task::LoadFontTask,
  map_error,
  measure_task::MeasureTask,
  needs_task::{AnimationNeedsTask, NeedsTask},
  read_font, read_options,
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
  /// The `@font-face` file URLs the render loads, sorted.
  pub font_files: Vec<String>,
  /// The URLs of the loaded `@font-face` files the render draws with, sorted.
  pub loaded_font_files: Vec<String>,
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
      font_files: needs.font_files.into_iter().collect(),
      loaded_font_files: needs.loaded_font_files.into_iter().collect(),
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
  registrations: FontRegistrations,
}

pub(crate) struct RendererState {
  /// Read without waiting through `font_state.load()`, so renders never block. Every change runs
  /// on the JS thread that owns the renderer, so changes never race, and renders in flight keep
  /// the state they started with.
  font_state: ArcSwap<FontState>,
  pub(crate) cache: Cache,
  pub(crate) sources: SourceCache,
  /// The leases each render in flight holds on the font files it uses.
  pub(crate) leases: RenderLeases,
}

impl RendererState {
  /// Applies `change` to a copy of the fonts and publishes it. Call it on the JS thread only.
  pub(crate) fn update_fonts<T>(
    &self,
    change: impl FnOnce(&mut Fonts, &mut FontRegistrations) -> Result<T>,
  ) -> Result<T> {
    let mut state = FontState::clone(&self.font_state.load());
    let changed = change(Arc::make_mut(&mut state.fonts), &mut state.registrations)?;

    self.font_state.store(Arc::new(state));

    Ok(changed)
  }

  /// Unregisters what the font files the cache evicted held. Call it on the JS thread only.
  pub(crate) fn release_evicted(&self) {
    let evicted = self.cache.fonts().take_evicted();

    if evicted.is_empty() {
      return;
    }

    let _ = self.update_fonts(|fonts, registrations| {
      registrations.release(fonts, &evicted);

      Ok(())
    });
  }

  /// A render's `fonts` list read against the fonts this renderer holds.
  pub(crate) fn listed(
    &self,
    families: &mut Option<Vec<String>>,
    listed: Option<Vec<ListedFont>>,
  ) -> Option<ListedFonts> {
    self.release_evicted();

    let listed = ListedFonts::read(&self.font_state.load().registrations, listed?);

    if families.is_none() {
      families.clone_from(&listed.families);
    }

    Some(listed)
  }

  /// The fonts for a render, with the `@font-face` files its document `failed` to download.
  pub(crate) fn fonts_for_render(&self, failed: Option<&[String]>) -> Arc<Fonts> {
    self.release_evicted();

    let state = self.font_state.load();

    match failed {
      Some(failed) if !failed.is_empty() => Arc::new(state.fonts.with_failed_font_files(failed)),
      _ => Arc::clone(&state.fonts),
    }
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
          registrations: FontRegistrations::default(),
        }),
        cache: options.cache(),
        sources: SourceCache::default(),
        leases: RenderLeases::default(),
      }),
    })
  }

  /// Register font into the renderer, under `key` when its loader gives one, returning the
  /// families produced. It is never evicted.
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
    self.load_font(env, font, key, Retention::Pinned, None, signal)
  }

  /// Registers a font a render's `fonts` list loaded under `key`, its file leased to `lease` and
  /// evicted once no lease holds it and the cache needs the room.
  #[napi(
    js_name = "cacheFont",
    ts_args_type = "fonts: Font, key: string, lease?: number | null, signal?: AbortSignal",
    ts_return_type = "Promise<RegisteredFamily[]>"
  )]
  pub fn cache_font(
    &self,
    env: Env,
    font: Object,
    key: String,
    lease: Option<u32>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<LoadFontTask>> {
    self.load_font(env, font, Some(key), Retention::Evictable, lease, signal)
  }

  /// The families a registration under `key` added, while the renderer holds its file. Leases the
  /// file to `lease`, or with `pin` keeps it from eviction for good.
  #[napi(js_name = "cachedFonts")]
  pub fn cached_fonts(
    &self,
    key: String,
    pin: bool,
    lease: Option<u32>,
  ) -> Result<Option<Vec<RegisteredFamily>>> {
    self.state.release_evicted();

    let registered = if pin {
      self
        .state
        .update_fonts(|_, registrations| Ok(registrations.pin(&self.state.cache, &key)))?
    } else {
      self
        .state
        .font_state
        .load()
        .registrations
        .lease(&self.state.cache, &key)
        .map(|(families, held)| {
          self.state.leases.hold(lease, held);

          families
        })
    };

    Ok(registered.map(|registered| registered.into_iter().map(Into::into).collect()))
  }

  /// Loads the `@font-face` file `url` names, as `needs` reported it, leased to `lease` and evicted
  /// once no lease holds it and the cache needs the room.
  #[napi(
    js_name = "cacheFontFile",
    ts_args_type = "url: string, data: Uint8Array | ArrayBuffer, lease?: number | null, signal?: AbortSignal",
    ts_return_type = "Promise<void>"
  )]
  pub fn cache_font_file(
    &self,
    env: Env,
    url: String,
    data: Object,
    lease: Option<u32>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<LoadFontFileTask>> {
    Ok(AsyncTask::with_optional_signal(
      LoadFontFileTask {
        state: Arc::clone(&self.state),
        url,
        buffer: JsBytes::from_object(env, data)?,
        lease,
      },
      signal,
    ))
  }

  /// Whether the renderer still holds the `@font-face` file `url`, leasing it to `lease` when it
  /// does.
  #[napi(js_name = "cachedFontFile")]
  pub fn cached_font_file(&self, url: String, lease: u32) -> bool {
    self.state.release_evicted();

    let Some(held) = self
      .state
      .font_state
      .load()
      .registrations
      .lease_file(&self.state.cache, &url)
    else {
      return false;
    };

    self.state.leases.hold(Some(lease), held);

    true
  }

  /// Opens a set of leases that keeps the font files a render uses from eviction until
  /// `releaseFonts`.
  #[napi(js_name = "leaseFonts")]
  pub fn lease_fonts(&self) -> u32 {
    self.state.leases.open()
  }

  /// Releases the leases of the set `lease` names, evicting what no longer fits.
  #[napi(js_name = "releaseFonts")]
  pub fn release_fonts(&self, lease: u32) {
    self.state.leases.close(lease);
    self.state.release_evicted();
  }

  /// Gives the faces registered under `keys` precedence in their families, a later key over an
  /// earlier one, as a render's `fonts` list orders them.
  #[napi(js_name = "orderFonts")]
  pub fn order_fonts(&self, keys: Vec<String>) {
    let state = self.state.font_state.load();
    let mut fonts = Fonts::clone(&state.fonts);

    if state.registrations.prefer(&mut fonts, &keys) {
      self.state.font_state.store(Arc::new(FontState {
        fonts: Arc::new(fonts),
        registrations: state.registrations.clone(),
      }));
    }
  }

  fn load_font(
    &self,
    env: Env,
    font: Object,
    key: Option<String>,
    retention: Retention,
    lease: Option<u32>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<LoadFontTask>> {
    let resource = read_font(env, font, &self.state.font_state.load().fonts)?;

    Ok(AsyncTask::with_optional_signal(
      LoadFontTask {
        state: Arc::clone(&self.state),
        resource: Some(resource),
        key,
        retention,
        lease,
      },
      signal,
    ))
  }

  /// Applies `options` to what the renderer keeps now, as if it were created with them. A smaller
  /// size or budget releases what no longer fits right away.
  #[napi(js_name = "setCacheOptions", ts_args_type = "options: CacheOptions")]
  pub fn set_cache_options(&self, options: Object) -> Result<()> {
    let options: CacheOptions = deserialize_with_tracing(options)?;

    self.state.cache.set_options(options.pool());
    self.state.release_evicted();

    Ok(())
  }

  /// What the renderer's caches hold and how their lookups went since it was created.
  #[napi(js_name = "cacheStats", ts_return_type = "CacheStats")]
  pub fn cache_stats<'env>(&self, env: &'env Env) -> Result<Unknown<'env>> {
    env.to_js_value(&CacheStats::new(&self.state.cache))
  }

  /// Removes every face registered under `family`, each `unicodeRange` subset alike, returning
  /// whether it had any.
  #[napi(js_name = "unregisterFont")]
  pub fn unregister_font(&self, family: String) -> Result<bool> {
    self.state.update_fonts(|fonts, registrations| {
      registrations.forget_family(&self.state.cache, &family);

      Ok(fonts.unregister_family(&family))
    })
  }

  /// Removes every registered font, leaving the bundled fallback font.
  #[napi(js_name = "clearFonts")]
  pub fn clear_fonts(&self) -> Result<()> {
    self.state.update_fonts(|fonts, registrations| {
      *fonts = default_fonts().map_err(map_error)?;
      registrations.clear(&self.state.cache);

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
