# takumi

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
