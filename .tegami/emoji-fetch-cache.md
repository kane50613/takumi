---
packages:
  "@takumi-rs/helpers": patch
---

# Emoji images download once per process

Without a `fetchCache`, every render used to download each emoji's SVG again. Emoji images from the `emoji` providers are now kept across renders in a cache of up to 8 MB per `fetch` function. Other remote images still need a `fetchCache` to be reused. The `fluent` and `fluentFlat` sets now load from a pinned commit of `shuding/fluentui-emoji-unicode`, as the other sets load from a pinned version, so a cached image cannot go stale.
