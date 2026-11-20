---
packages:
  takumi-pdf: patch
  takumi-paint: patch
---

# takumi-pdf and takumi-paint run under `nitro dev` and in Nuxt builds without config

`nitro dev` no longer fails with `Cannot find module .../pkg/takumi_pdf_wasm_bg.js`, and a Nuxt production server no longer fails with `ERR_MODULE_NOT_FOUND` for `bundlers/node.mjs`. Remove `noExternals: ["takumi-pdf"]` or `nitro.externals.traceInclude` if you added them as a workaround. Both still work.
