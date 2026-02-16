#ifndef TAKUMI_C_FFI_H
#define TAKUMI_C_FFI_H


#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Output format used by C render options.
 */
enum TakumiOutputFormat
#ifdef __cplusplus
  : int32_t
#endif // __cplusplus
 {
  /**
   * PNG output.
   */
  TAKUMI_OUTPUT_PNG = 0,
  /**
   * JPEG output.
   */
  TAKUMI_OUTPUT_JPEG = 1,
  /**
   * WebP output.
   */
  TAKUMI_OUTPUT_WEBP = 2,
  /**
   * Raw RGBA bytes.
   */
  TAKUMI_OUTPUT_RAW = 3,
};
#ifndef __cplusplus
typedef int32_t TakumiOutputFormat;
#endif // __cplusplus

/**
 * Output format for animation rendering.
 */
enum TakumiAnimationOutputFormat
#ifdef __cplusplus
  : int32_t
#endif // __cplusplus
 {
  /**
   * Animated WebP output.
   */
  TAKUMI_ANIMATION_OUTPUT_WEBP = 0,
  /**
   * Animated PNG output.
   */
  TAKUMI_ANIMATION_OUTPUT_APNG = 1,
};
#ifndef __cplusplus
typedef int32_t TakumiAnimationOutputFormat;
#endif // __cplusplus

/**
 * FFI status codes returned by Takumi C APIs.
 */
enum TakumiStatusCode
#ifdef __cplusplus
  : int32_t
#endif // __cplusplus
 {
  /**
   * Success.
   */
  TAKUMI_STATUS_OK = 0,
  /**
   * A required pointer argument was null.
   */
  TAKUMI_STATUS_NULL_POINTER = 1,
  /**
   * A string argument was not valid UTF-8.
   */
  TAKUMI_STATUS_INVALID_UTF8 = 2,
  /**
   * A JSON payload failed to deserialize.
   */
  TAKUMI_STATUS_INVALID_JSON = 3,
  /**
   * The provided arguments were invalid.
   */
  TAKUMI_STATUS_INVALID_ARGUMENT = 4,
  /**
   * An internal renderer error occurred.
   */
  TAKUMI_STATUS_INTERNAL_ERROR = 5,
  /**
   * A panic occurred inside the FFI boundary.
   */
  TAKUMI_STATUS_PANIC = 6,
};
#ifndef __cplusplus
typedef int32_t TakumiStatusCode;
#endif // __cplusplus

/**
 * Opaque node handle used by the C node builder API.
 */
typedef struct TakumiNode TakumiNode;

/**
 * Opaque renderer handle used by the C API.
 */
typedef struct TakumiRenderer TakumiRenderer;

/**
 * C ABI render options that avoid JSON parsing overhead.
 */
typedef struct {
  /**
   * Target width in pixels.
   * `0` means unset; non-zero sets a fixed width.
   */
  uint32_t width;
  /**
   * Target height in pixels.
   * `0` means unset; non-zero sets a fixed height.
   */
  uint32_t height;
  /**
   * Output format.
   */
  TakumiOutputFormat format;
  /**
   * Output quality for lossy formats.
   * `0` means unset; non-zero passes quality through.
   */
  uint8_t quality;
  /**
   * Draw debug borders (`0` = false, non-zero = true).
   */
  uint8_t draw_debug_border;
  /**
   * Device pixel ratio. Values <= 0 use Takumi default.
   */
  float device_pixel_ratio;
} TakumiRenderOptions;

/**
 * Owned byte buffer returned by Takumi FFI.
 */
typedef struct {
  /**
   * Pointer to the allocated byte data.
   */
  uint8_t *data;
  /**
   * Number of initialized bytes.
   */
  size_t len;
  /**
   * Allocation capacity for `data`.
   */
  size_t capacity;
} TakumiBytes;

/**
 * Flattened measured node returned by Takumi FFI.
 */
typedef struct {
  /**
   * Width of this node.
   */
  float width;
  /**
   * Height of this node.
   */
  float height;
  /**
   * Transform matrix.
   */
  float transform[6];
  /**
   * Index of first child node in `TakumiMeasuredLayout.nodes`.
   */
  uint32_t first_child;
  /**
   * Number of children.
   */
  uint32_t child_count;
  /**
   * Index of first text run in `TakumiMeasuredLayout.runs`.
   */
  uint32_t first_run;
  /**
   * Number of text runs.
   */
  uint32_t run_count;
} TakumiMeasuredNode;

/**
 * Measured text run returned by Takumi FFI.
 */
