# Svelte Example

Render a Svelte component and its CSS as an image with `takumi-js`.

Install [Rust](https://www.rust-lang.org/tools/install) and the workspace dependencies, then build from the repository root:

```bash
bun install
bun --filter '*' run build
```

Start the example with [portless](https://github.com/vercel-labs/portless):

```bash
cd example/svelte
portless run --name takumi-svelte vite dev
```

Open https://takumi-svelte.localhost/ to see the generated image.
