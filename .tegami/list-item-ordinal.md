---
packages:
  takumi-core: minor
---

# `<ol reversed>` numbers its items down

List markers and the `list-item` counter now number as Chrome does: an `<ol reversed>` counts down from `start`, or from its item count when it has no `start`, and a box with `contain: style` numbers the list items inside it from 1. An item's `counter-set` or `counter-increment` of `list-item` moves its marker too. `start` now applies only to an `<ol>`, and `value` only to an `<li>`.
