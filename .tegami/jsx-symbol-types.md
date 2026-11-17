---
packages:
  "@takumi-rs/helpers": patch
---

# `fromJsx` renders what `StrictMode`, `Suspense`, and `Profiler` wrap

A tree wrapped in `<StrictMode>`, `<Suspense>`, or `<Profiler>` rendered nothing through `fromJsx`. So did a tree built by a JSX runtime whose fragment is not React's `Fragment`. These elements now render their children, as `<>` already did.
