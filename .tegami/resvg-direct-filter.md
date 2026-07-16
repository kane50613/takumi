---
packages:
  "cargo:takumi-core": patch
---

### Apply filter references without building a render tree

Parse `<filter>` markup straight into resolved filters and run them on the layer pixels, skipping the synthetic document render.
