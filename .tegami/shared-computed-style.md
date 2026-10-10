---
packages:
  "takumi-core": minor
---

# Share a box's computed style by reference count

`RenderContext::style` is now an `Rc<ComputedStyle>` instead of a `Box<ComputedStyle>`. Reading it works as before. `RenderContextBuilder::style` still accepts a `Box<ComputedStyle>`.
