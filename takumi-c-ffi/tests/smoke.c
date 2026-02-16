#include "takumi.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CHECK_OK(code, call_name)                                                      \
  do {                                                                                 \
    if ((code) != TAKUMI_STATUS_OK) {                                                  \
      const char *msg__ = takumi_last_error_message();                                 \
      fprintf(stderr, "%s failed: %d (%s)\\n", (call_name), (int)(code),            \
              msg__ ? msg__ : "<no error>");                                          \
      return 1;                                                                        \
    }                                                                                  \
  } while (0)

#define CHECK(cond, msg)                                                               \
  do {                                                                                 \
    if (!(cond)) {                                                                     \
      fprintf(stderr, "check failed: %s\\n", (msg));                                 \
      return 1;                                                                        \
    }                                                                                  \
  } while (0)

static int read_file(const char *path, uint8_t **out_data, size_t *out_len) {
  FILE *fp = fopen(path, "rb");
  if (fp == NULL) {
    return 0;
  }

  if (fseek(fp, 0, SEEK_END) != 0) {
    fclose(fp);
    return 0;
  }

  long size = ftell(fp);
  if (size <= 0) {
    fclose(fp);
    return 0;
  }

  if (fseek(fp, 0, SEEK_SET) != 0) {
    fclose(fp);
    return 0;
  }

  uint8_t *buffer = (uint8_t *)malloc((size_t)size);
  if (buffer == NULL) {
    fclose(fp);
    return 0;
  }

  size_t read_len = fread(buffer, 1, (size_t)size, fp);
  fclose(fp);
  if (read_len != (size_t)size) {
    free(buffer);
    return 0;
  }

  *out_data = buffer;
  *out_len = read_len;
  return 1;
}

int main(int argc, char **argv) {
  CHECK(argc >= 3, "missing font/image path arguments");

  const char *font_path = argv[1];
  const char *image_path = argv[2];

  TakumiBytes initialized = {0};
  int32_t status = takumi_bytes_init(&initialized);
  CHECK_OK(status, "takumi_bytes_init");
  CHECK(initialized.data == NULL && initialized.len == 0 && initialized.capacity == 0,
        "takumi_bytes_init should zero initialize");

  // Trigger an error to validate takumi_last_error_message.
  status = takumi_renderer_clear_image_store(NULL);
  CHECK(status == TAKUMI_STATUS_NULL_POINTER,
        "takumi_renderer_clear_image_store(NULL) should return NULL_POINTER");
  const char *last_error = takumi_last_error_message();
  CHECK(last_error != NULL && strlen(last_error) > 0,
        "takumi_last_error_message should be populated after failure");

  TakumiRenderer *renderer = takumi_renderer_new();
  CHECK(renderer != NULL, "takumi_renderer_new returned NULL");

  uint8_t *font_data = NULL;
  size_t font_len = 0;
  CHECK(read_file(font_path, &font_data, &font_len), "failed to read font file");
  uint8_t *image_data = NULL;
  size_t image_len = 0;
  CHECK(read_file(image_path, &image_data, &image_len), "failed to read image file");

  status = takumi_renderer_load_font(renderer, font_data, font_len, "Archivo", "normal", 0);
  free(font_data);
  CHECK_OK(status, "takumi_renderer_load_font");

  status = takumi_renderer_put_persistent_image(
    renderer,
    "memory://pixel.png",
    image_data,
    image_len
  );
  free(image_data);
  CHECK_OK(status, "takumi_renderer_put_persistent_image");

  status = takumi_renderer_clear_image_store(renderer);
  CHECK_OK(status, "takumi_renderer_clear_image_store");

  TakumiNode *root = takumi_node_new_container();
  CHECK(root != NULL, "takumi_node_new_container returned NULL");
  TakumiNode *text = takumi_node_new_text("Hello Takumi");
  CHECK(text != NULL, "takumi_node_new_text returned NULL");
  status = takumi_node_set_tw(root, "w-full h-full bg-white flex items-center justify-center");
  CHECK_OK(status, "takumi_node_set_tw(root)");
  status = takumi_node_set_tw(text, "text-black text-lg");
  CHECK_OK(status, "takumi_node_set_tw(text)");
  status = takumi_node_add_child(root, text);
  CHECK_OK(status, "takumi_node_add_child");

  TakumiRenderOptions options;
  status = takumi_render_options_init(&options);
  CHECK_OK(status, "takumi_render_options_init");
  options.width = 128;
  options.height = 64;
  options.format = TAKUMI_OUTPUT_PNG;

  TakumiBytes render_bytes = {0};
  status = takumi_renderer_render(renderer, root, &options, &render_bytes);
  CHECK_OK(status, "takumi_renderer_render");
  CHECK(render_bytes.data != NULL && render_bytes.len > 0, "render output should not be empty");
  takumi_bytes_free(render_bytes);
  takumi_node_free(root);

  // Explicitly test null-safe free helpers.
  takumi_node_free(NULL);
  takumi_renderer_free(NULL);
  TakumiBytes empty = {0};
  takumi_bytes_free(empty);

  takumi_renderer_free(renderer);

  return 0;
}
