---
packages:
  takumi-core:
    type: patch
---

### Replace the unmaintained `paste` with `pastey`

`paste` has been unmaintained since 2024, and `cargo audit` reports RUSTSEC-2024-0436 for it. `pastey` is a maintained fork with the same macro surface, so the generated code is unchanged.
