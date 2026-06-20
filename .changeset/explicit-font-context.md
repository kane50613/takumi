---
"@takumi-rs/image-response": major
"@takumi-rs/core": major
"@takumi-rs/wasm": major
"takumi-js": major
"takumi": major
---

Make fonts and images explicit per-render resources: remove the persistent image store and `GlobalContext`, replace `loadFont`/`loadFontSync`/`loadFonts` with `registerFont`, add a per-render `fontFamilies` fallback chain, rename `fetchedResources` to `images`, add a per-image `cache` flag to opt individual images out of the decode cache, and remove the `createImageResponse` factory in favor of passing options to `ImageResponse` directly. The `Renderer` constructor is now parameterless — register fonts with `registerFont`, and the embedded default fonts are decoded once and shared across renderers.
