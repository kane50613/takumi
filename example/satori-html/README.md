# Render satori-html output with Takumi

Convert an HTML template with `satori-html`, then render it as a PNG with `takumi-js`. Takumi also accepts HTML strings directly through `render()`.

Install [Rust](https://www.rust-lang.org/tools/install), then build the native binary from the repository root.

```bash
cd takumi-napi
bun run build
```

From `takumi-napi`, run the example:

```bash
cd ../example/satori-html
node index.ts
```

Open `output.png` to see the result.

Use a Node.js version that runs TypeScript files directly, such as Node.js 22.18 or newer. Bun currently encounters an [ultrahtml selector error](https://github.com/natemoo-re/ultrahtml/issues/66) in this example.
