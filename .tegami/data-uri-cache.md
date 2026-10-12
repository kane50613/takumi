---
packages:
  "takumi": minor
---

# Decode `data:` images once per renderer

A template that inlines its logo or background as a `data:` URI decodes it once, not on every render. Real cards with inlined images render up to 92% faster.

Rust callers pass their `ResourceCache` to `RenderOptions::builder().resource_cache(...)`. `ResourceCache` now implements `Clone`, and clones share one store.
