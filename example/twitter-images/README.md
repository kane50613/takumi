# Generate Takumi's social images

Render the social cards, animations, and sample PDFs used in the Takumi README and documentation site. The templates live in [`components/`](./components).

## Run

Build both the [image example packages](../README.md#image-examples) and the [PDF example packages](../README.md#pdf-examples) first. Run each script from this directory:

```bash
cd example/twitter-images
```

Some templates load fonts from Google Fonts, so rendering needs network access. Every script writes to `output/`. Review generated files before committing them.

### Cards

```bash
bun index.tsx
```

The script renders each template in [`index.tsx`](./index.tsx) as a PNG and a 2× WebP, unless the template lists its own outputs. A template with timestamps renders one WebP per frame instead. Some templates also write an SVG.

- Pass name prefixes to render only matching templates, such as `bun index.tsx home-usecase`.
- Pass `--debug` to draw layout borders.

### Animations and videos

```bash
bun animation.tsx readme-banner
bun video.tsx dither-og
```

`animation.tsx` writes an animated WebP for a template that exports an `animation` config. `video.tsx` writes an MP4 for a template that exports a `video` config. It pipes frames to [FFmpeg](https://ffmpeg.org), which must be installed.

### PDFs

```bash
bun usecases-pdf.tsx
```

The script renders the ticket, invoice, and certificate templates as single-page PDFs with `takumi-pdf`.
