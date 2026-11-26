---
packages:
  takumi-core: minor
---

# Support `revert`, `revert-layer`, and `revert-rule`

The three keywords roll a property back to a weaker declaration, as in Chrome, or to `unset` without one:

- `revert` rolls back to the browser's default style.
- `revert-layer` rolls back to the cascade layer below, whatever the declaration's importance.
- `revert-rule` rolls back to the rule below, past both importance levels of its own rule.

They work on custom properties and in `all` too.

```css
@layer base {
  .card {
    color: navy;
  }
}
.card {
  color: red;
}
.card.quiet {
  color: revert-layer; /* navy */
}
```
