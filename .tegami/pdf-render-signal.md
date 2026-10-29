---
packages:
  takumi-pdf: minor
---

# `render` in `takumi-pdf` takes an abort `signal`

Pass `signal` to stop a render before it reads the document or draws it, as `@takumi-rs/core` renders take one.
