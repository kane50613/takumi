## takumi-css@0.2.0-beta.4

### Support `text-underline-offset`

Add the `text-underline-offset` property, accepting `auto` or a `<length-percentage>` that shifts the underline away from the text. Percentages resolve against `1em`. Applies to the raster and SVG backends.

### Support `font-variant` properties

Add `font-variant` and its `font-variant-ligatures`, `font-variant-numeric`, `font-variant-east-asian`, `font-variant-caps`, and `font-variant-position` longhands. Each maps to OpenType features and resolves before `font-feature-settings`, which still wins on a tag conflict. `font-variant-alternates` and `font-variant-emoji` are out of scope, and missing features are not synthesized.

### Fix `path()` ignoring device-pixel-ratio

`offset-path: path()` and `clip-path: path()` used the authored SVG coordinates verbatim while every other basic shape crossed the dpr boundary through `to_px`. At `devicePixelRatio != 1` the path was mis-scaled by `1/dpr`. The coordinates are now scaled into device space like the other shapes.

### Support `background-origin`

Add the `background-origin` property (`border-box`, `padding-box`, `content-box`), which sets the area that `background-position` and `background-size` resolve against. The `background` shorthand reads `<box>` values: the first sets origin and clip, a second overrides clip.

The initial value is `padding-box`, matching CSS, so backgrounds on bordered boxes now position against the padding box instead of the border box.

## takumi-css@0.2.0-beta.3

### Add CSS `offset-path` support

The path accepts `ray()`, the basic shapes `path()`/`circle()`/`ellipse()`/`polygon()`/`inset()`,
and a bare `<coord-box>`

`offset-distance`, `offset-rotate`, `offset-anchor`, `offset-position`, and the `offset` shorthand control placement.

`url()` references are not supported.

### Route device-pixel-ratio conversion through one boundary

Device pixels are the canonical unit and `viewport.size` is the source of truth.
`Viewport` and `SizingContext` now expose `to_device`/`to_css` as the only place
the ratio is applied, replacing ad-hoc multiplications across `to_px`, `calc()`,
font-relative collapses, tailwind breakpoints, and image intrinsic sizes.
Rendered output is unchanged.

## takumi-css@0.2.0-beta.2

### Apply `fontFamilies` as the default font

Text without an explicit `font-family` ignored the render's `fontFamilies` (and
any fonts passed through it), always using the embedded fallback. The
`fontFamilies` stack is now the root default, so passed fonts render without a
per-node `font-family`.

## takumi-css@0.2.0-beta.1

### Render text language-aware with the `lang` attribute

The `lang` attribute sets a node's language and inherits to its descendants; a
nested `lang` overrides, and an empty or invalid value clears it. The language
reaches the shaper as the locale, so the same code point draws its
language-specific glyph (Han unification across `ja` / `zh` / `ko`) and lines
break per language. Pass `lang` in the render options to set a default for the
whole tree.

## takumi-css@0.2.0-beta.0

### Align CSS handling with the spec

Fix the `border-*-width`/`outline-width` defaults, negative `scale`, the `position: static` default, and `line-clamp` longhand splitting.

# takumi-css

## 0.1.2

### Patch Changes

- b998cfd: Fix Tailwind per-side border colors and border-width rendering
- 1389d75: Support `safe`/`unsafe` overflow keywords on `align-items` and `justify-content` (taffy 0.11)

## 0.1.1

### Patch Changes

- 72ee5dd: Fix vendor-prefixed property names not being resolved
