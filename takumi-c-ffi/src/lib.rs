//! Takumi C FFI bindings

#![deny(
  missing_docs,
  clippy::unwrap_used,
  clippy::expect_used,
  clippy::panic,
  clippy::all,
  clippy::redundant_closure_for_method_calls
)]

use std::{
  borrow::Cow,
  cell::RefCell,
  collections::{HashMap, HashSet},
  ffi::{CStr, CString, c_char},
  fmt::Display,
  mem,
  panic::{AssertUnwindSafe, catch_unwind},
  ptr, slice,
  sync::Arc,
};

use base64::{Engine, prelude::BASE64_STANDARD};
use serde::Deserialize;
use takumi::{
  GlobalContext,
  layout::{
    DEFAULT_DEVICE_PIXEL_RATIO, DEFAULT_FONT_SIZE, Viewport,
    node::{Node, NodeKind},
  },
  parley::{FontWeight, fontique::FontInfoOverride},
  rendering::{
    AnimationFrame, ImageOutputFormat, RenderOptionsBuilder, encode_animated_png,
    encode_animated_webp, measure_layout, render, write_image,
  },
  resources::{image::load_image_source_from_bytes, task::FetchTaskCollection},
};
use xxhash_rust::xxh3::{Xxh3DefaultBuilder, xxh3_64};

#[derive(Debug, Clone, Copy)]
#[repr(i32)]
enum TakumiStatus {
  Ok = 0,
  NullPointer = 1,
  InvalidUtf8 = 2,
  InvalidJson = 3,
  InvalidArgument = 4,
  InternalError = 5,
  Panic = 6,
}

impl TakumiStatus {
  const fn code(self) -> i32 {
    self as i32
  }
}

#[allow(non_camel_case_types)]
#[repr(i32)]
/// FFI status codes returned by Takumi C APIs.
pub enum TakumiStatusCode {
  /// Success.
  TAKUMI_STATUS_OK = 0,
  /// A required pointer argument was null.
  TAKUMI_STATUS_NULL_POINTER = 1,
  /// A string argument was not valid UTF-8.
  TAKUMI_STATUS_INVALID_UTF8 = 2,
  /// A JSON payload failed to deserialize.
  TAKUMI_STATUS_INVALID_JSON = 3,
  /// The provided arguments were invalid.
  TAKUMI_STATUS_INVALID_ARGUMENT = 4,
  /// An internal renderer error occurred.
  TAKUMI_STATUS_INTERNAL_ERROR = 5,
  /// A panic occurred inside the FFI boundary.
  TAKUMI_STATUS_PANIC = 6,
}

#[derive(Debug)]
struct FfiError {
  status: TakumiStatus,
  message: String,
}

impl FfiError {
  fn new(status: TakumiStatus, message: impl Into<String>) -> Self {
    Self {
      status,
      message: message.into(),
    }
  }
}

impl From<takumi::Error> for FfiError {
  fn from(value: takumi::Error) -> Self {
    Self::new(TakumiStatus::InternalError, value.to_string())
  }
}

impl From<serde_json::Error> for FfiError {
  fn from(value: serde_json::Error) -> Self {
    Self::new(TakumiStatus::InvalidJson, value.to_string())
  }
}

fn internal_error(error: impl Display) -> FfiError {
  FfiError::new(TakumiStatus::InternalError, error.to_string())
}

fn default_error_message() -> CString {
  // SAFETY: The byte vector is a valid NUL-terminated empty string.
  unsafe { CString::from_vec_with_nul_unchecked(vec![0]) }
}

thread_local! {
  static LAST_ERROR: RefCell<CString> = RefCell::new(default_error_message());
}

fn set_last_error(message: impl AsRef<str>) {
  let sanitized = message.as_ref().replace('\0', " ");
  let value = match CString::new(sanitized) {
    Ok(value) => value,
    Err(_) => default_error_message(),
  };

  LAST_ERROR.with(|slot| {
    *slot.borrow_mut() = value;
  });
}

fn clear_last_error() {
  LAST_ERROR.with(|slot| {
    *slot.borrow_mut() = default_error_message();
  });
}

