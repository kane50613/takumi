# takumi

## 0.56.0

### Minor Changes

- 1ac44c4: `mask-image` behaves correctly like a "mask" now instead of overlay image.
- 1ac44c4: support `background-clip`

### Patch Changes

- c1260a2: `line-clamp` should has ellipsis if overflow

## 0.55.4

### Patch Changes

- 34bf0af: fix mask image on text drawing overflows

## 0.55.3

### Patch Changes

- cd93ee9: handles special case of `text-overflow: ellipsis` + `text-wrap: nowrap`

## 0.55.2

### Patch Changes

- 274c716: reuse masking buffer to avoid allocation

## 0.55.1

### Patch Changes

- 3df6648: use `RefCell` internally for scratch buffer

## 0.55.0

### Minor Changes

- 5e79e33: support COLR emoji font drawing (e.g. twemoji)

### Patch Changes

- 5e79e33: reuse buffer for masking to reduce allocation

## 0.54.3

## 0.54.2

### Patch Changes

- df1aa7e: update `parley` to `0.7`

## 0.54.1

### Patch Changes

- b16fd1b: fix whitespace keywords parsing

## 0.54.0

### Minor Changes

- e8ea400: refactor `TakumiError` struct and eliminate `unwrap()` calls

### Patch Changes

- e6a0934: Crate: fix justify-content css parse

## 0.53.1

### Patch Changes

- 29a575c: optimize `CssValue` deserialize implementation to reduce generated `Visitor` variant

## 0.53.0

### Minor Changes

- 7740504: drop `ts_rs` support
- 4623702: **`textStroke` related properties will have prefix `WebkitTextStroke`**

## 0.52.2

### Patch Changes

- 563bf31: optimize transform to reduce multiplications

## 0.52.1

### Patch Changes

- 3fa5c55: optimize tailwind parser function size

## 0.52.0

### Minor Changes

- ed409d4: refactor `overflow` & `clip-path` rendering to avoid extra allocations
- b9b0a85: speed up out of viewport image rendering

### Patch Changes

- ed409d4: make transform behave correctly

## 0.51.1

### Patch Changes

- eb26a60: fix `overflow`, `clip-path`, `background-position` deserialization

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

### Minor Changes

- 7d3dbf1: replace `csscolorparser` with `color` crate to support more color functions

## 0.46.6

## 0.46.5

## 0.46.4

### Patch Changes

- 37610e0: bump `csscolorparser` to 0.8

## 0.46.3

## 0.46.2

## 0.46.1

### Patch Changes

- 9365705: fix `justify-between`, `around`, `evenly` tailwind parsing

## 0.46.0

### Minor Changes

- 18bbc7c: support tailwind breakpoint & important parsing #273

## 0.45.3

### Patch Changes

- 3cf3867: fix `bg-size-[…]`, `bg-position-[…]` arbitrary value parsing

## 0.45.2

### Patch Changes

- d28e982: add `background-image` arbitrary value parsing
- 3c0243b: fix gradient step parsing
- 1ba2585: bump minimum rust version to 1.88
- 3c0243b: prevent panicing in font weight parsing

## 0.45.1

### Patch Changes

- 97ba495: fix `rounded` parsing

## 0.45.0

### Minor Changes

- 702c419: add tailwind parser
- 702c419: support `inline`/`block` for padding/margin/inset/border-width

## 0.44.0

### Minor Changes

- 368fc1c: Support `textWrap`, `textWrapMode`, `whiteSpace`, `whiteSpaceCollapse` properties

  **BREAKING CHANGE: by default text will collapse instead of preserve**, use `whiteSpace: pre;` to get the same behavior

## 0.43.1

## 0.43.0

## 0.42.0

### Minor Changes

- 44368b8: remove all Mutex/RwLock uses
- 44368b8: replace noise-v1 to use lighter hash function, only `opacity()` & `seed()` is supported

## 0.41.0

### Patch Changes

- 8318812: fix `PositionComponent` should be untagged

## 0.40.2

### Patch Changes

- 21a9988: add `word-break: break-word` as alias for `word-break: normal` + `overflow-wrap: anywhere`
- ddae1b5: fix `letter-spacing`, `word-spacing` should not divide by font size

## 0.40.1

### Patch Changes

- 8751a1b: fix fetch tasks collecting being overwritten

## 0.40.0

### Minor Changes

- ae7062f: support `clip-path`, `clip-rule`

### Patch Changes

- ae7062f: fix inline content not being clipped by overflow constraints

## 0.39.0

### Minor Changes

- 71ae4a5: use `data-url` crate, **remove `image_data_uri` feature**

### Patch Changes

- 71ae4a5: parallelize background image layers rendering

## 0.38.1

### Patch Changes

- 88a56ed: use faster noise crate `fastnoiselite`
- 88a56ed: use `crossbeam-channel`

## 0.38.0

### Minor Changes

- 7245e49: Add `FetchTask` for resources need to be fetch externally.

## 0.37.0

### Minor Changes

- 92f4dd8: support `opacity` property
- e6a1c39: refactor internal image/text measuring to match browser overflow behavior
- 0dfb76b: support overflow `hidden`, `visible`

## 0.36.2

### Patch Changes

- 568f76f: fix box shadow not being parsed

## 0.36.1

## 0.36.0

### Minor Changes

- 95715d0: support `filter` on images (except `blur()` and `drop-shadow()`)

## 0.35.2

### Patch Changes

- cac5444: remove glyph cache

## 0.35.1

## 0.35.0

### Minor Changes

- 264fa71: implement inline layout
- 264fa71: make all nodes' `style` field optional

### Patch Changes

- 12a2d3f: fix `aspect-ratio`, `flex-grow` numberic value parsing

## 0.34.0

### Minor Changes

- c06cdce: support `currentColor` keyword

### Patch Changes

- 7c402d8: setup npm trusted publisher

## 0.33.1

## 0.33.0

### Minor Changes

- 98755a7: **drop support for `debug` field, replace with `draw_debug_border` option in rendering functions**
- 5f15925: support `flex` shorthand property
- aa965bd: support `translate`, `rotate`, `scale` property
- 656be8d: support custom ellipsis character for `line-clamp`, `text-overflow`

### Patch Changes

- a9f3999: fix border width on image node that caused offset to be applied twice
