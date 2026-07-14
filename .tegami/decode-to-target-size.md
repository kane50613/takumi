---
packages:
  "cargo:takumi-core": minor
  "cargo:takumi-raster": patch
  "cargo:takumi-svg": patch
---

### Decode bitmaps at draw size

Bitmaps loaded through `ImageCache` stay encoded until draw time, decode scaled to the box they are drawn into, and cache per content and target size. Adds `ImageSource::Encoded`; `get_or_decode` returns it instead of `Bitmap` for PNG/JPEG/WebP. `cache: "none"` keeps returning an eagerly decoded `Bitmap`.
