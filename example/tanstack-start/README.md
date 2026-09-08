# TanStack Start Example

Return an Open Graph image from a TanStack Start server route using `takumi-js` and the Cloudflare Vite plugin.

Install [Rust](https://www.rust-lang.org/tools/install) and the workspace dependencies, then build from the repository root:

```bash
bun install
bun --filter '*' run build
```

Start the example with [portless](https://github.com/vercel-labs/portless):

```bash
cd example/tanstack-start
portless run --name takumi-tanstack vite dev
```

Open https://takumi-tanstack.localhost/ to see the generated image.
