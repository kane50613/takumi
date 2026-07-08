## takumi-svg@0.1.4

### Fix `fontFamilies` order being ignored

`fontFamilies` only fed the fallback bucket, never the root style, so text
picked whichever registered font resolved first instead of the requested
order. `FontFamily`'s default is now empty instead of a generic `sans-serif`
token, so an empty root style falls through to the fallback bucket directly.

## takumi-svg@0.1.0

### Render backdrop-filter in the SVG backend

SVG has no native backdrop source, so the backdrop is the scene replayed up to
the element, run through an SVG `<filter>` chain, then clipped to the border
box and attenuated by the element's mask and clip-path. Adds
`ComputedStyle::has_shape_mask` and `Filter::is_drop_shadow`.

## takumi-svg@0.1.0-rc.3

### Render backdrop-filter in the SVG backend

SVG has no native backdrop source, so the backdrop is the scene replayed up to
the element, run through an SVG `<filter>` chain, then clipped to the border
box and attenuated by the element's mask and clip-path. Adds
`ComputedStyle::has_shape_mask` and `Filter::is_drop_shadow`.

## takumi-svg@0.1.0-beta.5

### Fix `path()` ignoring device-pixel-ratio

`offset-path: path()` and `clip-path: path()` used the authored SVG coordinates verbatim while every other basic shape crossed the dpr boundary through `to_px`. At `devicePixelRatio != 1` the path was mis-scaled by `1/dpr`. The coordinates are now scaled into device space like the other shapes.

## takumi-svg@0.1.0-beta.2

### Render text language-aware with the `lang` attribute

The `lang` attribute sets a node's language and inherits to its descendants; a
nested `lang` overrides, and an empty or invalid value clears it. The language
reaches the shaper as the locale, so the same code point draws its
language-specific glyph (Han unification across `ja` / `zh` / `ko`) and lines
break per language. Pass `lang` in the render options to set a default for the
whole tree.

## takumi-svg@0.1.0-beta.1

### Match browser fake-bold for synthesized weights

Synthesize bold only at the CSS bold threshold (weight >= 600), so a medium weight keeps its regular face instead of being faux-bolded. Scale the synthetic stroke with text size — `1/24` easing to `1/32` per Skia — instead of a constant fraction, so large text is no longer over-emboldened.

## takumi-svg@0.1.0-beta.0

### Stop resolving `currentColor` in SVG images against the host color
