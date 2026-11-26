---
packages:
  takumi-core: patch
  "@takumi-rs/helpers": patch
---

# Element language and JSX `<select>` follow Chrome and React DOM

- An SVG element with both `xml:lang` and `lang` takes its language from `xml:lang`, in either order, as Chrome does.
- `:lang()` matches the render's `lang` option when no ancestor sets `lang`, as Chrome matches the document's language.
- A JSX `<select>` with `value` or `defaultValue` marks the options it names as `selected`, as React DOM's server renderer does.

```tsx
<select defaultValue="b">
  <option value="a">A</option>
  <option value="b">B</option>
</select>
// Before: neither option has `selected`, so `option:checked` matches A
// After: B has `selected`, so `option:checked` matches B
```
