---
"takumi-js": patch
---

Re-export `FontLoader`, `FontLoaderSync`, `ImageSourceLoader`, and `ImageSourceLoaderSync` from the package root, so the `fonts` / `persistentImages` option types no longer require a direct `@takumi-rs/core` import
