---
packages:
  takumi-core: minor
  takumi-raster: patch
---

# A render keeps to one thread while renders fill the cores

`takumi_core::parallel` counts the renders in flight. A render's blur and PNG deflate spread across the thread pool only while those renders leave a third of the pool idle, and stay on the render's own thread past that, which draws more images a second on a busy server. The bytes a render writes do not depend on it. `render` and `write_image` count themselves; a host that drives a render in several calls holds an `ActiveRender` across them.
