---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
---

### Parse `className` as Tailwind utilities behind `future.classNameUtilities`

The flag feeds `className` tokens to the `tw` parser, so Tailwind JSX pasted from an app renders without a build step. A token that is not a utility still matches stylesheet selectors, and `tw` wins a tie on the same node. The flag becomes the default in the next major.
