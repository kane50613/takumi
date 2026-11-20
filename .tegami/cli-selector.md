---
packages:
  takumi-cli: minor
  takumi-raster: minor
---

# `takumi image --selector` renders one element at its own size

`--selector` keeps only the first element it matches and crops the image to that element's box, so a card or a slide renders without the page around it.

```sh
takumi image page.html -o pricing.png --selector .pricing-card
```

- It takes a type, `#id`, and `.class` written together, such as `section.pricing-card`.
- It writes PNG, JPEG, or WebP. An SVG output with `--selector` exits with code `2`.
- No match fails with `no element matches`.
- In Rust, `Bitmap::crop` is now public, and returns `None` for an empty region or one that does not fit the bitmap.

```rust
let bitmap = Bitmap::from_raw(320, 256, vec![0; 320 * 256 * 4]).ok_or("invalid bitmap data")?;
let card = bitmap.crop(40, 120, 280, 136).ok_or("the card lies outside the image")?;
```
