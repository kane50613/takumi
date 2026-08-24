---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-pdf":
    type: minor
---

### Customize the `tw` prop's design tokens

The built-in Tailwind parser only knew Tailwind's own scales, so a project's brand color or spacing step had to be spelled as an arbitrary value on every element. Renders now take a `theme` option keyed by CSS custom property name: `--color-brand-500` makes `bg-brand-500` work, `--spacing-gutter` makes `p-gutter` work, and naming a built-in token such as `--color-red-500` replaces it.

Fifteen namespaces are wired up, covering colors, spacing, containers, type, radii, shadows, blur, aspect ratio and animations. A value of `initial` removes a token, and `--color-*: initial` clears the namespace along with the built-in palette underneath it.
