---
packages:
  takumi-js:
    type: patch
  takumi-pdf:
    type: patch
  "@takumi-rs/core":
    type: patch
  "@takumi-rs/wasm":
    type: patch
  "@takumi-rs/helpers":
    type: patch
---

### Name both images and PDF in the package metadata

Every package description and keyword list named the image pipeline only, so a search for HTML to PDF never reached `takumi-pdf`. Descriptions now state what each package takes in and what it writes out.
