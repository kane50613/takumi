---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Pull neighbouring text in with a span's negative margin

A negative horizontal margin on an inline span now overlaps the span with the content beside it, as Chrome does, instead of reserving nothing.
