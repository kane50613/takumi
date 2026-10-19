# Render an image in Rust

[`minimal::say_hello_to`](./src/minimal.rs) registers a font, builds a text node, renders a 1200 × 630 image, and writes `output.webp` to the current directory.

## Run

The example is a library function, not a binary. Check that it compiles from the repository root:

```bash
cargo check -p example
```

To render the image, call `say_hello_to` from your own code, or copy its body into a Rust application. See the [`takumi` crate example](../../takumi/README.md#example) for the rendering API.

## Fonts

Takumi does not load system fonts. The example embeds Geist with `include_bytes!` and registers it through `Fonts` and `FontResource` before rendering text.