fn ffi_call<F>(f: F) -> i32
where
  F: FnOnce() -> Result<(), FfiError>,
{
  match catch_unwind(AssertUnwindSafe(f)) {
    Ok(Ok(())) => {
      clear_last_error();
      TakumiStatus::Ok.code()
    }
    Ok(Err(error)) => {
      set_last_error(error.message);
      error.status.code()
    }
    Err(_) => {
      set_last_error("A panic occurred inside takumi-c-ffi");
      TakumiStatus::Panic.code()
    }
  }
}

#[repr(C)]
/// Owned byte buffer returned by Takumi FFI.
pub struct TakumiBytes {
  /// Pointer to the allocated byte data.
  pub data: *mut u8,
  /// Number of initialized bytes.
  pub len: usize,
  /// Allocation capacity for `data`.
  pub capacity: usize,
}

impl TakumiBytes {
  const fn empty() -> Self {
    Self {
      data: ptr::null_mut(),
      len: 0,
      capacity: 0,
    }
  }
}

fn vec_to_bytes(mut value: Vec<u8>) -> TakumiBytes {
  let len = value.len();
  let capacity = value.capacity();
  let data = value.as_mut_ptr();

  mem::forget(value);

  TakumiBytes {
    data,
    len,
    capacity,
  }
}

#[derive(PartialEq, Eq, Hash)]
struct ImageCacheKey {
  src: Box<str>,
  data_hash: u64,
}

#[derive(Default)]
/// Opaque renderer handle used by the C API.
pub struct TakumiRenderer {
  context: GlobalContext,
  persistent_image_cache: HashSet<ImageCacheKey, Xxh3DefaultBuilder>,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum OutputFormat {
  #[serde(alias = "Png")]
  Png,
  #[serde(alias = "Jpeg")]
  Jpeg,
  #[serde(alias = "WebP")]
  WebP,
  Raw,
}

impl From<OutputFormat> for ImageOutputFormat {
  fn from(format: OutputFormat) -> Self {
    match format {
      OutputFormat::Png => ImageOutputFormat::Png,
      OutputFormat::Jpeg => ImageOutputFormat::Jpeg,
      OutputFormat::WebP => ImageOutputFormat::WebP,
      OutputFormat::Raw => unreachable!("Raw format is handled separately"),
    }
  }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum AnimationOutputFormat {
  #[serde(alias = "apng")]
  APng,
  #[serde(alias = "webp")]
  WebP,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum FontStyle {
  Normal,
  Italic,
  Oblique,
}

impl From<FontStyle> for takumi::parley::FontStyle {
  fn from(value: FontStyle) -> Self {
    match value {
      FontStyle::Normal => Self::Normal,
      FontStyle::Italic => Self::Italic,
      FontStyle::Oblique => Self::Oblique(None),
    }
  }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ByteInput {
  Base64(String),
  Array(Vec<u8>),
}

#[derive(Clone)]
struct Bytes(Vec<u8>);

impl<'de> Deserialize<'de> for Bytes {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let source = ByteInput::deserialize(deserializer)?;

    let value = match source {
      ByteInput::Base64(text) => BASE64_STANDARD
        .decode(text)
        .map_err(serde::de::Error::custom)?,
      ByteInput::Array(values) => values,
    };

    Ok(Self(value))
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageSourceInput {
  src: Arc<str>,
  data: Bytes,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FontDetailsInput {
  name: Option<String>,
  data: Bytes,
  weight: Option<u16>,
  style: Option<FontStyle>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum FontInput {
  Object(FontDetailsInput),
  Buffer(Bytes),
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConstructRendererOptions {
  fonts: Option<Vec<FontInput>>,
  persistent_images: Option<Vec<ImageSourceInput>>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderOptionsInput {
  width: Option<u32>,
  height: Option<u32>,
  format: Option<OutputFormat>,
  quality: Option<u8>,
  fetched_resources: Option<Vec<ImageSourceInput>>,
  draw_debug_border: Option<bool>,
  device_pixel_ratio: Option<f32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderAnimationOptionsInput {
  width: u32,
  height: u32,
  format: Option<AnimationOutputFormat>,
  draw_debug_border: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnimationFrameSourceInput {
  node: NodeKind,
  duration_ms: u32,
}

fn map_ffi_font_weight(weight: Option<u16>) -> Option<FontWeight> {
  weight.and_then(|weight| (weight > 0).then_some(FontWeight::new(weight as f32)))
}

impl TakumiRenderer {
  fn new(options: Option<ConstructRendererOptions>) -> Result<Self, FfiError> {
    let mut renderer = Self::default();
    let options = options.unwrap_or_default();

    if let Some(fonts) = options.fonts {
      for font in fonts {
        renderer.load_font_input(font)?;
      }
    }

    if let Some(images) = options.persistent_images {
      for image in images {
        renderer.put_persistent_image_internal(&image)?;
      }
    }

    Ok(renderer)
  }

  fn load_font_input(&mut self, input: FontInput) -> Result<(), FfiError> {
    match input {
      FontInput::Buffer(buffer) => {
        self
          .context
          .font_context
          .load_and_store(Cow::Owned(buffer.0), None, None)
          .map_err(internal_error)?;
      }
      FontInput::Object(details) => {
        self
          .context
          .font_context
          .load_and_store(
            Cow::Owned(details.data.0),
            Some(FontInfoOverride {
              family_name: details.name.as_deref(),
              style: details.style.map(Into::into),
              weight: map_ffi_font_weight(details.weight),
              axes: None,
              width: None,
            }),
            None,
          )
          .map_err(internal_error)?;
      }
    }

    Ok(())
  }

  fn put_persistent_image_internal(&mut self, source: &ImageSourceInput) -> Result<(), FfiError> {
    let key = ImageCacheKey {
      src: source.src.as_ref().into(),
      data_hash: xxh3_64(&source.data.0),
    };

    if self.persistent_image_cache.contains(&key) {
      return Ok(());
    }

    let image = load_image_source_from_bytes(&source.data.0).map_err(internal_error)?;
    self
      .context
      .persistent_image_store
      .insert(source.src.to_string(), image);
    self.persistent_image_cache.insert(key);

    Ok(())
  }

  fn render_internal(
    &self,
    node: NodeKind,
    options: RenderOptionsInput,
  ) -> Result<Vec<u8>, FfiError> {
    let fetched_resources = options
      .fetched_resources
      .map(|resources| -> Result<HashMap<Arc<str>, _>, FfiError> {
        resources
          .into_iter()
          .map(|source| {
            let image = load_image_source_from_bytes(&source.data.0).map_err(internal_error)?;
            Ok((source.src, image))
          })
          .collect()
      })
      .transpose()?
      .unwrap_or_default();

    let render_options = RenderOptionsBuilder::default()
      .viewport(Viewport {
        width: options.width,
        height: options.height,
        font_size: DEFAULT_FONT_SIZE,
        device_pixel_ratio: options
          .device_pixel_ratio
          .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
      })
      .draw_debug_border(options.draw_debug_border.unwrap_or_default())
      .fetched_resources(fetched_resources)
      .node(node)
      .global(&self.context)
      .build()
      .map_err(|error| {
        FfiError::new(
          TakumiStatus::InvalidArgument,
          format!("Failed to build render options: {error}"),
        )
      })?;

    let image = render(render_options).map_err(FfiError::from)?;
    let format = options.format.unwrap_or(OutputFormat::Png);

    if format == OutputFormat::Raw {
      return Ok(image.into_raw());
    }

    let mut buffer = Vec::new();
    write_image(&image, &mut buffer, format.into(), options.quality).map_err(FfiError::from)?;

    Ok(buffer)
  }

  fn measure_internal(
    &self,
    node: NodeKind,
    options: RenderOptionsInput,
  ) -> Result<String, FfiError> {
    let fetched_resources = options
      .fetched_resources
      .map(|resources| -> Result<HashMap<Arc<str>, _>, FfiError> {
        resources
          .into_iter()
          .map(|source| {
            let image = load_image_source_from_bytes(&source.data.0).map_err(internal_error)?;
            Ok((source.src, image))
          })
          .collect()
      })
      .transpose()?
      .unwrap_or_default();

    let render_options = RenderOptionsBuilder::default()
      .viewport(Viewport {
        width: options.width,
        height: options.height,
        font_size: DEFAULT_FONT_SIZE,
        device_pixel_ratio: options
          .device_pixel_ratio
          .unwrap_or(DEFAULT_DEVICE_PIXEL_RATIO),
      })
      .draw_debug_border(options.draw_debug_border.unwrap_or_default())
      .fetched_resources(fetched_resources)
      .node(node)
      .global(&self.context)
      .build()
      .map_err(|error| {
        FfiError::new(
          TakumiStatus::InvalidArgument,
          format!("Failed to build render options: {error}"),
        )
      })?;

    let layout = measure_layout(render_options).map_err(FfiError::from)?;
    serde_json::to_string(&layout)
      .map_err(|error| FfiError::new(TakumiStatus::InternalError, error.to_string()))
  }

  fn render_animation_internal(
    &self,
    frames: Vec<AnimationFrameSourceInput>,
    options: RenderAnimationOptionsInput,
  ) -> Result<Vec<u8>, FfiError> {
    let rendered_frames = frames
      .into_iter()
      .map(|frame| -> Result<AnimationFrame, FfiError> {
        let render_options = RenderOptionsBuilder::default()
          .viewport((options.width, options.height).into())
          .draw_debug_border(options.draw_debug_border.unwrap_or_default())
          .node(frame.node)
          .global(&self.context)
          .build()
          .map_err(|error| {
            FfiError::new(
              TakumiStatus::InvalidArgument,
              format!("Failed to build render options: {error}"),
            )
          })?;

        let image = render(render_options).map_err(FfiError::from)?;

        Ok(AnimationFrame::new(image, frame.duration_ms))
      })
      .collect::<Result<Vec<_>, _>>()?;

    let mut buffer = Vec::new();

    match options.format.unwrap_or(AnimationOutputFormat::WebP) {
      AnimationOutputFormat::WebP => {
        encode_animated_webp(&rendered_frames, &mut buffer, true, false, None)
          .map_err(FfiError::from)?;
      }
      AnimationOutputFormat::APng => {
        encode_animated_png(&rendered_frames, &mut buffer, None).map_err(FfiError::from)?;
      }
    }

    Ok(buffer)
  }
}

unsafe fn as_required_bytes<'a>(ptr: *const u8, len: usize) -> Result<&'a [u8], FfiError> {
  if ptr.is_null() {
    return Err(FfiError::new(
      TakumiStatus::NullPointer,
      "required pointer is null",
    ));
  }

  Ok(unsafe { slice::from_raw_parts(ptr, len) })
}

unsafe fn as_optional_bytes<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
  if ptr.is_null() || len == 0 {
    return None;
  }

  Some(unsafe { slice::from_raw_parts(ptr, len) })
}

unsafe fn parse_required_json<T: for<'de> Deserialize<'de>>(
  ptr: *const u8,
  len: usize,
) -> Result<T, FfiError> {
  let bytes = unsafe { as_required_bytes(ptr, len)? };

  serde_json::from_slice(bytes).map_err(FfiError::from)
}

unsafe fn parse_optional_json<T: for<'de> Deserialize<'de> + Default>(
  ptr: *const u8,
  len: usize,
) -> Result<T, FfiError> {
  match unsafe { as_optional_bytes(ptr, len) } {
    Some(bytes) => serde_json::from_slice(bytes).map_err(FfiError::from),
    None => Ok(T::default()),
  }
}

unsafe fn read_cstr<'a>(ptr: *const c_char, name: &str) -> Result<&'a str, FfiError> {
  if ptr.is_null() {
    return Err(FfiError::new(
      TakumiStatus::NullPointer,
      format!("{name} must not be null"),
    ));
  }

  let text = unsafe { CStr::from_ptr(ptr) };
  text
    .to_str()
    .map_err(|error| FfiError::new(TakumiStatus::InvalidUtf8, error.to_string()))
}

unsafe fn read_optional_cstr<'a>(ptr: *const c_char) -> Result<Option<&'a str>, FfiError> {
  if ptr.is_null() {
    return Ok(None);
  }

  let text = unsafe { CStr::from_ptr(ptr) };
  text
    .to_str()
    .map(Some)
    .map_err(|error| FfiError::new(TakumiStatus::InvalidUtf8, error.to_string()))
}

fn encode_c_string(value: String) -> Result<*mut c_char, FfiError> {
  CString::new(value)
    .map(CString::into_raw)
    .map_err(|_| FfiError::new(TakumiStatus::InternalError, "string contains interior NUL"))
}

#[unsafe(no_mangle)]
/// Returns a pointer to the last thread-local error message.
pub extern "C" fn takumi_last_error_message() -> *const c_char {
  LAST_ERROR.with(|slot| slot.borrow().as_ptr())
}

#[unsafe(no_mangle)]
/// Creates a renderer with default options.
///
/// # Safety
/// The returned pointer must be released with [`takumi_renderer_free`].
pub unsafe extern "C" fn takumi_renderer_new() -> *mut TakumiRenderer {
  match catch_unwind(AssertUnwindSafe(|| TakumiRenderer::new(None))) {
    Ok(Ok(renderer)) => {
      clear_last_error();
      Box::into_raw(Box::new(renderer))
    }
    Ok(Err(error)) => {
      set_last_error(error.message);
      ptr::null_mut()
    }
    Err(_) => {
      set_last_error("A panic occurred inside takumi-c-ffi");
      ptr::null_mut()
    }
  }
}

#[unsafe(no_mangle)]
/// Creates a renderer with optional JSON options payload.
///
/// # Safety
/// `options_json` must point to `options_json_len` readable bytes when non-null.
/// The returned pointer must be released with [`takumi_renderer_free`].
pub unsafe extern "C" fn takumi_renderer_new_with_options(
  options_json: *const u8,
  options_json_len: usize,
) -> *mut TakumiRenderer {
  match catch_unwind(AssertUnwindSafe(|| {
    let options: ConstructRendererOptions =
      unsafe { parse_optional_json(options_json, options_json_len)? };
    TakumiRenderer::new(Some(options))
  })) {
    Ok(Ok(renderer)) => {
      clear_last_error();
      Box::into_raw(Box::new(renderer))
    }
    Ok(Err(error)) => {
      set_last_error(error.message);
      ptr::null_mut()
    }
    Err(_) => {
      set_last_error("A panic occurred inside takumi-c-ffi");
      ptr::null_mut()
    }
  }
}

#[unsafe(no_mangle)]
/// Frees a renderer previously created by this library.
///
/// # Safety
/// `renderer` must be null or a pointer returned by `takumi_renderer_new*` that has
/// not already been freed.
pub unsafe extern "C" fn takumi_renderer_free(renderer: *mut TakumiRenderer) {
  if renderer.is_null() {
    return;
  }

  // SAFETY: `renderer` is expected to originate from `Box::into_raw`.
  unsafe {
    drop(Box::from_raw(renderer));
  }
}

#[unsafe(no_mangle)]
/// Loads a font into a renderer.
///
/// # Safety
/// `renderer` must be a valid renderer pointer. `font_data` must point to
/// `font_data_len` readable bytes. Optional C strings must be valid UTF-8 and
/// NUL-terminated when non-null.
pub unsafe extern "C" fn takumi_renderer_load_font(
  renderer: *mut TakumiRenderer,
  font_data: *const u8,
  font_data_len: usize,
  family_name: *const c_char,
  style: *const c_char,
  weight: u16,
) -> i32 {
  ffi_call(|| {
    if renderer.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "renderer must not be null",
      ));
    }

    let bytes = unsafe { as_required_bytes(font_data, font_data_len)? };
    let family_name = unsafe { read_optional_cstr(family_name)? };
    let style = unsafe { read_optional_cstr(style)? };

    let style = match style {
      Some("normal") => Some(FontStyle::Normal),
      Some("italic") => Some(FontStyle::Italic),
      Some("oblique") => Some(FontStyle::Oblique),
      Some(other) => {
        return Err(FfiError::new(
          TakumiStatus::InvalidArgument,
          format!("Unsupported font style '{other}'"),
        ));
      }
      None => None,
    };

    let override_info = if family_name.is_none() && style.is_none() && weight == 0 {
      None
    } else {
      Some(FontInfoOverride {
        family_name,
        style: style.map(Into::into),
        weight: map_ffi_font_weight(Some(weight)),
        axes: None,
        width: None,
      })
    };

    // SAFETY: validated non-null above.
    let renderer = unsafe { &mut *renderer };
    renderer
      .context
      .font_context
      .load_and_store(Cow::Owned(bytes.to_vec()), override_info, None)
      .map_err(internal_error)?;

    Ok(())
  })
}

#[unsafe(no_mangle)]
/// Inserts a persistent image resource into the renderer.
///
/// # Safety
/// `renderer` must be valid. `src` must be a valid NUL-terminated UTF-8 string.
/// `image_data` must point to `image_data_len` readable bytes.
pub unsafe extern "C" fn takumi_renderer_put_persistent_image(
  renderer: *mut TakumiRenderer,
  src: *const c_char,
  image_data: *const u8,
  image_data_len: usize,
) -> i32 {
  ffi_call(|| {
    if renderer.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "renderer must not be null",
      ));
    }

    let src = unsafe { read_cstr(src, "src")? };
    let data = unsafe { as_required_bytes(image_data, image_data_len)? };

    let source = ImageSourceInput {
      src: Arc::<str>::from(src),
      data: Bytes(data.to_vec()),
    };

    // SAFETY: validated non-null above.
    let renderer = unsafe { &mut *renderer };
    renderer.put_persistent_image_internal(&source)
  })
}

#[unsafe(no_mangle)]
/// Clears the renderer persistent image store.
///
/// # Safety
/// `renderer` must be a valid renderer pointer.
pub unsafe extern "C" fn takumi_renderer_clear_image_store(renderer: *mut TakumiRenderer) -> i32 {
  ffi_call(|| {
    if renderer.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "renderer must not be null",
      ));
    }

    // SAFETY: validated non-null above.
    let renderer = unsafe { &mut *renderer };
    renderer.context.persistent_image_store.clear();
    renderer.persistent_image_cache.clear();

    Ok(())
  })
}

#[unsafe(no_mangle)]
/// Renders a node JSON payload and returns encoded bytes.
///
/// # Safety
/// `renderer` must be valid. `node_json` must point to `node_json_len` readable bytes.
/// `options_json` may be null; otherwise it must point to `options_json_len` readable bytes.
/// `out_bytes` must be a valid writable pointer; free with [`takumi_bytes_free`].
pub unsafe extern "C" fn takumi_renderer_render(
  renderer: *const TakumiRenderer,
  node_json: *const u8,
  node_json_len: usize,
  options_json: *const u8,
  options_json_len: usize,
  out_bytes: *mut TakumiBytes,
) -> i32 {
  ffi_call(|| {
    if renderer.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "renderer must not be null",
      ));
    }
    if out_bytes.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "out_bytes must not be null",
      ));
    }

    let node: NodeKind = unsafe { parse_required_json(node_json, node_json_len)? };
    let options: RenderOptionsInput =
      unsafe { parse_optional_json(options_json, options_json_len)? };

    // SAFETY: validated pointers above.
    let renderer = unsafe { &*renderer };
    let output = renderer.render_internal(node, options)?;

    // SAFETY: validated pointer above.
    unsafe {
      *out_bytes = vec_to_bytes(output);
    }

    Ok(())
  })
}

#[unsafe(no_mangle)]
/// Measures a node JSON payload and returns layout JSON.
///
/// # Safety
/// `renderer` must be valid. `node_json` must point to `node_json_len` readable bytes.
/// `options_json` may be null; otherwise it must point to `options_json_len` readable bytes.
/// `out_json` must be a valid writable pointer; free with [`takumi_string_free`].
pub unsafe extern "C" fn takumi_renderer_measure(
  renderer: *const TakumiRenderer,
  node_json: *const u8,
  node_json_len: usize,
  options_json: *const u8,
  options_json_len: usize,
  out_json: *mut *mut c_char,
) -> i32 {
  ffi_call(|| {
    if renderer.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "renderer must not be null",
      ));
    }
    if out_json.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "out_json must not be null",
      ));
    }

    let node: NodeKind = unsafe { parse_required_json(node_json, node_json_len)? };
    let options: RenderOptionsInput =
      unsafe { parse_optional_json(options_json, options_json_len)? };

    // SAFETY: validated pointer above.
    let renderer = unsafe { &*renderer };
    let output = renderer.measure_internal(node, options)?;

    let ptr = encode_c_string(output)?;

    // SAFETY: validated pointer above.
    unsafe {
      *out_json = ptr;
    }

    Ok(())
  })
}

