#ifndef TAKUMI_C_FFI_H
#define TAKUMI_C_FFI_H


#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

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
 * Opaque renderer handle used by the C API.
 */
typedef struct TakumiRenderer TakumiRenderer;

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

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Returns a pointer to the last thread-local error message.
 */
const char *takumi_last_error_message(void);

/**
 * Creates a renderer with default options.
 *
 * # Safety
 * The returned pointer must be released with [`takumi_renderer_free`].
 */
TakumiRenderer *takumi_renderer_new(void);

/**
 * Creates a renderer with optional JSON options payload.
 *
 * # Safety
 * `options_json` must point to `options_json_len` readable bytes when non-null.
 * The returned pointer must be released with [`takumi_renderer_free`].
 */
TakumiRenderer *takumi_renderer_new_with_options(const uint8_t *options_json,
                                                 size_t options_json_len);

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
 * Renders a node JSON payload and returns encoded bytes.
 *
 * # Safety
 * `renderer` must be valid. `node_json` must point to `node_json_len` readable bytes.
 * `options_json` may be null; otherwise it must point to `options_json_len` readable bytes.
 * `out_bytes` must be a valid writable pointer; free with [`takumi_bytes_free`].
 */
int32_t takumi_renderer_render(const TakumiRenderer *renderer,
                               const uint8_t *node_json,
                               size_t node_json_len,
                               const uint8_t *options_json,
                               size_t options_json_len,
                               TakumiBytes *out_bytes);

/**
 * Measures a node JSON payload and returns layout JSON.
 *
 * # Safety
 * `renderer` must be valid. `node_json` must point to `node_json_len` readable bytes.
 * `options_json` may be null; otherwise it must point to `options_json_len` readable bytes.
 * `out_json` must be a valid writable pointer; free with [`takumi_string_free`].
 */
int32_t takumi_renderer_measure(const TakumiRenderer *renderer,
                                const uint8_t *node_json,
                                size_t node_json_len,
                                const uint8_t *options_json,
                                size_t options_json_len,
                                char **out_json);

/**
 * Renders animation frames and returns encoded animation bytes.
 *
 * # Safety
 * `renderer` must be valid. `frames_json` and `options_json` must point to readable
 * byte ranges with the given lengths. `out_bytes` must be writable and later freed
 * with [`takumi_bytes_free`].
 */
int32_t takumi_renderer_render_animation(const TakumiRenderer *renderer,
                                         const uint8_t *frames_json,
                                         size_t frames_json_len,
                                         const uint8_t *options_json,
                                         size_t options_json_len,
                                         TakumiBytes *out_bytes);

/**
 * Extracts external resource URLs from a node JSON payload.
 *
 * # Safety
 * `node_json` must point to `node_json_len` readable bytes and `out_json` must be
 * a valid writable pointer; free with [`takumi_string_free`].
 */
int32_t takumi_extract_resource_urls(const uint8_t *node_json,
                                     size_t node_json_len,
                                     char **out_json);

/**
 * Frees bytes returned by this library.
 *
 * # Safety
 * `bytes` must originate from Takumi APIs that return `TakumiBytes` and must not
 * be freed more than once.
 */
void takumi_bytes_free(TakumiBytes bytes);

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

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif
