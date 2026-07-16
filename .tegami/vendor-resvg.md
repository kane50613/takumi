---
packages:
  "cargo:takumi-core": patch
---

### Vendor resvg into takumi-core

Replace the external resvg dependency with a vendored copy of usvg + resvg 0.47, stripped of the text, svgz, system-fonts, memmap-fonts and writer features and the CLI. Rendering output is unchanged.
