# Takumi documentation website

This workspace contains the guides, homepage, playground, and template registry for [takumi.kane.tw](https://takumi.kane.tw). It uses Fumapress, Fumadocs MDX, and Waku.

## Run locally

Install workspace dependencies from the repository root:

```bash
bun install
```

The playground uses built workspace packages. Follow the [contribution guide](../CONTRIBUTING.md#docs-playground-builds) if those packages are missing or their source has changed.

Start the docs server:

```bash
cd docs
bun dev
```

Open `https://takumi.localhost`. The dev script uses portless. Check `portless list` and reuse an existing server when one is already running.

## Find the source

| Content                                   | Location                    |
| ----------------------------------------- | --------------------------- |
| Guides and API reference                  | `content/docs/*.mdx`        |
| PDF guides                                | `content/docs/pdf/`         |
| Navigation order                          | `content/docs/**/meta.json` |
| Homepage, playground, and showcase routes | `src/pages/`                |
| Homepage sections                         | `app/components/home/`      |
| Showcase projects and templates           | `app/data/showcase.ts`      |
| Installable image templates               | `app/registry/image/`       |
| Playground templates                      | `app/playground/templates/` |
| Site configuration and MDX components     | `press.config.tsx`          |

Give each guide a distinct title and description in its frontmatter. These appear in navigation, page metadata, and generated Open Graph images. Keep existing URLs and heading anchors when editing prose so incoming links continue to work.

## Validate edits

Run these commands from `docs`:

```bash
bun run typecheck
bun run test --silent
bun run build
```

Run `bun lint` from the repository root. The production build checks documentation links and renders page Open Graph images. It needs network access to download the configured Google Fonts.

Follow the [writing guidelines](../CONTRIBUTING.md#writing-and-editing) when adding or revising content.
