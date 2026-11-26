---
packages:
  takumi-core: minor
---

# Tailwind rings, stacked shadows, corner gradients, and form state variants

`ring`, `ring-<width>`, `ring-<color>`, `ring-inset`, `ring-offset-<width>`, `ring-offset-<color>`, `inset-ring`, `inset-ring-<width>`, `inset-ring-<color>`, `inset-shadow-<color>`, and `inset-shadow-[…]` now work. Shadow, inset shadow, ring, and inset ring utilities stack in one `box-shadow` in Tailwind's order instead of replacing each other.

`bg-linear-to-tr` and the other corner directions point at the box's corner as `to top right` does, instead of at a fixed 45° angle. Gradients read `/srgb`, `/oklch`, `/longer`, and the other interpolation modifiers, and `bg-linear-[…]`, `bg-radial-[…]`, and `bg-conic-[…]` take an arbitrary position. A bare `drop-shadow` draws both of Tailwind's layers.

`checked:`, `indeterminate:`, `placeholder-shown:`, `optional:`, `required:`, `read-only:`, `enabled:`, `disabled:`, `aria-*:`, `aria-[…]:`, `data-*:`, and `data-[…]:` now apply, as do their `not-` forms. `not-` also negates breakpoints and media variants, so `not-md:`, `not-dark:`, `not-print:`, and `not-motion-safe:` apply where the variant does not.

```html
<div tw="shadow-lg ring-2 ring-sky-500 ring-offset-2 bg-linear-to-tr from-indigo-600 to-fuchsia-500"></div>
<!-- Before: the ring classes were dropped, and the gradient ran at 45° -->
<!-- After: the ring draws around the shadow, and the gradient runs toward the top right corner -->
```
