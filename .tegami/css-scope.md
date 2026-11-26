---
packages:
  takumi-core: minor
---

# Support `@scope`

Style rules in `@scope` apply only from the elements its root selector matches down to, but not into, the ones its `to (…)` limit matches, as in Chrome. `:scope` matches the root, declarations written directly in the rule apply to the root, and a rule from a closer root wins over an equally specific one from a farther root or from no scope. Nested in a style rule, its root selector is relative to the rule's elements, and `&` stands for them. Nested in another `@scope`, its root selector is relative to that scope's root. Without a root selector, it roots at the parent of the `<style>` element that holds it or the `<link>` element that loads it, nested or not.

```css
.list {
  @scope (& > .card) to (.footer) {
    padding: 8px;
    p {
      margin: 0;
    }
  }
}
```
