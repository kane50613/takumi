---
name: takumi-deploy
description: Use when Takumi's backend fails to load on a runtime, bundler, or host, such as Next.js, Vite, Nitro, Cloudflare Workers, Vercel, Netlify, Docker, or `bun build --compile`. Typical signs are a render that works in development but fails in production, or an error that names a native binding, `@takumi-rs/core`, or a `.wasm` file.
---

# Deploy Takumi

`takumi-js` runs on one of two backends. Node.js and Bun load the native addon `@takumi-rs/core`, while Cloudflare Workers, edge runtimes, Deno, and browsers load the WebAssembly build `@takumi-rs/wasm`. The runtime's export conditions pick the backend, and `takumi-js` loads it without init code. `takumi-pdf` and `takumi-paint` always run on WebAssembly. Most deploy failures come from a bundler or host that drops the backend's binary, or from a second copy of it.

## Steps

1. **Name the backend production runs.** Find the runtime the route runs on in production, which can differ from the dev server's, and look up its backend in the install section of [docs.md](https://takumi.kane.tw/docs.md). Done when you can name the backend, and the only WebAssembly loading code is the `module` option from step 4 or a browser bundle's `init`, both listed in the entry table in [troubleshooting.md](https://takumi.kane.tw/docs/troubleshooting.md).

2. **Keep one version.** `takumi-js` depends on exact versions of `@takumi-rs/core` and `@takumi-rs/wasm`. List only `takumi-js` in `package.json`, or give a listed `@takumi-rs/core` the same version. Done when `npm ls @takumi-rs/core` (or `pnpm why @takumi-rs/core`) shows one version, and it matches `takumi-js`.

3. **Configure the framework.** Apply the setup step from the framework's guide in [frameworks.md](https://takumi.kane.tw/docs/frameworks.md). For Next.js, that step adds `@takumi-rs/core` to `serverExternalPackages`. With pnpm, hoist the native package as the "Cannot find native binding" section of [troubleshooting.md](https://takumi.kane.tw/docs/troubleshooting.md) shows. Done when the project's config matches the guide.

4. **Ship the binary.** A package manager installs the native package for the machine that runs the install, so a lockfile written on macOS can leave a Linux deploy without one. Install the package for the deploy target from the table in the "Install the package for your deploy target" section of [troubleshooting.md](https://takumi.kane.tw/docs/troubleshooting.md). A single-file `bun build --compile` executable leaves the addon out, so embed it as [bun-compile.md](https://takumi.kane.tw/docs/frameworks/bun-compile.md) shows. Done when the deploy artifact or the executable holds `@takumi-rs/core-<platform>` for the target.

5. **Match the error message.** Search [troubleshooting.md](https://takumi.kane.tw/docs/troubleshooting.md) for the exact message from the build or server log, and apply that fix. Done when the fix comes from that page, or the message is not on it. In that case, draft an issue for the user with the message, the runtime, the bundler, and the installed versions of `takumi-js` and `@takumi-rs/core`.

6. **Test the production build.** Development servers resolve modules differently from production bundles, so a dev render proves nothing about the deploy. Build the app, start the production server or a preview of the host, and call the code that renders. Done when the output opens as what the package makes: the expected image from `takumi-js`, the expected document from `takumi-pdf`, or a paint tree from `takumi-paint` whose boxes and text runs match the input. A route that serves it also answers status 200 with an image `content-type` or `application/pdf`.
