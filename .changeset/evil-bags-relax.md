---
"takumi": major
---

**Replaced `RenderOptionsBuilder` with `RenderOptions::builder()`**

Switch to [typed-builder](https://docs.rs/typed-builder) for compile time options validation, no unwrap needed.

Before:

```rust
let options = RenderOptionsBuilder::default().build().unwrap();
```

After:

```rust
let options = RenderOptions::builder().build();
```
