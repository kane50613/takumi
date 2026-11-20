---
packages:
  takumi-core: minor
  takumi: minor
  takumi-cli: minor
---

# `@media (prefers-color-scheme)` matches, and `takumi --color-scheme dark` renders the dark variant

`prefers-color-scheme` used to match nothing. It now matches `light` by default, and `dark` when the viewport asks for it, so a page that keeps its dark colors in a `prefers-color-scheme: dark` rule renders them.

```sh
takumi image email.html -o email-dark.png --width 600 --color-scheme dark
```

```rust
let viewport = Viewport::new((600, 400)).with_color_scheme(ColorScheme::Dark);
```

- `takumi image` and `takumi paint` take `--color-scheme light` or `dark`.
- In Rust, `Viewport::with_color_scheme` sets it, and `ColorScheme` is in the prelude. `Viewport` gains the `color_scheme` field, so a `Viewport { .. }` literal needs it. `Viewport::new` and its `with_*` methods need no change.
- A rule under `prefers-color-scheme: light` now applies by default, as in a browser.
- `light-dark()` still takes its first color.
