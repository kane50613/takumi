---
packages:
  takumi-core: minor
---

# A `font-family` whose first family never loads now warns

When no loaded face provides the first family a `font-family` names, takumi logs a warning that names it. The `takumi` CLI prints it and fails on it under `--deny-warnings`. The JS and WASM bindings keep their own warning for a family no font provides. Generic families, `-apple-system`, and `BlinkMacSystemFont` never warn, and missing later families stay at debug level.

```sh
$ takumi image card.html -o card.png
warning: no face is loaded for font-family "Playfair Display", so text falls back to the next family that has one. Load the font, or check the name in font-family.
```
