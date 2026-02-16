//! Takumi C FFI bindings

#![deny(
  missing_docs,
  clippy::unwrap_used,
  clippy::expect_used,
  clippy::panic,
  clippy::all,
  clippy::redundant_closure_for_method_calls
)]

mod model;

use std::{
  borrow::Cow,
  cell::RefCell,
  ffi::{CStr, CString, c_char},
  fmt::Display,
  mem,
  panic::{AssertUnwindSafe, catch_unwind},
  ptr, slice,
  str::FromStr,
  sync::Arc,
};

use model::*;
use takumi::{
  GlobalContext,
  layout::{
    DEFAULT_DEVICE_PIXEL_RATIO, DEFAULT_FONT_SIZE, Viewport,
    node::{ContainerNode, ImageNode, Node, NodeKind, TextNode},
    style::{
      AlignItems, ColorInput, CssValue, Display as StyleDisplay, FontWeight as StyleFontWeight,
      FromCss, JustifyContent, Length, Style, TextAlign, tw::TailwindValues,
    },
  },
  parley::{FontWeight as ParleyFontWeight, fontique::FontInfoOverride},
  rendering::{
    AnimationFrame, MeasuredNode, MeasuredTextRun, RenderOptionsBuilder, encode_animated_png,
    encode_animated_webp, measure_layout, render, write_image,
  },
  resources::{image::load_image_source_from_bytes, task::FetchTaskCollection},
};

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
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

