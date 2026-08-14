---
packages:
  takumi-pdf:
    type: minor
---

### Render from a Next.js route without configuring the bundler

Turbopack bundles a server route's imports, and it resolved `takumi-pdf` to the Vite entry, whose `?url` import only Vite reads. The build failed unless the package was listed in `serverExternalPackages`. `takumi-pdf/next` hands Turbopack the binary in the form it emits, on the Node runtime and the Edge runtime alike.
