# Render an image in Rust

[`minimal::say_hello_to`](./src/minimal.rs) loads a bundled font, builds a text node, renders a 1200 × 630 image, and writes `output.webp` in the caller's working directory.

This example is a library function, not a command-line binary. Use its source as a starting point for a Rust application, or see the [`takumi` crate example](../../takumi/README.md#example) for the rendering API.

Check that the example compiles from the repository root:

```bash
cargo check -p example
```

Takumi does not load system fonts. Register font bytes before rendering text, as the example does with `Fonts` and `FontResource`.
