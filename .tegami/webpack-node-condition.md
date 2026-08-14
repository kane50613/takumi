---
packages:
  "@takumi-rs/wasm":
    type: patch
  takumi-pdf:
    type: patch
---

### Pick the Node entry when webpack targets Node

A webpack build for Node resolved the Vite entry, because both environments set the `module` condition and it is listed first. The build then failed on that entry's `?url` import, which only Vite reads. A `webpack` condition now routes webpack's Node target to the Node entry, and every other bundler keeps the entry it already resolved.
