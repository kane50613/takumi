---
packages:
  takumi-core:
    type: minor
  takumi:
    type: minor
  "@takumi-rs/helpers":
    type: patch
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  takumi-pdf:
    type: minor
  takumi-paint:
    type: minor
---

# Evict fonts one file at a time from the renderer's cache

A renderer now keeps the font files that renders and `@font-face` rules load in its cache pool, so `memory` and `maxBytes` count them, and the `fonts` budget caps them within the pool instead of apart from it. When the cache needs room, it evicts the file renders used least recently, so one family that keeps gaining files stays within the budget. The same bytes count once under any key, a buffer included. A render keeps every font it uses until it finishes, so concurrent renders draw with their fonts even under a budget of `0`. Fonts from `registerFont` still stay, and now count toward the cache. Among equally matching faces of one family that cover a character, the one a render lists last wins every time. A prepared render loads any `@font-face` file the renderer evicted after `prepare`. In Rust, `Cache::fonts` holds font files with leases, `CacheOptions::fonts` budgets them, `Fonts::unregister_source` removes the faces of one font file from a family, and `Fonts::prefer_sources` orders a family's faces.
