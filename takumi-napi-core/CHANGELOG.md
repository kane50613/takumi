# @takumi-rs/core

## 0.68.8

## 0.68.7

## 0.68.6

## 0.68.5

## 0.68.4

## 0.68.3

## 0.68.2

## 0.68.1

## 0.68.0

### Patch Changes

- 7684faa: refactor font loading to reduce buffer copying

## 0.67.3

## 0.67.2

### Patch Changes

- e8cc16c: add `loadFontSync`

## 0.67.1

## 0.67.0

## 0.66.14

## 0.66.13

## 0.66.12

### Patch Changes

- 7389d6e: add persistent image cache

## 0.66.11

## 0.66.10

## 0.66.9

## 0.66.8

## 0.66.7

### Patch Changes

- a3e7f9c: document all the functions

## 0.66.6

## 0.66.5

## 0.66.4

## 0.66.3

## 0.66.2

## 0.66.1

## 0.66.0

### Patch Changes

- 80da5a7: remove unused `browser` field

## 0.65.0

### Minor Changes

- 1319540: new `measure()` API

## 0.64.1

### Patch Changes

- 0dc36ce: hint GC about the external buffers

## 0.64.0

### Minor Changes

- 1600ff0: make `fetchedResources` accept `ImageSource` array instead of map

### Patch Changes

- 1600ff0: deprecate `PersistentImage` type

## 0.63.2

## 0.63.1

### Patch Changes

- 9fb085f: deprecate `purgeResourcesCache`, remove `resourceCacheCapacity` option for constructing `Renderer`

## 0.63.0

### Minor Changes

- 75b0f10: **BREAKING: Externalize image fetching**

  To allow more control over fetching and match the WASM version, `@takumi-rs/core` no longer runs `fetch` for you. `@takumi-rs/image-response` will not be affected by this change.

  Before:

  ```tsx
  const renderer = new Renderer();
  const node = await fromJsx(<img src="https://example.com/image.png" />);
  const image = await renderer.render(node);
  ```

  After:

  ```tsx
  import { extractResourceUrls } from "@takumi-rs/core";
  import { fetchResources } from "@takumi-rs/helpers";

  const renderer = new Renderer();
  const node = await fromJsx(<img src="https://example.com/image.png" />);

  const urls = extractResourceUrls(node);
  const fetchedResources = await fetchResources(urls);

  const image = await renderer.render(node, {
    fetchedResources,
  });
  ```

## 0.62.8

## 0.62.7

### Patch Changes

- 43036a0: return error instead of panicing

## 0.62.6

## 0.62.5

## 0.62.4

### Patch Changes

- 56ec805: downgrade napi requirement from napi10 to napi4 enables node.js v10.16+ support

## 0.62.3

### Patch Changes

- e5d41bc: fix `fetch` option type definition
- 33c9ba0: fix failure to retrieve buffer instance after `fetch().arrayBuffer()` #349

## 0.62.2

## 0.62.1

## 0.62.0

## 0.61.1

## 0.61.0

## 0.60.8

## 0.60.7

## 0.60.6

## 0.60.5

## 0.60.4

## 0.60.3

## 0.60.2

## 0.60.1

## 0.60.0

## 0.59.1

## 0.59.0

## 0.58.0

## 0.57.6

## 0.57.5

## 0.57.4

## 0.57.3

## 0.57.2

## 0.57.1

## 0.57.0

### Minor Changes

- 42572bb: **Drop `avif` format support**

## 0.56.1

## 0.56.0

## 0.55.4

## 0.55.3

## 0.55.2

## 0.55.1

## 0.55.0

## 0.54.3

### Patch Changes

- 8c6e17e: make `render(options)` parameter optional

## 0.54.2

## 0.54.1

## 0.54.0

## 0.53.1

## 0.53.0

## 0.52.2

## 0.52.1

## 0.52.0

## 0.51.1

## 0.51.0

### Minor Changes

- 27ac6c5: support `devicePixelRatio` value

## 0.50.0

## 0.49.1

## 0.49.0

## 0.48.0

### Minor Changes

- c3f1b7d: support optional width/height

## 0.47.0

## 0.46.6

### Patch Changes

- 73c07ff: dont create ref if only buffer slice is needed

## 0.46.5

### Patch Changes

- b97af12: fix failed to parse buffer input

## 0.46.4

## 0.46.3

## 0.46.2

## 0.46.1

## 0.46.0

## 0.45.3

## 0.45.2

## 0.45.1

## 0.45.0

## 0.44.0

## 0.43.1

## 0.43.0

### Minor Changes

- 796c578: deprecate pascal case image format

## 0.42.0

## 0.41.0

## 0.40.2

## 0.40.1

## 0.40.0

## 0.39.0

### Patch Changes

- 71ae4a5: parallelize background image layers rendering

## 0.38.1

## 0.38.0

### Minor Changes

- 7245e49: calls nodejs `fetch()` to get url resources.
- 7245e49: **drop `renderSync` support** (since `fetch()` requires async event loop).

## 0.37.0

## 0.36.2

## 0.36.1

## 0.36.0

## 0.35.2

## 0.35.1

### Patch Changes

- f18e9c5: enable `@takumi-rs/core-darwin-x64` target support

## 0.35.0

### Patch Changes

- 12a2d3f: fix `aspect-ratio`, `flex-grow` numberic value parsing

## 0.34.0

### Patch Changes

- 7c402d8: setup npm trusted publisher

## 0.33.1

### Patch Changes

- df18b3d: exclude `CHANGELOG.md` from being published

## 0.33.0

### Minor Changes

- 98755a7: **drop support for `debug` field, replace with `draw_debug_border` option in rendering functions**
