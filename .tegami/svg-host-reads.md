---
packages:
  takumi-core:
    type: minor
  "@takumi-rs/core":
    type: patch
  "@takumi-rs/wasm":
    type: patch
---

# An SVG image drawn in two text colors shows each color

An SVG image that uses its host's `color` or fonts now follows each host it is drawn in. Before, the cache kept the raster of the first color, so the same icon next to red and blue text came out red both times. This affected an SVG image that:

- sets `color="currentColor"` on its root
- inherits `color` through a `<style>` rule
- nests an SVG image that uses `currentColor`

In Rust, `SvgSource::picture` returns the laid-out drawing ops with the key the cache keeps them under, and `Cache::builder().pictures(16 << 20)` sets their budget.

#### Migration

`SvgSource::vector_ops` still works, and is deprecated.

```diff
- let ops = source.vector_ops(&host, viewport);
+ let picture = source.picture(&host, viewport);
+ let ops = picture.ops();
```
