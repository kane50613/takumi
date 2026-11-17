---
packages:
  takumi-core:
    type: minor
  "@takumi-rs/core":
    type: patch
  "@takumi-rs/wasm":
    type: patch
---

# An `svg` element drawn in two text colors shows each color

An `svg` element whose root sets `color="currentColor"` or inherits `color` through a `<style>` rule now draws in the `color` of each box it sits in. Before, the cache kept the raster of the first color, so the same icon next to red and blue text came out red both times. An SVG image that uses its host's fonts also follows each host it is drawn in.

In Rust, `SvgSource::picture` returns the laid-out drawing ops with the key the cache keeps them under, and `Cache::builder().pictures(16 << 20)` caps their bytes.

#### Migration

`SvgSource::vector_ops` is removed. `SvgSource::picture` returns the same ops, shared with the cache.

```diff
- let ops = source.vector_ops(&host, viewport);
+ let picture = source.picture(&host, viewport);
+ let ops = picture.ops();
```
