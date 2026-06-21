## takumi-raster@0.1.0-beta.0

### Rename render entry points and return a `Bitmap`

`measure_layout` becomes `measure`, `render_sequence_animation` becomes `render_animation`, and `ImageOutputFormat` becomes `OutputFormat`. `render` returns a `Bitmap` newtype instead of `image::RgbaImage`, and `write_image` takes `&Bitmap`.

### Split `takumi` into `takumi-core`, `takumi-raster`, and `takumi-svg` behind a re-export facade

### Minimize the public API

`takumi::prelude` exposes the stable data structures, entry-point functions sit at the crate root, the full backend crates move behind an `unstable` feature, and backend internals drop to `pub(crate)`.

### Rename the `raster` feature to `raster-backend`

This mirrors `svg-backend`, and `rayon` no longer enables it implicitly.

### Model image output quality per format

`ImageOutputFormat::Jpeg`/`WebP` carry a `Quality`, a new `WebPLossless` variant replaces lossless WebP (a `lossless` flag in the napi/wasm bindings), and `write_image` drops its quality argument.
