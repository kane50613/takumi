---
packages:
  "@takumi-rs/helpers": patch
---

# `StrictMode`, `Suspense`, `Profiler`, and other runtimes' fragments render their children

`fromJsx` dropped every element whose type was a symbol other than React's `Fragment`, so a tree wrapped in `<StrictMode>`, `<Suspense>`, or `<Profiler>`, or built by a JSX runtime with its own fragment symbol, rendered nothing. These elements now render their children, as the SVG serializer already did.
