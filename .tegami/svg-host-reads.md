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

An `svg` element that inherits `color`, through `color="currentColor"` on its root or a `<style>` rule, now draws in the `color` of each box it sits in. Before, every copy took the first color it was drawn in. An SVG image that uses its host's fonts follows each host too.

#### Migration

`SvgSource::vector_ops` is removed. `SvgSource::picture` returns the same ops, shared with the cache.

```diff
- let ops = source.vector_ops(&host, viewport);
+ let picture = source.picture(&host, viewport);
+ let ops = picture.ops();
```
