---
packages:
  cargo:takumi-raster:
    replay:
      - exit-prerelease(cargo:takumi-raster)
---

### Clip backdrop-filter output by the node's mask and clip-path

The filtered backdrop painted across the whole border box even when the node
had a `mask` or `clip-path`, unlike browsers where the mask applies to the
filtered backdrop too. The backdrop composite is now attenuated by the node
mask, and a fully masked-out node skips the backdrop filter entirely.
