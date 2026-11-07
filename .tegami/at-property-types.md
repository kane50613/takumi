---
packages:
  takumi-core: minor
---

# `@property` types its custom property

A custom property that `@property` registers now computes to the type its `syntax` names, inherits that computed value, and animates through it. A `<length>` computes to px, an `<angle>` to deg, a `<color>` to its color, and so on, so a `@keyframes` rule can spin a conic gradient through a registered `<angle>`. A declared value the syntax does not take falls back to the inherited or initial value, and a rule whose `syntax` or `initial-value` Chromium rejects is dropped. `PropertyRule::syntax` is now a parsed `PropertySyntax`.