typedef struct {
  /**
   * Text content for this run.
   */
  char *text;
  /**
   * The x position of the run.
   */
  float x;
  /**
   * The y position of the run.
   */
  float y;
  /**
   * The width of the run.
   */
  float width;
  /**
   * The height of the run.
   */
  float height;
} TakumiMeasuredTextRun;

/**
 * Flattened measured layout result.
 */
typedef struct {
  /**
   * Flat measured nodes array.
   */
  TakumiMeasuredNode *nodes;
  /**
   * Number of nodes.
   */
  size_t nodes_len;
  /**
   * Allocation capacity for `nodes`.
   */
  size_t nodes_capacity;
  /**
   * Flat measured runs array.
   */
  TakumiMeasuredTextRun *runs;
  /**
   * Number of runs.
   */
  size_t runs_len;
  /**
   * Allocation capacity for `runs`.
   */
  size_t runs_capacity;
} TakumiMeasuredLayout;

/**
 * Animation frame descriptor for C API.
 */
typedef struct {
  /**
   * Node handle for this frame.
   */
  const TakumiNode *node;
  /**
   * Frame duration in milliseconds.
   */
  uint32_t duration_ms;
} TakumiAnimationFrame;

/**
 * C ABI options for animation rendering.
 */
typedef struct {
  /**
   * Output width in pixels.
   */
  uint32_t width;
  /**
   * Output height in pixels.
   */
  uint32_t height;
  /**
   * Animation format.
   */
  TakumiAnimationOutputFormat format;
  /**
   * Draw debug borders (`0` = false, non-zero = true).
   */
  uint8_t draw_debug_border;
} TakumiRenderAnimationOptions;

/**
 * C string array result.
 */
typedef struct {
  /**
   * Pointer to C string pointers.
   */
  char **items;
  /**
   * Number of strings.
   */
  size_t len;
  /**
   * Allocation capacity for `items`.
   */
  size_t capacity;
} TakumiStringArray;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Returns a pointer to the last thread-local error message.
 */
const char *takumi_last_error_message(void);

/**
 * Initializes a [`TakumiRenderOptions`] value with defaults.
 *
 * # Safety
 * `out_options` must be a valid writable pointer.
 */
int32_t takumi_render_options_init(TakumiRenderOptions *out_options);

/**
 * Creates a container node handle.
 *
 * # Safety
 * The returned pointer must be released with [`takumi_node_free`].
 */
TakumiNode *takumi_node_new_container(void);

/**
 * Creates a text node handle.
 *
 * # Safety
 * `text` must be a valid UTF-8 NUL-terminated string.
 * The returned pointer must be released with [`takumi_node_free`].
 */
TakumiNode *takumi_node_new_text(const char *text);

/**
 * Creates an image node handle.
 *
 * # Safety
 * `src` must be a valid UTF-8 NUL-terminated string.
 * The returned pointer must be released with [`takumi_node_free`].
 */
TakumiNode *takumi_node_new_image(const char *src,
                                  float width,
                                  uint8_t has_width,
                                  float height,
                                  uint8_t has_height);

/**
 * Sets Tailwind utility classes on a node.
 *
 * Pass null or empty string to clear Tailwind values.
 *
 * # Safety
 * `node` must be a valid node pointer. `tw` must be null or a valid UTF-8
 * NUL-terminated string.
 */
int32_t takumi_node_set_tw(TakumiNode *node, const char *tw);

/**
 * Sets an inline style property on a node.
 *
 * Currently supported properties: `width`, `height`, `backgroundColor`, `color`,
 * `fontSize`, `fontWeight`, `display`, `justifyContent`, `alignItems`, `textAlign`.
 *
 * # Safety
 * `node` must be valid. `property` and `value` must be valid UTF-8 NUL-terminated
 * strings.
 */
int32_t takumi_node_set_style(TakumiNode *node, const char *property, const char *value);

/**
 * Appends `child` to `parent`.
 *
 * Ownership of `child` is transferred on success.
 *
 * # Safety
 * `parent` and `child` must be valid pointers created by this library.
 */
int32_t takumi_node_add_child(TakumiNode *parent, TakumiNode *child);

/**
 * Frees a node created by this library.
 *
 * # Safety
 * `node` must be null or a pointer returned by `takumi_node_new_*` that has
 * not already been freed.
 */
void takumi_node_free(TakumiNode *node);

/**
 * Creates a renderer with default options.
 *
 * # Safety
 * The returned pointer must be released with [`takumi_renderer_free`].
 */
TakumiRenderer *takumi_renderer_new(void);