#[unsafe(no_mangle)]
/// Renders animation frames and returns encoded animation bytes.
///
/// # Safety
/// `renderer` must be valid. `frames_json` and `options_json` must point to readable
/// byte ranges with the given lengths. `out_bytes` must be writable and later freed
/// with [`takumi_bytes_free`].
pub unsafe extern "C" fn takumi_renderer_render_animation(
  renderer: *const TakumiRenderer,
  frames_json: *const u8,
  frames_json_len: usize,
  options_json: *const u8,
  options_json_len: usize,
  out_bytes: *mut TakumiBytes,
) -> i32 {
  ffi_call(|| {
    if renderer.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "renderer must not be null",
      ));
    }
    if out_bytes.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "out_bytes must not be null",
      ));
    }

    let frames: Vec<AnimationFrameSourceInput> =
      unsafe { parse_required_json(frames_json, frames_json_len)? };
    let options: RenderAnimationOptionsInput =
      unsafe { parse_required_json(options_json, options_json_len)? };

    // SAFETY: validated pointer above.
    let renderer = unsafe { &*renderer };
    let output = renderer.render_animation_internal(frames, options)?;

    // SAFETY: validated pointer above.
    unsafe {
      *out_bytes = vec_to_bytes(output);
    }

    Ok(())
  })
}

