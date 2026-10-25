---
packages:
  takumi-core: patch
---

# Light SVG filters as Chrome does

`feDiffuseLighting` and `feSpecularLighting` now follow Skia's lighting shaders. Normals come from one Sobel kernel clamped at the primitive's edges, `surfaceScale` scales with the image, a spotlight fades in over Skia's narrow band past its cone, and a cone angle of zero or past 90 degrees widens to 90 as in Blink. They also run about 40% faster on large images.
