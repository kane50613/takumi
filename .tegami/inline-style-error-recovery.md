---
packages:
  takumi-core:
    type: minor
  takumi-html:
    type: patch
---

### Keep the rest of a `style` attribute when one declaration fails

A value this crate cannot read, such as `width: fit-content`, discarded every other declaration in the same `style` attribute. It now invalidates only itself, which is the recovery CSS asks for and what a `<style>` sheet already did.