#[unsafe(no_mangle)]
/// Extracts external resource URLs from a node JSON payload.
///
/// # Safety
/// `node_json` must point to `node_json_len` readable bytes and `out_json` must be
/// a valid writable pointer; free with [`takumi_string_free`].
pub unsafe extern "C" fn takumi_extract_resource_urls(
  node_json: *const u8,
  node_json_len: usize,
  out_json: *mut *mut c_char,
) -> i32 {
  ffi_call(|| {
    if out_json.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "out_json must not be null",
      ));
    }

    let node: NodeKind = unsafe { parse_required_json(node_json, node_json_len)? };

    let mut collection = FetchTaskCollection::default();
    node.collect_fetch_tasks(&mut collection);
    node.collect_style_fetch_tasks(&mut collection);

    let urls = collection
      .into_inner()
      .iter()
      .map(ToString::to_string)
      .collect::<Vec<_>>();

    let payload = serde_json::to_string(&urls)
      .map_err(|error| FfiError::new(TakumiStatus::InternalError, error.to_string()))?;

    let ptr = encode_c_string(payload)?;

    // SAFETY: validated pointer above.
    unsafe {
      *out_json = ptr;
    }

    Ok(())
  })
}

#[unsafe(no_mangle)]
/// Frees bytes returned by this library.
///
/// # Safety
/// `bytes` must originate from Takumi APIs that return `TakumiBytes` and must not
/// be freed more than once.
pub unsafe extern "C" fn takumi_bytes_free(bytes: TakumiBytes) {
  if bytes.data.is_null() {
    return;
  }

  // SAFETY: Memory originates from `vec_to_bytes`.
  unsafe {
    drop(Vec::from_raw_parts(bytes.data, bytes.len, bytes.capacity));
  }
}

