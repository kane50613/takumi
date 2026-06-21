# satori-html

This is a example of how to use satori-html with Takumi.

Before running the example, you need to build the native binary.

Make sure you have [Rust installed](https://www.rust-lang.org/tools/install).

```bash
cd takumi-napi
bun run build
```

Then, run the code.

```bash
cd example/satori-html
node index.ts # Bun fails https://github.com/natemoo-re/ultrahtml/issues/66
```

Open `output.png` to see the result.
