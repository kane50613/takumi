# Next.js Example

Generate an Open Graph image from a Next.js App Router route with `takumi-js`. Change the `name` query parameter to update the card.

Install [Rust](https://www.rust-lang.org/tools/install) and the workspace dependencies, then build from the repository root:

```bash
bun install
bun --filter '*' run build
```

Start the example with [portless](https://github.com/vercel-labs/portless):

```bash
cd example/nextjs
portless run --name takumi-nextjs next dev
```

Open https://takumi-nextjs.localhost/?name=Kane to see the generated image.
