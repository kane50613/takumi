# takumi

## 0.68.9

### Patch Changes

- 2016a3c: fix nested inline-block caused infinite recursive calls

## 0.68.8

### Patch Changes

- b7d7570: fix box-shadow rendering
- 4b13519: support `overflow: clip`

## 0.68.7

### Patch Changes

- 9248c6e: support `inline-block`, `inline-flex`, `inline-grid` layout, closes #219
- 2639396: fix decoration rounding caused gap
- 91037be: fix style shorthand overrides

## 0.68.6

### Patch Changes

- 51a5bd5: support `text-decoration-skip-ink` property
- 642cf06: refactor `TextDecorationLines` to use `bitflags`, parses Tailwind `underline`, `overline`...
- 57d6594: fix text painting order for shadow, decoration and actual content to avoid overlapping
- 9215906: fix embolden and skew should avoid emojis
- 53069e4: refactor taffy tree structure, support `calc()`

## 0.68.5

### Patch Changes

- 8fcbb90: support `font-synthesis-weight` & `font-synthesis-style` properties for faux-bold & skew
- 07649be: fix text overline decoration should be drawn before text

## 0.68.4

### Patch Changes

- d6f1cce: use `libm` for text rendering
- 37864fd: support `outline` properties

## 0.68.3

### Patch Changes

- 585e1ba: support `background: conic-gradient()`
- 755b998: support `font-stretch` property
- 12ef8ce: loosen SVG check
- db327e6: fix overflow hidden constraint should include border radius mask
- 133c6a4: fix line height "normal" behavior
- 3191865: fix inheritance should store computed instead of initial value

## 0.68.2

## 0.68.1

### Patch Changes

- fb32121: parse `bg-none` for Tailwind `background-image` and improve isolation coverage

## 0.68.0

### Minor Changes

- 7684faa: refactor font loading to reduce buffer copying

## 0.67.3

### Patch Changes

- 0632470: unpremultiply SVG pixels after rasterization #446
- 1e9b23e: fix plus-lighter compositing #447

## 0.67.2

## 0.67.1

### Patch Changes

- 4d8955d: fix text wrap ignores original height & line clamp constraint #439

## 0.67.0

### Minor Changes

- 691df9d: support `mix-blend-mode` and `isolation` property

### Patch Changes

- 7e34727: support `visibility` property
- ba7aa93: fix prevent text balancer from forcing breaks #437

## 0.66.14

### Patch Changes

- 102c24a: include DPR when calculating `text-wrap: balance` #434

## 0.66.13

## 0.66.12

### Patch Changes

- 7389d6e: add font loading cache

## 0.66.11

### Patch Changes

- 8f28434: refactor inline box drawing & measure API

## 0.66.10

### Patch Changes

- e457042: fix line height should be resolved absolute value

## 0.66.9

### Patch Changes

- c917b6d: add `parse_tw_with_arbitrary` method
- 9929474: fix missing Tailwind `leading-*` keywords

## 0.66.8

### Patch Changes

- 28a8348: merge `draw_background_color` & `draw_background_image` implementation

## 0.66.7

## 0.66.6

### Patch Changes

- 2e7dbed: support `strokeLinejoin` property for text stroke

## 0.66.5

### Patch Changes

- c8244eb: fix text clip drawing order

## 0.66.4

### Patch Changes

- 5f2b5ac: add `WebkitTextFillColor` property
- 7eed4a1: support background clipping on text stroke

## 0.66.3

### Patch Changes

- 77c7107: fix `WebkitTextStroke` deserialize naming

## 0.66.2

### Patch Changes

- 058f87a: use `zune-core` & `zune-jpeg` dev branch version (not published to crates.io)

## 0.66.1

### Patch Changes

- 7e513fc: add `text-shadow-*`, `drop-shadow-*` tailwind properties parser

## 0.66.0

## 0.65.0

### Minor Changes

- 1319540: new `measure()` API

## 0.64.1

## 0.64.0

### Patch Changes

- 6571216: fix viewport check should include defined values

## 0.63.2

### Patch Changes

- 63088f4: make `background_color` field optional, draw background color on text spans #220

## 0.63.1

## 0.63.0