fn ffi_bool(flag: u8) -> bool {
  flag != 0
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

fn vec_to_measured_layout(
  mut nodes: Vec<TakumiMeasuredNode>,
  mut runs: Vec<TakumiMeasuredTextRun>,
) -> TakumiMeasuredLayout {
  let nodes_len = nodes.len();
  let nodes_capacity = nodes.capacity();
  let nodes_ptr = nodes.as_mut_ptr();
  mem::forget(nodes);

  let runs_len = runs.len();
  let runs_capacity = runs.capacity();
  let runs_ptr = runs.as_mut_ptr();
  mem::forget(runs);

  TakumiMeasuredLayout {
    nodes: nodes_ptr,
    nodes_len,
    nodes_capacity,
    runs: runs_ptr,
    runs_len,
    runs_capacity,
  }
}

fn vec_to_string_array(mut items: Vec<*mut c_char>) -> TakumiStringArray {
  let len = items.len();
  let capacity = items.capacity();
  let ptr = items.as_mut_ptr();
  mem::forget(items);

  TakumiStringArray {
    items: ptr,
    len,
    capacity,
  }
}

#[derive(Default)]
/// Opaque renderer handle used by the C API.
pub struct TakumiRenderer {
  context: GlobalContext,
}

fn map_ffi_font_weight(weight: Option<u16>) -> Option<ParleyFontWeight> {
  weight.and_then(|weight| (weight > 0).then_some(ParleyFontWeight::new(weight as f32)))
}

fn ensure_node_style(node: &mut TakumiNode) -> &mut Style {
  match &mut node.node {
    NodeKind::Container(inner) => inner.style.get_or_insert_with(Style::default),
    NodeKind::Text(inner) => inner.style.get_or_insert_with(Style::default),
    NodeKind::Image(inner) => inner.style.get_or_insert_with(Style::default),
  }
}

fn measured_run_to_ffi(run: &MeasuredTextRun) -> Result<TakumiMeasuredTextRun, FfiError> {
  let text = CString::new(run.text.clone())
    .map(CString::into_raw)
    .map_err(|_| {
      FfiError::new(
        TakumiStatus::InternalError,
        "text run contains interior NUL",
      )
    })?;

  Ok(TakumiMeasuredTextRun {
    text,
    x: run.x,
    y: run.y,
    width: run.width,
    height: run.height,
  })
}

fn flatten_measured_node(
  node: &MeasuredNode,
  nodes: &mut Vec<TakumiMeasuredNode>,
  runs: &mut Vec<TakumiMeasuredTextRun>,
) -> Result<(), FfiError> {
  let index = nodes.len();
  nodes.push(TakumiMeasuredNode {
    width: node.width,
    height: node.height,
    transform: node.transform,
    first_child: 0,
    child_count: 0,
    first_run: 0,
    run_count: 0,
  });

  let first_run = runs.len();
  for run in &node.runs {
    runs.push(measured_run_to_ffi(run)?);
  }
  let run_count = runs.len().saturating_sub(first_run);

  let first_child = nodes.len();
  for child in &node.children {
    flatten_measured_node(child, nodes, runs)?;
  }
  let child_count = nodes.len().saturating_sub(first_child);

  nodes[index].first_child = first_child as u32;
  nodes[index].child_count = child_count as u32;
  nodes[index].first_run = first_run as u32;
  nodes[index].run_count = run_count as u32;

  Ok(())
}

impl TakumiRenderer {
  fn new() -> Self {
    Self::default()
  }

  fn put_persistent_image_internal(&mut self, src: &str, data: &[u8]) -> Result<(), FfiError> {
    let image = load_image_source_from_bytes(data).map_err(internal_error)?;
    self
      .context
      .persistent_image_store
      .insert(src.to_owned(), image);

    Ok(())
  }

  fn render_internal_node(
    &self,
    node: NodeKind,
    options: TakumiRenderOptions,
  ) -> Result<Vec<u8>, FfiError> {
    let device_pixel_ratio = if options.device_pixel_ratio <= 0.0 {
      DEFAULT_DEVICE_PIXEL_RATIO
    } else {
      options.device_pixel_ratio
    };

    let render_options = RenderOptionsBuilder::default()
      .viewport(Viewport {
        width: (options.width > 0).then_some(options.width),
        height: (options.height > 0).then_some(options.height),
        font_size: DEFAULT_FONT_SIZE,
        device_pixel_ratio,
      })
      .draw_debug_border(ffi_bool(options.draw_debug_border))
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
    let format: OutputFormat = options.format.into();

    if format == OutputFormat::Raw {
      return Ok(image.into_raw());
    }

    let mut buffer = Vec::new();
    let quality = (options.quality > 0).then_some(options.quality);
    write_image(&image, &mut buffer, format.into(), quality).map_err(FfiError::from)?;

    Ok(buffer)
  }

  fn measure_internal_node(
    &self,
    node: NodeKind,
    options: TakumiRenderOptions,
  ) -> Result<TakumiMeasuredLayout, FfiError> {
    let device_pixel_ratio = if options.device_pixel_ratio <= 0.0 {
      DEFAULT_DEVICE_PIXEL_RATIO
    } else {
      options.device_pixel_ratio
    };

    let render_options = RenderOptionsBuilder::default()
      .viewport(Viewport {
        width: (options.width > 0).then_some(options.width),
        height: (options.height > 0).then_some(options.height),
        font_size: DEFAULT_FONT_SIZE,
        device_pixel_ratio,
      })
      .draw_debug_border(ffi_bool(options.draw_debug_border))
      .node(node)
      .global(&self.context)
      .build()
      .map_err(|error| {
        FfiError::new(
          TakumiStatus::InvalidArgument,
          format!("Failed to build render options: {error}"),
        )
      })?;

    let measured = measure_layout(render_options).map_err(FfiError::from)?;
    let mut nodes = Vec::new();
    let mut runs = Vec::new();
    flatten_measured_node(&measured, &mut nodes, &mut runs)?;
    Ok(vec_to_measured_layout(nodes, runs))
  }

  fn render_animation_internal(
    &self,
    frames: &[TakumiAnimationFrame],
    options: TakumiRenderAnimationOptions,
  ) -> Result<Vec<u8>, FfiError> {
    let rendered_frames = frames
      .iter()
      .map(|frame| -> Result<AnimationFrame, FfiError> {
        if frame.node.is_null() {
          return Err(FfiError::new(
            TakumiStatus::NullPointer,
            "frame.node must not be null",
          ));
        }

        let frame_node = unsafe { &*frame.node };
        let render_options = RenderOptionsBuilder::default()
          .viewport((options.width, options.height).into())
          .draw_debug_border(ffi_bool(options.draw_debug_border))
          .node(frame_node.node.clone())
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
    match AnimationOutputFormat::from(options.format) {
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

#[unsafe(no_mangle)]
/// Returns a pointer to the last thread-local error message.
pub extern "C" fn takumi_last_error_message() -> *const c_char {
  LAST_ERROR.with(|slot| slot.borrow().as_ptr())
}

#[unsafe(no_mangle)]
/// Initializes a [`TakumiRenderOptions`] value with defaults.
///
/// # Safety
/// `out_options` must be a valid writable pointer.
pub unsafe extern "C" fn takumi_render_options_init(out_options: *mut TakumiRenderOptions) -> i32 {
  ffi_call(|| {
    if out_options.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "out_options must not be null",
      ));
    }

    // SAFETY: pointer validity checked above.
    unsafe {
      *out_options = TakumiRenderOptions::default();
    }

    Ok(())
  })
}

#[unsafe(no_mangle)]
/// Creates a container node handle.
///
/// # Safety
/// The returned pointer must be released with [`takumi_node_free`].
pub unsafe extern "C" fn takumi_node_new_container() -> *mut TakumiNode {
  match catch_unwind(AssertUnwindSafe(|| TakumiNode {
    node: NodeKind::Container(ContainerNode {
      preset: None,
      style: None,
      children: None,
      tw: None,
    }),
  })) {
    Ok(node) => {
      clear_last_error();
      Box::into_raw(Box::new(node))
    }
    Err(_) => {
      set_last_error("A panic occurred inside takumi-c-ffi");
      ptr::null_mut()
    }
  }
}

#[unsafe(no_mangle)]
/// Creates a text node handle.
///
/// # Safety
/// `text` must be a valid UTF-8 NUL-terminated string.
/// The returned pointer must be released with [`takumi_node_free`].
pub unsafe extern "C" fn takumi_node_new_text(text: *const c_char) -> *mut TakumiNode {
  match catch_unwind(AssertUnwindSafe(|| {
    let text = unsafe { read_cstr(text, "text")? };

    Ok::<TakumiNode, FfiError>(TakumiNode {
      node: NodeKind::Text(TextNode {
        preset: None,
        style: None,
        text: text.to_owned(),
        tw: None,
      }),
    })
  })) {
    Ok(Ok(node)) => {
      clear_last_error();
      Box::into_raw(Box::new(node))
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
/// Creates an image node handle.
///
/// # Safety
/// `src` must be a valid UTF-8 NUL-terminated string.
/// The returned pointer must be released with [`takumi_node_free`].
pub unsafe extern "C" fn takumi_node_new_image(
  src: *const c_char,
  width: f32,
  has_width: u8,
  height: f32,
  has_height: u8,
) -> *mut TakumiNode {
  match catch_unwind(AssertUnwindSafe(|| {
    let src = unsafe { read_cstr(src, "src")? };

    Ok::<TakumiNode, FfiError>(TakumiNode {
      node: NodeKind::Image(ImageNode {
        preset: None,
        style: None,
        src: Arc::<str>::from(src),
        width: ffi_bool(has_width).then_some(width),
        height: ffi_bool(has_height).then_some(height),
        tw: None,
      }),
    })
  })) {
    Ok(Ok(node)) => {
      clear_last_error();
      Box::into_raw(Box::new(node))
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
/// Sets Tailwind utility classes on a node.
///
/// Pass null or empty string to clear Tailwind values.
///
/// # Safety
/// `node` must be a valid node pointer. `tw` must be null or a valid UTF-8
/// NUL-terminated string.
pub unsafe extern "C" fn takumi_node_set_tw(node: *mut TakumiNode, tw: *const c_char) -> i32 {
  ffi_call(|| {
    if node.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "node must not be null",
      ));
    }

    let parsed_tw = match unsafe { read_optional_cstr(tw)? } {
      Some(value) if !value.is_empty() => Some(
        TailwindValues::from_str(value)
          .map_err(|error| FfiError::new(TakumiStatus::InvalidArgument, error))?,
      ),
      _ => None,
    };

    // SAFETY: pointer validity checked above.
    let node = unsafe { &mut *node };
    match &mut node.node {
      NodeKind::Container(inner) => inner.tw = parsed_tw,
      NodeKind::Text(inner) => inner.tw = parsed_tw,
      NodeKind::Image(inner) => inner.tw = parsed_tw,
    }

    Ok(())
  })
}

#[unsafe(no_mangle)]
/// Sets an inline style property on a node.
///
/// Currently supported properties: `width`, `height`, `backgroundColor`, `color`,
/// `fontSize`, `fontWeight`, `display`, `justifyContent`, `alignItems`, `textAlign`.
///
/// # Safety
/// `node` must be valid. `property` and `value` must be valid UTF-8 NUL-terminated
/// strings.
pub unsafe extern "C" fn takumi_node_set_style(
  node: *mut TakumiNode,
  property: *const c_char,
  value: *const c_char,
) -> i32 {
  ffi_call(|| {
    if node.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "node must not be null",
      ));
    }

    let property = unsafe { read_cstr(property, "property")? };
    let value = unsafe { read_cstr(value, "value")? };

    let node = unsafe { &mut *node };
    let style = ensure_node_style(node);

    match property {
      "width" => {
        style.width = CssValue::Value(
          <Length as FromCss<'_>>::from_str(value)
            .map_err(|error| FfiError::new(TakumiStatus::InvalidArgument, error.to_string()))?,
        );
      }
      "height" => {
        style.height = CssValue::Value(
          <Length as FromCss<'_>>::from_str(value)
            .map_err(|error| FfiError::new(TakumiStatus::InvalidArgument, error.to_string()))?,
        );
      }
      "backgroundColor" => {
        style.background_color = CssValue::Value(Some(
          <ColorInput<false> as FromCss<'_>>::from_str(value)
            .map_err(|error| FfiError::new(TakumiStatus::InvalidArgument, error.to_string()))?,
        ));
      }
      "color" => {
        style.color = CssValue::Value(
          <ColorInput as FromCss<'_>>::from_str(value)
            .map_err(|error| FfiError::new(TakumiStatus::InvalidArgument, error.to_string()))?,
        );
      }
      "fontSize" => {
        style.font_size = CssValue::Value(Some(
          <Length as FromCss<'_>>::from_str(value)
            .map_err(|error| FfiError::new(TakumiStatus::InvalidArgument, error.to_string()))?,
        ));
      }
      "fontWeight" => {
        style.font_weight = CssValue::Value(
          <StyleFontWeight as FromCss<'_>>::from_str(value)
            .map_err(|error| FfiError::new(TakumiStatus::InvalidArgument, error.to_string()))?,
        );
      }
      "display" => {
        style.display = CssValue::Value(
          <StyleDisplay as FromCss<'_>>::from_str(value)
            .map_err(|error| FfiError::new(TakumiStatus::InvalidArgument, error.to_string()))?,
        );
      }
      "justifyContent" => {
        style.justify_content = CssValue::Value(
          <JustifyContent as FromCss<'_>>::from_str(value)
            .map_err(|error| FfiError::new(TakumiStatus::InvalidArgument, error.to_string()))?,
        );
      }
      "alignItems" => {
        style.align_items = CssValue::Value(
          <AlignItems as FromCss<'_>>::from_str(value)
            .map_err(|error| FfiError::new(TakumiStatus::InvalidArgument, error.to_string()))?,
        );
      }
      "textAlign" => {
        style.text_align = CssValue::Value(
          <TextAlign as FromCss<'_>>::from_str(value)
            .map_err(|error| FfiError::new(TakumiStatus::InvalidArgument, error.to_string()))?,
        );
      }
      other => {
        return Err(FfiError::new(
          TakumiStatus::InvalidArgument,
          format!("Unsupported style property '{other}'"),
        ));
      }
    }

    Ok(())
  })
}

#[unsafe(no_mangle)]
/// Appends `child` to `parent`.
///
/// Ownership of `child` is transferred on success.
///
/// # Safety
/// `parent` and `child` must be valid pointers created by this library.
pub unsafe extern "C" fn takumi_node_add_child(
  parent: *mut TakumiNode,
  child: *mut TakumiNode,
) -> i32 {
  ffi_call(|| {
    if parent.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "parent must not be null",
      ));
    }
    if child.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "child must not be null",
      ));
    }

    // SAFETY: pointer validity checked above.
    let parent = unsafe { &mut *parent };
    let NodeKind::Container(container) = &mut parent.node else {
      return Err(FfiError::new(
        TakumiStatus::InvalidArgument,
        "parent node must be a container",
      ));
    };

    // SAFETY: ownership transfer is documented and pointers are validated above.
    let child_node = unsafe { Box::from_raw(child) };
    let mut children = container.children.take().map(Vec::from).unwrap_or_default();
    children.push(child_node.node);
    container.children = Some(children.into_boxed_slice());

    Ok(())
  })
}

