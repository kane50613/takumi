# Next.js Example

This is a simple example of how to use `takumi-js` in Next.js.

Before running the example, you need to build the native binary.

Make sure you have [Rust installed](https://www.rust-lang.org/tools/install).

```bash
bun --filter '*' run build
```

Then, run the development server.

```bash
cd example/nextjs
bun dev
```

Visit http://localhost:3000/hello?name=Kane to see the result.
