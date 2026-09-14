---
"takumi-pdf": patch
---

# Write the CIDFont default width as an integer

The PDF spec types `/DW` as an integer. Poppler ignores a real one and falls back to the spec default of 1000, so every glyph the entry covered advanced far too far in poppler-based viewers.