#[unsafe(no_mangle)]
/// Frees a node created by this library.
///
/// # Safety
/// `node` must be null or a pointer returned by `takumi_node_new_*` that has
/// not already been freed.
pub unsafe extern "C" fn takumi_node_free(node: *mut TakumiNode) {
  if node.is_null() {
    return;
  }

  // SAFETY: `node` is expected to originate from `Box::into_raw`.
  unsafe {
    drop(Box::from_raw(node));
  }
}

#[unsafe(no_mangle)]
/// Creates a renderer with default options.
///
/// # Safety
/// The returned pointer must be released with [`takumi_renderer_free`].
pub unsafe extern "C" fn takumi_renderer_new() -> *mut TakumiRenderer {
  match catch_unwind(AssertUnwindSafe(TakumiRenderer::new)) {
    Ok(renderer) => {
      clear_last_error();
      Box::into_raw(Box::new(renderer))
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
      Some("normal") => Some(takumi::parley::FontStyle::Normal),
      Some("italic") => Some(takumi::parley::FontStyle::Italic),
      Some("oblique") => Some(takumi::parley::FontStyle::Oblique(None)),
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
        style,
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

    // SAFETY: validated non-null above.
    let renderer = unsafe { &mut *renderer };
    renderer.put_persistent_image_internal(src, data)
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

    Ok(())
  })
}

