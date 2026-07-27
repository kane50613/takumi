---
packages:
  "cargo:takumi": patch
---

### Export `FontSource` from the prelude

`FontResource::new` takes anything that converts into a `FontSource`, and naming that type is the only way to reach `FontSource::from_static` or `from_shared`. It was missing from the prelude, so registering an `include_bytes!` face meant going through the semver-exempt `unstable` module.
