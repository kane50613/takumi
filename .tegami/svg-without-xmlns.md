---
packages:
  "cargo:takumi-core": patch
---

### Accept SVG sources without xmlns

Inline SVG image sources no longer need an `xmlns` declaration. Data-URI decoding and base64 encoding are shared through `resources::image` (new `to_data_url`).