#[unsafe(no_mangle)]
/// Renders a pre-built node handle and returns encoded bytes.
///
/// # Safety
/// `renderer` and `node` must be valid pointers. `options` may be null to use
/// defaults. `out_bytes` must be a valid writable pointer; free with
/// [`takumi_bytes_free`].
pub unsafe extern "C" fn takumi_renderer_render(
  renderer: *const TakumiRenderer,
  node: *const TakumiNode,
  options: *const TakumiRenderOptions,
  out_bytes: *mut TakumiBytes,
) -> i32 {
  ffi_call(|| {
    if renderer.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "renderer must not be null",
      ));
    }
    if node.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "node must not be null",
      ));
    }
    if out_bytes.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "out_bytes must not be null",
      ));
    }

    // SAFETY: pointers validated above.
    let renderer = unsafe { &*renderer };
    let node = unsafe { &*node };
    let options = if options.is_null() {
      TakumiRenderOptions::default()
    } else {
      // SAFETY: null-checked above.
      unsafe { *options }
    };

    let output = renderer.render_internal_node(node.node.clone(), options)?;

    // SAFETY: validated pointer above.
    unsafe {
      *out_bytes = vec_to_bytes(output);
    }

    Ok(())
  })
}

#[unsafe(no_mangle)]
/// Measures a pre-built node handle and returns a flattened layout struct.
///
/// # Safety
/// `renderer` and `node` must be valid pointers. `options` may be null to use
/// defaults. `out_layout` must be writable and later freed with
/// [`takumi_measured_layout_free`].
pub unsafe extern "C" fn takumi_renderer_measure(
  renderer: *const TakumiRenderer,
  node: *const TakumiNode,
  options: *const TakumiRenderOptions,
  out_layout: *mut TakumiMeasuredLayout,
) -> i32 {
  ffi_call(|| {
    if renderer.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "renderer must not be null",
      ));
    }
    if node.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "node must not be null",
      ));
    }
    if out_layout.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "out_layout must not be null",
      ));
    }

    let renderer = unsafe { &*renderer };
    let node = unsafe { &*node };
    let options = if options.is_null() {
      TakumiRenderOptions::default()
    } else {
      unsafe { *options }
    };

    let measured = renderer.measure_internal_node(node.node.clone(), options)?;
    unsafe {
      *out_layout = measured;
    }

    Ok(())
  })
}

