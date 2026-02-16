use std::{ffi::c_char, ptr};

use takumi::{
  layout::{DEFAULT_DEVICE_PIXEL_RATIO, node::NodeKind},
  rendering::ImageOutputFormat,
};

#[allow(non_camel_case_types)]
#[allow(dead_code)]
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
  pub(crate) const fn empty() -> Self {
    Self {
      data: ptr::null_mut(),
      len: 0,
      capacity: 0,
    }
  }
}

#[repr(C)]
/// Measured text run returned by Takumi FFI.
pub struct TakumiMeasuredTextRun {
  /// Text content for this run.
  pub text: *mut c_char,
  /// The x position of the run.
  pub x: f32,
  /// The y position of the run.
  pub y: f32,
  /// The width of the run.
  pub width: f32,
  /// The height of the run.
  pub height: f32,
}

#[repr(C)]
/// Flattened measured node returned by Takumi FFI.
pub struct TakumiMeasuredNode {
  /// Width of this node.
  pub width: f32,
  /// Height of this node.
  pub height: f32,
  /// Transform matrix.
  pub transform: [f32; 6],
  /// Index of first child node in `TakumiMeasuredLayout.nodes`.
  pub first_child: u32,
  /// Number of children.
  pub child_count: u32,
  /// Index of first text run in `TakumiMeasuredLayout.runs`.
  pub first_run: u32,
  /// Number of text runs.
  pub run_count: u32,
}

#[repr(C)]
/// Flattened measured layout result.
pub struct TakumiMeasuredLayout {
  /// Flat measured nodes array.
  pub nodes: *mut TakumiMeasuredNode,
  /// Number of nodes.
  pub nodes_len: usize,
  /// Allocation capacity for `nodes`.
  pub nodes_capacity: usize,
  /// Flat measured runs array.
  pub runs: *mut TakumiMeasuredTextRun,
  /// Number of runs.
  pub runs_len: usize,
  /// Allocation capacity for `runs`.
  pub runs_capacity: usize,
}

impl TakumiMeasuredLayout {
  pub(crate) const fn empty() -> Self {
    Self {
      nodes: ptr::null_mut(),
      nodes_len: 0,
      nodes_capacity: 0,
      runs: ptr::null_mut(),
      runs_len: 0,
      runs_capacity: 0,
    }
  }
}

#[repr(C)]
/// C string array result.
pub struct TakumiStringArray {
  /// Pointer to C string pointers.
  pub items: *mut *mut c_char,
  /// Number of strings.
  pub len: usize,
  /// Allocation capacity for `items`.
  pub capacity: usize,
}

impl TakumiStringArray {
  pub(crate) const fn empty() -> Self {
    Self {
      items: ptr::null_mut(),
      len: 0,
      capacity: 0,
    }
  }
}

#[allow(non_camel_case_types)]
#[repr(i32)]
#[derive(Clone, Copy)]
/// Output format used by C render options.
pub enum TakumiOutputFormat {
  /// PNG output.
  TAKUMI_OUTPUT_PNG = 0,
  /// JPEG output.
  TAKUMI_OUTPUT_JPEG = 1,
  /// WebP output.
  TAKUMI_OUTPUT_WEBP = 2,
  /// Raw RGBA bytes.
  TAKUMI_OUTPUT_RAW = 3,
}

#[repr(C)]
#[derive(Clone, Copy)]
/// C ABI render options that avoid JSON parsing overhead.
pub struct TakumiRenderOptions {
  /// Target width in pixels.
  /// `0` means unset; non-zero sets a fixed width.
  pub width: u32,
  /// Target height in pixels.
  /// `0` means unset; non-zero sets a fixed height.
  pub height: u32,
  /// Output format.
  pub format: TakumiOutputFormat,
  /// Output quality for lossy formats.
  /// `0` means unset; non-zero passes quality through.
  pub quality: u8,
  /// Draw debug borders (`0` = false, non-zero = true).
  pub draw_debug_border: u8,
  /// Device pixel ratio. Values <= 0 use Takumi default.
  pub device_pixel_ratio: f32,
}

impl Default for TakumiRenderOptions {
  fn default() -> Self {
    Self {
      width: 0,
      height: 0,
      format: TakumiOutputFormat::TAKUMI_OUTPUT_PNG,
      quality: 0,
      draw_debug_border: 0,
      device_pixel_ratio: DEFAULT_DEVICE_PIXEL_RATIO,
    }
  }
}

#[allow(non_camel_case_types)]
#[repr(i32)]
#[derive(Clone, Copy)]
/// Output format for animation rendering.
pub enum TakumiAnimationOutputFormat {
  /// Animated WebP output.
  TAKUMI_ANIMATION_OUTPUT_WEBP = 0,
  /// Animated PNG output.
  TAKUMI_ANIMATION_OUTPUT_APNG = 1,
}

#[repr(C)]
#[derive(Clone, Copy)]
/// C ABI options for animation rendering.
pub struct TakumiRenderAnimationOptions {
  /// Output width in pixels.
  pub width: u32,
  /// Output height in pixels.
  pub height: u32,
  /// Animation format.
  pub format: TakumiAnimationOutputFormat,
  /// Draw debug borders (`0` = false, non-zero = true).
  pub draw_debug_border: u8,
}

/// Opaque node handle used by the C node builder API.
pub struct TakumiNode {
  pub(crate) node: NodeKind,
}

#[repr(C)]
#[derive(Clone, Copy)]
/// Animation frame descriptor for C API.
pub struct TakumiAnimationFrame {
  /// Node handle for this frame.
  pub node: *const TakumiNode,
  /// Frame duration in milliseconds.
  pub duration_ms: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputFormat {
  Png,
  Jpeg,
  WebP,
  Raw,
}

#[derive(Clone, Copy)]
pub(crate) enum AnimationOutputFormat {
  APng,
  WebP,
}

impl From<TakumiOutputFormat> for OutputFormat {
  fn from(value: TakumiOutputFormat) -> Self {
    match value {
      TakumiOutputFormat::TAKUMI_OUTPUT_PNG => OutputFormat::Png,
      TakumiOutputFormat::TAKUMI_OUTPUT_JPEG => OutputFormat::Jpeg,
      TakumiOutputFormat::TAKUMI_OUTPUT_WEBP => OutputFormat::WebP,
      TakumiOutputFormat::TAKUMI_OUTPUT_RAW => OutputFormat::Raw,
    }
  }
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

impl From<TakumiAnimationOutputFormat> for AnimationOutputFormat {
  fn from(value: TakumiAnimationOutputFormat) -> Self {
    match value {
      TakumiAnimationOutputFormat::TAKUMI_ANIMATION_OUTPUT_WEBP => AnimationOutputFormat::WebP,
      TakumiAnimationOutputFormat::TAKUMI_ANIMATION_OUTPUT_APNG => AnimationOutputFormat::APng,
    }
  }
}
