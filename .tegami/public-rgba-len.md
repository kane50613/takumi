---
packages:
  takumi-core: minor
---

# Expose `rgba_len` for sizing RGBA buffers

`resources::image_buffer::rgba_len` returns the byte length of a `width` by `height` RGBA buffer, or `None` when it overflows `usize`, so backends check sizes the same way the core does.
