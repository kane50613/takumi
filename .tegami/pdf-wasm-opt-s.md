---
packages:
  takumi-pdf:
    type: patch
---

# The WebAssembly build renders PDFs 12 to 15% faster

The PDF WebAssembly binary now builds its layout and paint code for speed as well as size. An invoice renders in 17.5 ms in place of 20.7 ms, and an 11,000-row table in 63 s in place of 71 s, with the same PDF. The download grows from 1.61 MB to 1.70 MB with brotli.
