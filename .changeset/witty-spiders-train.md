---
"@takumi-rs/core": major
---

**`renderer.putPersistentImage()` now takes `ImageSource`**

Before:

```tsx
const data = await readFile("foo.png");
await renderer.putPersistentImage("foo.png", data);
```

After:

```tsx
const data = await readFile("foo.png");
await renderer.putPersistentImage({
  src: "foo.png",
  data,
});
```
