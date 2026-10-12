---
packages:
  "takumi-core": minor
  "takumi": patch
  "takumi-pdf": patch
---

# Lay text out once

Painting, page breaking and PDF output reuse the lines layout made instead of shaping text again. A 1,500-page document renders in about a sixth less memory and a tenth less time.

`FragmentItems`, `OwnContent::fragment_items` and `RenderContext::release_shaped_text` are new.
