---
packages:
  takumi-core: patch
---

# Draw `feTurbulence` noise as Chrome does

`feTurbulence` now follows Skia's Perlin noise. It samples pixel centers without the specification's lattice offset, looks the lattice up once per corner, stores gradients in 16 bits, stitches tiles at the primitive subregion's whole size, and caps octaves at 9 as Blink does. Its noise now matches Chrome's to within rounding, where it used to differ visibly, and it runs about a third faster.
