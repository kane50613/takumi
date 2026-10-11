---
packages:
  "takumi-core": minor
  "takumi": patch
  "takumi-pdf": patch
---

# Lay text out once, during layout

Long documents render with less memory and time. Layout keeps each text box's lines as compact fragment items and drops the shaped text, so painting, page breaking and PDF output no longer shape or break the text again. A 1,500-page document of paragraphs renders in about a sixth less memory and a tenth less time.

`FragmentItems`, `OwnContent::fragment_items` and `RenderContext::release_shaped_text` are new.
