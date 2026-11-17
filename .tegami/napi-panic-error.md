---
packages:
  "@takumi-rs/core":
    type: patch
  takumi-js:
    type: patch
---

# A panic inside Takumi rejects one render instead of crashing Node.js

A bug that made Takumi's Rust code panic used to abort the whole Node.js process, ending every other request it was serving. The render that hit it now rejects with an error that starts with `Takumi panicked:`, and the process keeps running.

```ts
try {
  await renderer.render(source, options);
} catch (error) {
  // Before: the process exited, and this line never ran
  // After: "Takumi panicked: …. Please report it at https://github.com/kane50613/takumi/issues"
  console.error(error);
}
```
