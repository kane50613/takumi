---
"takumi": major
"takumi-raster": minor
---

Rename `measure_layout` to `measure`, `render_sequence_animation` to `render_animation`, and `ImageOutputFormat` to `OutputFormat`; return a `Bitmap` newtype from `render` instead of `image::RgbaImage`, and take `&Bitmap` in `write_image`
