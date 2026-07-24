---
packages:
  "cargo:takumi-core": patch
---

### Bound calc() depth, var() substitution size, and list interpolation length

Four places in the CSS value layer let caller-supplied text drive unbounded recursion or allocation. `calc()` recursed once per leading unary sign and once per nested `calc(`. `var()` substitution capped neither its nesting nor its total substituted bytes; its cycle guard is pushed and popped per reference, so it stops a property referencing itself but not fan-out, and `--n: var(--n-1)var(--n-1)` doubles per link. `RepeatToLcm` list interpolation allocated the full LCM of the two list lengths. A `calc()` resolving to NaN or infinity reached taffy unclamped, where every other `Length` arm is clamped on its way through `to_px`.

Release builds abort on panic, so a stack overflow or a failed allocation here took down the host process instead of returning an error. The limits match Blink: depth 100, 2 MiB of substituted text (the value the spec and Firefox use as well), and 1000 interpolated list entries.
