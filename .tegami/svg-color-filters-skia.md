---
packages:
  takumi-core: patch
---

# Run SVG color filters in floats as Chrome does

`feColorMatrix`, `feComponentTransfer`, and the conversions between sRGB and linearRGB now run as Skia's color filters do, in floats on unpremultiplied colors with Skia's `approx_powf`. A run of them stores bytes once instead of after each step. `saturate` above 1 now oversaturates, a `values` list of the wrong length leaves the colors as they are, and `feComponentTransfer` truncates its tables as Blink does. A chain of these filters now matches Chrome to within a level of rounding.
