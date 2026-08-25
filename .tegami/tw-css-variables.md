---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-pdf":
    type: minor
---

### Resolve `tw` utilities through CSS variables

Utilities now read the CSS variables Tailwind compiles them to, with the built-in value as the fallback. Define tokens in `:root` or the `cssVariables` option; `--color-brand-500` makes `bg-brand-500` work.

- Colours: `bg-*`, `text-*`, `border-*`, gradient stops (`from-brand-500`), and shadow colours (`shadow-brand-500` through `--tw-shadow-color`).
- Lengths: `p-4` reads `calc(var(--spacing) * 4)`, `p-gutter` reads `--spacing-gutter`; same for `max-w-*`, `rounded-*`, `tracking-*`, `leading-*`, `aspect-*`. Text sizes read `--text-*`: `text-xl` reads `--text-xl`, while `text-red-500` is a colour under `--color-*`.
- Fonts: `font-sans` reads `--font-sans`, `font-bold` reads `--font-weight-bold`.
- Shapes: `blur-md`, `drop-shadow-md`, `shadow-md`, `inset-shadow-sm` and `text-shadow-sm` read their `--blur-*` / `--drop-shadow-*` / `--shadow-*` / `--inset-shadow-*` / `--text-shadow-*` tokens. A custom shadow shape carries its own colours, so shadow colour utilities only reach the built-in fallback.
- Animations: `animate-spin` reads `var(--animate-spin)`, and an unknown token like `animate-wiggle` reads `var(--animate-wiggle)` alone; pair it with its `@keyframes`.
- Breakpoints: an unconditional `:root` `--breakpoint-*` declaration re-sizes the `sm:`–`2xl:` variants and defines new ones like `3xl:`. Variants gate before the cascade, so a media query cannot move them.

Two gradient behaviours move to match Tailwind: stops alone no longer paint without `bg-linear-*`, `bg-radial` or `bg-conic`, and a missing `to` stop fades to `transparent`.

### Let stylesheet rules win over `tw` utilities

Utilities now sit in the last cascade layer: below unlayered CSS, above rules in a named `@layer`. Templates that relied on `tw` beating a matching rule need the rule moved into a layer, or the utility marked `!`.
