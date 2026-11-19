---
packages:
  takumi-core: minor
  takumi: minor
  takumi-cli: minor
  takumi-paint: minor
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  takumi-pdf: minor
---

# The paint tree lists the CSS a render dropped

A declaration or at-rule that takumi drops no longer vanishes without a trace. The paint tree lists each one once, with its property and value as written, why takumi dropped it, and where it was written. The image stays the same.

```ts
const tree = await paint(html, { width: 600, height: 200 });

console.log(tree.ignoredDeclarations);
// [{ property: "writing-mode", value: "vertical-rl", reason: "unknown-property", origin: "style-attribute" }]
```

- `reason` is `unknown-property`, `invalid-value`, or `unknown-at-rule`.
- `origin` is `style-attribute`, `style-sheet`, or `style-object`.
- `takumi paint` prints the list under `ignored`, and its JSON output holds it as `ignoredDeclarations`.
- `needs()` in `@takumi-rs/core`, `@takumi-rs/wasm`, and `takumi-pdf` returns the same list as `ignoredDeclarations`.
- In Rust, `RenderNeeds::ignored_declarations` and `PaintDocument::ignored_declarations` hold the same list, and `StyleSheet::ignored_declarations` holds what one style sheet dropped.
