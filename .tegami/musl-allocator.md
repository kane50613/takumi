---
packages:
  "@takumi-rs/core": patch
---

# The Node.js binding renders faster on Alpine and other musl systems

On musl, the binding now allocates with mimalloc instead of musl's own allocator, which more than doubles the renders a second when several run at once. It holds more memory while it does; `MIMALLOC_PURGE_DELAY=0` returns freed memory at once, as the performance guide describes.
