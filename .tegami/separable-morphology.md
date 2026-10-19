---
packages:
  takumi-core: patch
---

# Speed up `feMorphology`

`feMorphology` read every pixel of its radius rectangle for each output pixel. It now sweeps rows, then columns, with the same result. A large radius runs several times faster.
