---
"@takumi-rs/image-response": major
"@takumi-rs/core": major
"@takumi-rs/wasm": major
"takumi-js": major
"takumi": major
---

Make fonts and images explicit per-render resources: remove the persistent image store and `GlobalContext`, replace `loadFont`/`loadFontSync`/`loadFonts` with `registerFont`, add a per-render `fontFamilies` fallback chain, rename `fetchedResources` to `images`, and add a per-image `cache` flag to opt individual images out of the decode cache.
