---
packages:
  takumi-core: minor
---

# Resolve conflicting `tw` utilities in Tailwind's order

Two `tw` utilities or `@apply` entries that set the same property now resolve as Tailwind's compiled CSS does, whatever their order. `pt-2 p-4` and `p-4 pt-2` both pad the top by `pt-2`, where before the later class won. To keep the old behavior, merge the class list first with a tool such as `tailwind-merge`.
