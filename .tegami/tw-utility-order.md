---
packages:
  takumi-core: minor
---

# Resolve conflicting `tw` utilities in Tailwind's order

Two `tw` utilities that set the same property now resolve as Tailwind's compiled CSS does, whatever their class order. A utility for one side beats one for every side, so `pt-2 p-4` and `p-4 pt-2` both pad the top by `pt-2`. Before, the later class won. To keep that behavior, merge the class list first with a tool such as `tailwind-merge`.

`@apply` expands its utilities in the same order.
