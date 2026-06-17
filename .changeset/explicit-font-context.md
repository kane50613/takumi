---
"@takumi-rs/image-response": major
"@takumi-rs/core": major
"@takumi-rs/wasm": major
"takumi-js": major
"takumi": major
---

Remove the persistent image store and make fonts and images explicit per-render resources.

- Remove `putPersistentImage`, `clearImageStore`, and the `persistentImages` constructor option. Provide images via the per-render `images` option (bytes or a sync/async loader, keyed by `src`). Rename the `fetchedResources` option to `images`.
- Remove `loadFont`/`loadFontSync`/`loadFonts`. Register fonts via `registerFonts`, which returns the resolved families per font (`{ name, faces }`).
- `render`/`measure` accept a per-render `fonts` list (family names) as the fallback chain, applied without affecting concurrent renders. `takumi-js` resolves its `fonts` loaders through `registerFonts` automatically.
- Cache decoded images per renderer, keyed by content hash. Add `configureImageCache({ maxBytes })` to tune or disable it.
- Rust: `render`/`measure` take an explicit `&Fonts` (`RenderOptions::builder().fonts(..)`). Rename `RenderOptions::fetched_resources` to `images`. Remove `GlobalContext` and `PersistentImageStore`.
