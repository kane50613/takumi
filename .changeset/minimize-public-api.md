---
"takumi": major
"takumi-base": minor
"takumi-raster": minor
---

Minimize the public API: curate the `takumi` facade into stable `base`/`raster`/`svg` modules and move the full backend crates behind a new `unstable` feature; demote backend-internal items to `pub(crate)`
