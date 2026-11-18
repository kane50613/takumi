---
packages:
  "@takumi-rs/helpers": patch
---

# Editors flag the unit helpers and the fetch helpers that leaked from `@takumi-rs/helpers`

These exports keep working and will be removed in v3:

- `percentage`, `vw`, `vh`, `em`, `rem`, `fr`, and `rgba`. Write the CSS string instead.
- `style`. Type the object as React's `CSSProperties` instead.
- `fetchOk`, `readBodyLimited`, and `defaultMaxFetchBytes`. Call `fetch`, or set the `maxBytes` fetch option.
- `collectCodepoints`, `LIST_MARKER_CHARACTERS`, and `CodepointSource`. A render picks the font subsets its text uses.

#### Migration

```diff
- style={{ width: percentage(50), color: rgba(0, 0, 0, 0.5) }}
+ style={{ width: "50%", color: "rgb(0 0 0 / 0.5)" }}
```
