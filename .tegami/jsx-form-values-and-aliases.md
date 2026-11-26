---
packages:
  "@takumi-rs/helpers": patch
---

# JSX writes form values and attribute names as React DOM does

Elements from JSX now carry the attributes and text React DOM's server renderer writes for them. Check selectors and styles that read these attributes:

- An `<input>` writes `defaultChecked` as `checked` and `defaultValue` as `value` when the controlled prop is absent.
- A `<textarea>` shows its `value` or `defaultValue` as its text.
- `<select>` and `<textarea>` no longer write `value` as an attribute.
- `suppressContentEditableWarning` and `innerHTML` are no longer written as attributes.
- Props React DOM renames, such as `strokeWidth`, `crossOrigin`, `xlinkHref`, and `autoFocus`, are written as `stroke-width`, `crossorigin`, `xlink:href`, and `autofocus`, in HTML elements and in SVG.

```tsx
<input type="checkbox" defaultChecked />
// Before: `:checked` and `checked:` did not match it
// After: they match
```
