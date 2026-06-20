---
"takumi": major
"takumi-core": minor
"takumi-raster": minor
---

Minimize the public API: expose stable data structures via `takumi::prelude` and entry-point functions at the crate root, move the full backend crates behind a new `unstable` feature, and demote backend-internal items to `pub(crate)`
