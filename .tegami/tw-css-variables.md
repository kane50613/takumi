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

Utilities now read the CSS variables Tailwind compiles them to, falling back to the built-in value. Define tokens in `:root` or through the `cssVariables` option. `--color-brand-500` makes `bg-brand-500` work, and spacing, fonts, shadows, animations and breakpoints follow the same rule.

Gradients now match Tailwind on two counts. Stops alone no longer paint without `bg-linear-*`, `bg-radial` or `bg-conic`, and a missing `to` stop fades to `transparent`.

### Let stylesheet rules win over `tw` utilities

Utilities now sit in the last cascade layer, below unlayered CSS and above rules in a named `@layer`. Important utilities mirror that. They lose to important rules in a layer and beat unlayered ones, while inline important declarations stay on top. A template that relied on `tw` beating a matching rule needs a fix. Move that rule into a layer, or mark the utility `!`.
