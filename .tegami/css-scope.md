---
packages:
  takumi-core: minor
---

# Support `@scope`

Style rules in `@scope` apply only from the elements its root selector matches down to, but not into, the ones its `to (…)` limit matches, as in Chrome. `:scope` matches the root, and a rule from a closer root wins over an equally specific one from a farther root or from no scope. `@scope` inside a style rule is not read yet.
