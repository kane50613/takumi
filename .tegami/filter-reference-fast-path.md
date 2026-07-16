---
packages:
  "cargo:takumi-core": patch
  "cargo:takumi-raster": patch
---

### Apply filter references without the base64 roundtrip

`apply_svg_filter` hands the layer to resvg through a custom href resolver as fast-compressed PNG bytes, dropping the base64 encode, data-URI decode, and multi-megabyte XML parse.
