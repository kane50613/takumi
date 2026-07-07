---
packages:
  cargo:takumi:
    replay:
      - exit-prerelease(cargo:takumi)
---

### Rename the `svg` feature to `svg-source`

`svg` and `svg-backend` read as the same thing at a glance despite gating
opposite directions (image-source input vs. render output). The umbrella's
input-side feature is now `svg-source`; `svg-backend` is unchanged.
