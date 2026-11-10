---
packages:
  "@takumi-rs/core": patch
---

# The Node.js binding renders faster on Alpine and other musl systems

On musl, the binding now allocates with mimalloc instead of musl's own allocator, which doubles the renders a second when several run at once. It holds about a quarter more memory at the peak; `MIMALLOC_PURGE_DELAY=0` trades some of the speed back for less, as the performance guide describes.