#[unsafe(no_mangle)]
/// Frees a C string returned by this library.
///
/// # Safety
/// `value` must be null or a pointer returned by Takumi that has not been freed.
pub unsafe extern "C" fn takumi_string_free(value: *mut c_char) {
  if value.is_null() {
    return;
  }

  // SAFETY: Memory originates from `CString::into_raw`.
  unsafe {
    drop(CString::from_raw(value));
  }
}

#[unsafe(no_mangle)]
/// Initializes a `TakumiBytes` output struct to an empty state.
///
/// # Safety
/// `out_bytes` must be a valid writable pointer.
pub unsafe extern "C" fn takumi_bytes_init(out_bytes: *mut TakumiBytes) -> i32 {
  ffi_call(|| {
    if out_bytes.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "out_bytes must not be null",
      ));
    }

    // SAFETY: pointer validity checked above.
    unsafe {
      *out_bytes = TakumiBytes::empty();
    }

    Ok(())
  })
}

#[cfg(test)]
mod tests {
  use super::map_ffi_font_weight;

  #[test]
  fn c_api_weight_zero_means_no_override() {
    assert!(map_ffi_font_weight(None).is_none());
    assert!(map_ffi_font_weight(Some(0)).is_none());
    assert!(map_ffi_font_weight(Some(400)).is_some());
  }
}
