---
packages:
  "takumi": minor
---

# Decode data URI images once per renderer

An image inlined as a `data:` URI now decodes through the renderer's resource cache, so a template that embeds its logo or background decodes it once instead of on every render. Real cards with inlined images render up to 92% faster.

Rust callers opt in by passing their `ResourceCache` to `RenderOptions::builder().resource_cache(...)`. `ResourceCache` now implements `Clone`, and clones share one store.
