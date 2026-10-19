# Render a Svelte component as an image

A SvelteKit route that server-renders a Svelte component to HTML, then returns it as a 1200 × 630 image with `ImageResponse` from `takumi-js`.

## Run

Build the [image example packages](../README.md#image-examples) first. Then start SvelteKit with [portless](https://github.com/vercel-labs/portless):

```bash
cd example/svelte
portless run --name takumi-svelte vite dev
```

Open `https://takumi-svelte.localhost/?name=Kane` to see the image.

## Notes

- The route is [`src/routes/+server.ts`](./src/routes/+server.ts). It passes the component's HTML and the compiled Tailwind CSS from `app.css` to `ImageResponse`.
- The route loads Geist from `takumi.kane.tw`, so rendering needs network access.
- [`vite.config.ts`](./vite.config.ts) marks `@takumi-rs/core` as an SSR external, so Vite loads the native addon from `node_modules`.
