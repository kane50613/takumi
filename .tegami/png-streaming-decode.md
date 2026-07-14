---
packages:
  "cargo:takumi-core": patch
---

### Stream PNG decode at draw size

Non-interlaced PNGs decode row-by-row through a streaming resampler, so downscaling never materializes the full-size buffer. Output is byte-identical to decode-then-resize.
