---
packages:
  takumi-pdf: patch
  takumi-paint: patch
---

# takumi-pdf and takumi-paint run under `nitro dev` and in Nuxt builds without config

Two setups no longer fail:

- `nitro dev` used to fail with `Cannot find module .../pkg/takumi_pdf_wasm_bg.js`.
- A Nuxt production server used to fail with `ERR_MODULE_NOT_FOUND` for `bundlers/node.mjs`.

Remove the workaround if you added one. Both still work if you keep them.

```diff
 // nitro.config.ts
 export default defineConfig({
-  noExternals: ["takumi-pdf"],
 });

 // nuxt.config.ts
 export default defineNuxtConfig({
-  nitro: {
-    externals: { traceInclude: ["node_modules/takumi-pdf/bundlers/node.mjs"] },
-  },
 });
```
