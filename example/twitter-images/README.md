# Generate Takumi social images

This workspace renders the social cards and animation frames used in the README and documentation website. Templates live in `components/`. The render list, output formats, and filenames are defined in [`index.tsx`](./index.tsx).

## Run

Install dependencies and build workspace packages from the repository root:

```bash
bun install
bun --filter '*' run build
```

Render the images:

```bash
cd example/twitter-images
bun index.tsx
```

The script writes to `output/`. It generates PNG and WebP variants for static templates and WebP frames for templates with timestamps. Pass `--debug` to draw node outlines.

Review generated images before committing them. Some templates load remote assets, so rendering may need network access.
