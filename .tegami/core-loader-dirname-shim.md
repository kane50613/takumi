---
packages:
  "npm:@takumi-rs/core": patch
---

### Strip the unused `__dirname` shim from the generated loader

NAPI-RS emits a `new URL(".", import.meta.url)` `__dirname` shim the loader never
reads. webpack and Turbopack treat it as an asset reference and fail the build
when the addon is bundled for a Node runtime (such as a Next.js node-runtime
route). The loader patch now drops it.