/**
 * Frees a renderer previously created by this library.
 *
 * # Safety
 * `renderer` must be null or a pointer returned by `takumi_renderer_new*` that has
 * not already been freed.
 */
void takumi_renderer_free(TakumiRenderer *renderer);

/**
 * Loads a font into a renderer.
 *
 * # Safety
 * `renderer` must be a valid renderer pointer. `font_data` must point to
 * `font_data_len` readable bytes. Optional C strings must be valid UTF-8 and
 * NUL-terminated when non-null.
 */
int32_t takumi_renderer_load_font(TakumiRenderer *renderer,
                                  const uint8_t *font_data,
                                  size_t font_data_len,
                                  const char *family_name,
                                  const char *style,
                                  uint16_t weight);

/**
 * Inserts a persistent image resource into the renderer.
 *
 * # Safety
 * `renderer` must be valid. `src` must be a valid NUL-terminated UTF-8 string.
 * `image_data` must point to `image_data_len` readable bytes.
 */
int32_t takumi_renderer_put_persistent_image(TakumiRenderer *renderer,
                                             const char *src,
                                             const uint8_t *image_data,
                                             size_t image_data_len);

/**
 * Clears the renderer persistent image store.
 *
 * # Safety
 * `renderer` must be a valid renderer pointer.
 */
int32_t takumi_renderer_clear_image_store(TakumiRenderer *renderer);

/**
 * Renders a pre-built node handle and returns encoded bytes.
 *
 * # Safety
 * `renderer` and `node` must be valid pointers. `options` may be null to use
 * defaults. `out_bytes` must be a valid writable pointer; free with
 * [`takumi_bytes_free`].
 */
int32_t takumi_renderer_render(const TakumiRenderer *renderer,
                               const TakumiNode *node,
                               const TakumiRenderOptions *options,
                               TakumiBytes *out_bytes);

/**
 * Measures a pre-built node handle and returns a flattened layout struct.
 *
 * # Safety
 * `renderer` and `node` must be valid pointers. `options` may be null to use
 * defaults. `out_layout` must be writable and later freed with
 * [`takumi_measured_layout_free`].
 */
int32_t takumi_renderer_measure(const TakumiRenderer *renderer,
                                const TakumiNode *node,
                                const TakumiRenderOptions *options,
                                TakumiMeasuredLayout *out_layout);

/**
 * Renders animation frames and returns encoded animation bytes.
 *
 * # Safety
 * `renderer` and `frames` must be valid pointers. `out_bytes` must be writable
 * and later freed with [`takumi_bytes_free`].
 */
int32_t takumi_renderer_render_animation(const TakumiRenderer *renderer,
                                         const TakumiAnimationFrame *frames,
                                         size_t frame_count,
                                         const TakumiRenderAnimationOptions *options,
                                         TakumiBytes *out_bytes);

/**
 * Extracts external resource URLs from a node and returns C string array.
 *
 * # Safety
 * `node` must be valid and `out_urls` must be a writable pointer.
 */
int32_t takumi_extract_resource_urls(const TakumiNode *node, TakumiStringArray *out_urls);

/**
 * Frees bytes returned by this library.
 *
 * # Safety
 * `bytes` must originate from Takumi APIs that return `TakumiBytes` and must not
 * be freed more than once.
 */
void takumi_bytes_free(TakumiBytes bytes);

/**
 * Frees a measured layout returned by this library.
 *
 * # Safety
 * `layout` must originate from `takumi_renderer_measure`.
 */
void takumi_measured_layout_free(TakumiMeasuredLayout layout);

/**
 * Frees a string array returned by this library.
 *
 * # Safety
 * `value` must originate from APIs returning `TakumiStringArray`.
 */
void takumi_string_array_free(TakumiStringArray value);

/**
 * Frees a C string returned by this library.
 *
 * # Safety
 * `value` must be null or a pointer returned by Takumi that has not been freed.
 */
void takumi_string_free(char *value);

/**
 * Initializes a `TakumiBytes` output struct to an empty state.
 *
 * # Safety
 * `out_bytes` must be a valid writable pointer.
 */
int32_t takumi_bytes_init(TakumiBytes *out_bytes);

/**
 * Initializes a `TakumiMeasuredLayout` output struct to an empty state.
 *
 * # Safety
 * `out_layout` must be a valid writable pointer.
 */
int32_t takumi_measured_layout_init(TakumiMeasuredLayout *out_layout);

/**
 * Initializes a `TakumiStringArray` output struct to an empty state.
 *
 * # Safety
 * `out_value` must be a valid writable pointer.
 */
int32_t takumi_string_array_init(TakumiStringArray *out_value);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif
