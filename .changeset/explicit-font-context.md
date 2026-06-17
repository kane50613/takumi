---
"@takumi-rs/image-response": major
"@takumi-rs/core": major
"@takumi-rs/wasm": major
"takumi-js": major
"takumi": major
---

Remove the persistent image store and make font state explicit.

- Removed `putPersistentImage`, `clearImageStore`, and the `persistentImages` constructor option. Provide images up front via `images` (keyed by `src`, each with bytes or a sync/async loader) instead. The render option formerly named `fetchedResources` is now `images`.
- Renderer font state is now a content-addressed decode cache that memoizes the expensive woff2/woff decode and deduplicates re-registered fonts, so reusing a font never re-decodes or piles up duplicate faces.
- Images are now decoded through a per-renderer content-addressed cache, so an image reused across renders (e.g. animation frames) only decodes once. Added `Renderer.configureImageCache({ maxBytes })` to tune (or disable) it.
- Added `Renderer.configureFontCache({ maxBytes })` to tune (or disable) the per-renderer decode cache.
- **Rust:** `GlobalContext` and `PersistentImageStore` are gone; `render`/`measure` take an explicit `&Fonts` (via `RenderOptions::builder().fonts(..)`). `RenderOptions::fetched_resources` is now `images`.