#[unsafe(no_mangle)]
/// Renders animation frames and returns encoded animation bytes.
///
/// # Safety
/// `renderer` and `frames` must be valid pointers. `out_bytes` must be writable
/// and later freed with [`takumi_bytes_free`].
pub unsafe extern "C" fn takumi_renderer_render_animation(
  renderer: *const TakumiRenderer,
  frames: *const TakumiAnimationFrame,
  frame_count: usize,
  options: *const TakumiRenderAnimationOptions,
  out_bytes: *mut TakumiBytes,
) -> i32 {
  ffi_call(|| {
    if renderer.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "renderer must not be null",
      ));
    }
    if frames.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "frames must not be null",
      ));
    }
    if options.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "options must not be null",
      ));
    }
    if out_bytes.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "out_bytes must not be null",
      ));
    }

    let renderer = unsafe { &*renderer };
    let frames = unsafe { slice::from_raw_parts(frames, frame_count) };
    let options = unsafe { *options };

    let output = renderer.render_animation_internal(frames, options)?;
    unsafe {
      *out_bytes = vec_to_bytes(output);
    }

    Ok(())
  })
}

#[unsafe(no_mangle)]
/// Extracts external resource URLs from a node and returns C string array.
///
/// # Safety
/// `node` must be valid and `out_urls` must be a writable pointer.
pub unsafe extern "C" fn takumi_extract_resource_urls(
  node: *const TakumiNode,
  out_urls: *mut TakumiStringArray,
) -> i32 {
  ffi_call(|| {
    if node.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "node must not be null",
      ));
    }
    if out_urls.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "out_urls must not be null",
      ));
    }

    let node = unsafe { &*node };
    let mut collection = FetchTaskCollection::default();
    node.node.collect_fetch_tasks(&mut collection);
    node.node.collect_style_fetch_tasks(&mut collection);

    let urls = collection
      .into_inner()
      .iter()
      .map(ToString::to_string)
      .collect::<Vec<_>>();

    let mut output = Vec::with_capacity(urls.len());
    for url in urls {
      let value = CString::new(url)
        .map(CString::into_raw)
        .map_err(|_| FfiError::new(TakumiStatus::InternalError, "url contains interior NUL"))?;
      output.push(value);
    }

    unsafe {
      *out_urls = vec_to_string_array(output);
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
/// Frees a measured layout returned by this library.
///
/// # Safety
/// `layout` must originate from `takumi_renderer_measure`.
pub unsafe extern "C" fn takumi_measured_layout_free(layout: TakumiMeasuredLayout) {
  if !layout.runs.is_null() {
    let runs = unsafe { Vec::from_raw_parts(layout.runs, layout.runs_len, layout.runs_capacity) };
    for run in runs {
      if !run.text.is_null() {
        unsafe {
          drop(CString::from_raw(run.text));
        }
      }
    }
  }

  if !layout.nodes.is_null() {
    unsafe {
      drop(Vec::from_raw_parts(
        layout.nodes,
        layout.nodes_len,
        layout.nodes_capacity,
      ));
    }
  }
}

#[unsafe(no_mangle)]
/// Frees a string array returned by this library.
///
/// # Safety
/// `value` must originate from APIs returning `TakumiStringArray`.
pub unsafe extern "C" fn takumi_string_array_free(value: TakumiStringArray) {
  if value.items.is_null() {
    return;
  }

  let items = unsafe { Vec::from_raw_parts(value.items, value.len, value.capacity) };
  for item in items {
    if !item.is_null() {
      unsafe {
        drop(CString::from_raw(item));
      }
    }
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

#[unsafe(no_mangle)]
/// Initializes a `TakumiMeasuredLayout` output struct to an empty state.
///
/// # Safety
/// `out_layout` must be a valid writable pointer.
pub unsafe extern "C" fn takumi_measured_layout_init(out_layout: *mut TakumiMeasuredLayout) -> i32 {
  ffi_call(|| {
    if out_layout.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "out_layout must not be null",
      ));
    }

    unsafe {
      *out_layout = TakumiMeasuredLayout::empty();
    }

    Ok(())
  })
}

#[unsafe(no_mangle)]
/// Initializes a `TakumiStringArray` output struct to an empty state.
///
/// # Safety
/// `out_value` must be a valid writable pointer.
pub unsafe extern "C" fn takumi_string_array_init(out_value: *mut TakumiStringArray) -> i32 {
  ffi_call(|| {
    if out_value.is_null() {
      return Err(FfiError::new(
        TakumiStatus::NullPointer,
        "out_value must not be null",
      ));
    }

    unsafe {
      *out_value = TakumiStringArray::empty();
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
