---
packages:
  takumi-core: patch
---

# Run `feMorphology` and `feConvolveMatrix` as Chrome does

`feMorphology` now rounds its radius, reads transparent pixels past the filter region, and leaves an axis with a radius of zero as it is. `feConvolveMatrix` now adds its bias before premultiplying, tiles `duplicate` and `wrap` within the primitive's subregion, passes the source graphic on for a kernel over 256 cells, and falls back to the kernel's sum for a divisor of zero. Both also run several times faster on large images.
