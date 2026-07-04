---
packages:
  cargo:takumi-core:
    replay:
      - exit-prerelease(cargo:takumi-core)
---

### Hide the encoder and layout crates behind opaque `Error` variants

`Error::PngError`, `WebPEncodingError`, `GifEncodingError`, `ImageError`, and
`LayoutError` exposed the `png`, `image_webp`, `gif`, `image`, and `taffy`
crates. They collapse into `Error::Encode` (an opaque boxed source, built via
`Error::encode`) and `Error::Layout(String)`, so the public API no longer
tracks those crates' versions. `EmptyAnimationFrames` and
`MixedAnimationFrameDimensions` also drop their `format` fields.
