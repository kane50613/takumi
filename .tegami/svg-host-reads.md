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

In Rust, `SvgSource::picture` returns the laid-out drawing ops, which the render's cache keeps for the next host that reads the same color and fonts.

#### Migration

`SvgSource::vector_ops` is removed. `SvgSource::picture` returns the same ops, shared with the cache.

```diff
- let ops = source.vector_ops(&host, viewport);
+ let picture = source.picture(&host, viewport);
+ let ops = picture.ops();
```
