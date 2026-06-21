---
packages:
  "cargo:takumi": major
  "npm:takumi-js": major
  "npm:@takumi-rs/core": major
  "npm:@takumi-rs/image-response": major
  "npm:@takumi-rs/wasm": major
---

### Make fonts and images explicit per-render resources

Drop the persistent image store and `GlobalContext`, and pass fonts and images per render. `registerFont` replaces `loadFont`/`loadFontSync`/`loadFonts`, each render takes a `fontFamilies` fallback chain, and `images` replaces `fetchedResources`.
