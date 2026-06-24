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
