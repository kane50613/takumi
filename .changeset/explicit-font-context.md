---
"@takumi-rs/image-response": major
"@takumi-rs/core": major
"@takumi-rs/wasm": major
"takumi-js": major
"takumi": major
---

Make fonts and images explicit per-render resources: remove the persistent image store and `GlobalContext`, replace `loadFont`/`loadFontSync`/`loadFonts` with `registerFonts`, add a per-render `fonts` fallback chain and rename `fetchedResources` to `images`.
