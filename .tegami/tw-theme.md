---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-pdf":
    type: minor
---

### Resolve `tw` utilities through theme variables

The built-in Tailwind parser burned its own scales into every utility, so a project's brand colour or spacing step had to be spelled as an arbitrary value on every element. A utility now reads a custom property the way Tailwind compiles it: `bg-red-500` resolves `var(--color-red-500)` and `p-4` resolves `calc(var(--spacing) * 4)`, with the built-in value behind them. Define the variable in a `:root` rule or pass a `variables` option, and `--color-brand-500` makes `bg-brand-500` work while `--color-red-500` replaces the built-in red.

Because the value comes from the cascade, a token redefined under `@media (prefers-color-scheme: dark)` or a `[data-theme]` selector takes effect like any other custom property.

### Let a stylesheet rule win over a `tw` utility

The `tw` prop sat above every stylesheet rule, so `#hero { background: blue }` lost to `tw="bg-red-500"` even though an id selector outranks a class. Utilities now sit where Tailwind declares them: the last cascade layer, below unlayered CSS and above rules in a named `@layer`. So Preflight wrapped in `@layer base` resets browser defaults without beating utilities. A utility marked `!` moves to the other end where it beats an important stylesheet rule.

Templates that relied on `tw` overriding a matching `className` rule need the rule moved into a cascade layer, or the utility marked with `!`.