## 0.62.8

### Patch Changes

- b0a21a4: refactor opacity blending should be on render level

## 0.62.7

## 0.62.6

### Patch Changes

- a10f933: fix tailwind filter classes (blur, brightness, etc.) now append instead of replace

## 0.62.5

### Patch Changes

- dd1c0e1: fix tailwind `text-pretty` & `text-balance` not being parsed

## 0.62.4

## 0.62.3

## 0.62.2

### Patch Changes

- 57cca21: Improve backdrop filter performance
- 520f15d: Improve drop shadow performance and reduce allocation

## 0.62.1

### Patch Changes

- 5214274: refactor `overlay_image` to take any `GenericImageView` (avoid unnecessary `RgbaImage` recreation)

## 0.62.0

### Minor Changes

- 4675458: use `Box` slices instead of `Vec` to optimize memory

### Patch Changes

- 7849598: SIMD enhanced `interpolate_rgba`
- a774aa6: optimize filters to render using LUTs

## 0.61.1

### Patch Changes

- 19235dd: support AVX2 & AVX-512 SIMD blurring
- 8066f93: bump MSRV to 1.89

## 0.61.0

### Minor Changes

- c4bf981: enrich CSS error

  The error message is much more helpful now.

  > InvalidArg, invalid type: integer `123`, expected a value of 'currentColor' or \<color>; also accepts 'initial' or 'inherit'.

- 98e9254: support `backdrop-filter`

## 0.60.8

### Patch Changes

- 4c6bf92: fix text drawing bypasses overflow constrain check

## 0.60.7

### Patch Changes

- f07b7f5: switch to gaussian box blur, integer based alpha blending

## 0.60.6

### Patch Changes

- 7813b86: use bit masking for faster alpha quantiazation

## 0.60.5

### Patch Changes

- 12415ba: fix alpha blending precision

## 0.60.4

### Patch Changes

- 6f74c75: fix `try_collect_palette` collecting over 256 colors

## 0.60.3

### Patch Changes

- 5e1cb26: try collect png palette if possible

## 0.60.2

### Patch Changes

- 946fc9e: update ellipsis condition explicity check `text-overflow: ellipsis`

## 0.60.1

### Patch Changes

- 71ab744: Unify text node & inline logic

  Brings more unified and consistent ellipsis, transform, collapse, measurement behavior.

## 0.60.0

### Minor Changes

- ef3ec72: support `text-wrap: balance` & `pretty` (`text-wrap-style`)!

## 0.59.1

### Patch Changes

- c6b4eab: use stack blur algorithm
- 8f02159: add `sepia()` filter, tailwind `filter` parsers

## 0.59.0

### Minor Changes

- 13eca0e: rename `LengthUnit` to `Length` #347
- 4dee0c0: support `blur()`, `drop-shadow()` filter, premultiply alpha blending for shadows

## 0.58.0

### Minor Changes

- 0deafbd: decouple base Chromium styles (or customized from `defaultStylePresets`) from `style` field to independent `preset` field.

## 0.57.6

### Patch Changes

- 68e8fc2: fix inline style order should be greater than tailwind styles

## 0.57.5

### Patch Changes

- 9bf3333: disable font hinting, apply normalized coordinates to glyph scaler

## 0.57.4

### Patch Changes

- a8ebbba: remove redundant style property wrapper
- a8ebbba: fix `matrix()` function parsing
- a8ebbba: support `col`, `row` tailwind grid properties

## 0.57.3

### Patch Changes

- fa2f034: fix COLR layers blending

## 0.57.2

### Patch Changes

- 695f34a: fix passing opacity to COLR palette

## 0.57.1

### Patch Changes

- 61191b2: handles `background-size` for rasterized images
- 260dbd0: optimize `encode_animated_webp` to reduce allocation

## 0.57.0

### Minor Changes

- 42572bb: **Drop `avif` format support**

### Patch Changes

- 26173c5: add `create_background_image` fast path for exact one image

## 0.56.1

### Patch Changes

- f4d54fa: fix `opacity` should be applied to image as well
- 1972df9: fix `background-size` css parsing
- 1972df9: support `background`, `mask` shorthand

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
