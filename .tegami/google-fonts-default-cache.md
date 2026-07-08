---
packages:
  npm:@takumi-rs/helpers:
    type: patch
---

### Default the Google Fonts CSS cache

`googleFonts` now caches the CSS metadata process-wide when no `cache` is
passed, so callers who omit it still fetch each URL once. Pass your own `Map`
to scope the cache, or a fresh one per call to opt out.
