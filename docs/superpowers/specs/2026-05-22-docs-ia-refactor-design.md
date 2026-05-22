# Docs IA Refactor

The current docs are 18 pages, ~3600 lines. They reflect the shape of the project as it existed a few releases ago, not what Takumi actually does today. A lot of the engine — Grid, color spaces, blend modes, clip-path syntax, `measure()`, `fromHtml()`, the helpers package, six emoji providers, the video pipeline, half the examples — is implemented but unwritten. The 650-line `reference.mdx` is a wall of tables nobody scrolls through. There is no concept layer, no recipes, no 5-minute path in.

This spec describes a full reorganization. Keep what's good, rewrite what's stale, fill in what's missing, and let `fumadocs` do work it's already paid for (AutoTypeTable, Twoslash, Tabs, Files).

Out of scope: a `takumi` Rust crate reference site (defer to docs.rs), and `takumi-server` docs (experimental, hide).

## Audience

JS/TS users come first. Every navigation decision favors the shortest path from "I have a Next.js app" to "I shipped an OG image." Rust users get a single link to docs.rs.

## Voice

Match the README. Punchy, opinionated, contrastive. No "Let's explore," no "In this section we will," no marketing bullet lists pretending to be docs. Lead with what something does and why a real person would care.

## IA

```
[For LLMs]                                          (existing)
  Leaf   → /llms.txt
  Brain  → /llms-full.txt

[Get Started]
  Introduction               index.mdx              rewrite
  Installation               install.mdx            new
  Quickstart                 quickstart.mdx         new
  Migration to v1            upgrade/v1.mdx         keep URL, add "what's new"

[Frameworks]
  Overview                   frameworks/index.mdx   new, decision matrix
  Next.js                    frameworks/nextjs.mdx
  Nuxt                       frameworks/nuxt.mdx
  SvelteKit                  frameworks/sveltekit.mdx
  Svelte (raw)               frameworks/svelte.mdx           new
  TanStack Start             frameworks/tanstack-start.mdx
  Waku                       frameworks/waku.mdx              new
  Cloudflare Workers         frameworks/cloudflare.mdx        new
  Bun / Vanilla              frameworks/vanilla.mdx           new

[Guides]
  Layout                     guides/layout.mdx      rewrite — Flex + Grid + Block + Float
  Typography & Fonts         guides/typography.mdx  keep + variable fonts, fallback chain
  Styling                    guides/styling.mdx     new — Tailwind vs inline vs stylesheets
  Colors & Gradients         guides/colors.mdx      new — oklch, color-mix, conic/radial/linear
  Effects                    guides/effects.mdx     new — filter, blend, shadow, transform, clip-path
  Images & SVG               guides/images.mdx      merge load-images + SVG
  Emoji                      guides/emoji.mdx       new — six providers compared
  Animations                 guides/animations.mdx  rewrite keyframes basics
  Video Frames               guides/video.mdx       new — ffmpeg / ffplay pipeline
  Measure                    guides/measure.mdx     keep, add "why you'd use this"
  Performance                guides/performance.mdx keep, de-dupe with other pages

[Recipes]
  Overview                   recipes/index.mdx
  OG Image                   recipes/og-image.mdx
  Twitter / X Card           recipes/x-card.mdx
  Product Card               recipes/product-card.mdx
  Blog Post Card             recipes/blog-card.mdx
  GitHub Social Preview      recipes/github-preview.mdx
  Package OG                 recipes/package-og.mdx
  Animated Spinner           recipes/spinner.mdx
  Video via ffmpeg           recipes/video.mdx
  Multi-template Gallery     recipes/template-gallery.mdx

[Reference]                                          AutoTypeTable everywhere
  Overview                   reference/index.mdx    entrypoint matrix
  takumi-js                  reference/core.mdx
  takumi-js/response         reference/response.mdx
  takumi-js/node             reference/node.mdx
  takumi-js/wasm             reference/wasm.mdx
  takumi-js/helpers          reference/helpers.mdx
  takumi-js/helpers/jsx      reference/helpers-jsx.mdx
  takumi-js/helpers/emoji    reference/helpers-emoji.mdx
  Node Types                 reference/nodes.mdx
  Style Properties           reference/style.mdx
  Output Formats             reference/output-formats.mdx
  Errors                     reference/errors.mdx

[Explanation]
  Architecture               architecture.mdx       keep, expand fromJsx rules

[Help]
  Troubleshooting            troubleshooting.mdx    expand to 15+ entries
  Rust API                   → docs.rs              external link in nav
```

18 pages → ~50. Most growth is in Reference splits and Recipes.

## Page-level plan

### Get Started

**Introduction (rewrite):** One paragraph defining what Takumi is and is not. One paragraph on when to reach for it vs. headless Chrome vs. Satori. A single Mermaid pipeline diagram (the one already on the page is fine). A `<Cards>` block with three concrete next-steps: Quickstart, Frameworks, Playground. No feature bullet list — that lives on the homepage, not in docs.

**Installation (new):** Tabs for Node, Bun, Cloudflare Workers, Browser. Each tab shows the install command and the single import line a user needs. That's it.

**Quickstart (new):** Three runnable examples in sequence. Static OG image. Animated WebP. Cloudflare Workers route handler. Each one ~10 lines, copy-paste runs. Twoslash on every block so hovering an export shows its type.

**Migration to v1:** Keep the URL. Existing breaking-changes content stays. Add a short "What's new" section at the top so people who land here from a search don't bounce.

### Frameworks

**Overview (new):** A decision matrix — Framework column, Runtime column (Node / Edge / Workers), Recommended Entrypoint column, Example link. Plus four `<Cards>` for the most common picks.

**Existing four (nextjs, nuxt, sveltekit, tanstack-start):** Move under `/frameworks/`. Each page gets a `<Files>` block showing the relevant project structure, a Steps block for setup, and a final "Gotchas" section. Strip the install boilerplate — it lives in the global Installation page now.

**New four (svelte, waku, cloudflare, vanilla):** Each based on an existing `example/` directory. Same template as above. Cloudflare gets a section on `putPersistentImage` for asset preloading because that's the runtime where it matters most.

### Guides

**Layout (rewrite):** Three sections — Flex, Grid, Block. Each section shows a minimal example with rendered output, then a callout listing the supported subset. Currently undocumented Grid features (template-areas, repeat, named lines) get explicit examples. Finishes with a "Common pitfalls" section.

**Typography & Fonts:** Keep most existing content. Add variable fonts (`font-variation-settings`), explicit fallback-chain behavior, COLR vs bitmap glyph notes.

**Styling (new):** Tailwind vs inline `style` vs external stylesheet — when each is right. Migrated from current `tailwind-css.mdx`, expanded with the stylesheet/Vite/UnoCSS patterns from `example/css-library-integration`.

**Colors & Gradients (new):** Linear, radial, conic gradients. Color spaces (sRGB, oklch, oklab, display-p3). `color-mix()`. Each with a rendered swatch image. Almost none of this is in current docs.

**Effects (new):** Filter functions, backdrop-filter, blend-mode (the 16 modes table), box-shadow, transform (including 3D perspective), clip-path (inset / circle / ellipse / polygon / path). Rendered example per category.

**Images & SVG (merge):** Loading remote images, inline data URLs, `object-fit`, `image-rendering`, SVG via `resvg`. Persistent image store. From `load-images.mdx` + new SVG content.

**Emoji (new):** Six providers (twemoji, blobmoji, noto, openmoji, fluent, fluentFlat) with a comparison image. When to use COLR fonts vs `extractEmojis` helper.

**Animations (rewrite):** CSS `@keyframes` basics, timing functions, `animation` shorthand, Tailwind animation utilities. Pull video pipeline out into its own page.

**Video Frames (new):** The `ffmpeg-keyframe-animation` and `ffplay` examples — multi-scene `renderAnimation`, time-stepped raw RGBA frame loop, piping to ffmpeg. Currently buried at the bottom of the keyframe page.

**Measure:** Keep examples. Add a "Why you'd use this" section — text-fitting, responsive cards, layout-aware truncation.

**Performance:** Keep the renderer-reuse and font-preload sections. Remove duplication with Images/Typography pages — link out instead.

### Recipes

All nine recipes pull directly from `example/twitter-images/components/*` and a few new ones. Each recipe page is: rendered preview image, copy-paste source, one paragraph explaining the technique. No long prose. The point of Recipes is "I want that, give me the code."

### Reference

`reference/index.mdx` is an entrypoint matrix — every subpath of `takumi-js` and what it exports. Each subpath gets its own page using `AutoTypeTable` against the TypeScript source. `style.mdx` is the CSS-property surface — auto-derived where possible, hand-annotated where the engine has a quirk.

### Architecture

Keep the existing page. Expand the `fromJsx` ruleset (currently a link to source). Add a short "why no headless browser" section that has been hiding in the README.

### Troubleshooting

Expand from 3 entries to ~15, organized by runtime (Node / WASM / Workers) and topic (fonts / images / CSS / animation). Each entry is a Symptom → Cause → Fix triplet using `<Accordion>`.

## Redirects

301s configured in `vercel.json`. Hash-anchor links into the old `reference.mdx` redirect to `/docs/reference` (the new overview) without preserving the hash — users re-pick from a clean entry page. Simpler than client-side hash routing and the old anchors are unstable anyway after the type-table split.

| Old                                  | New                                         |
| ------------------------------------ | ------------------------------------------- |
| `/docs/integration`                  | `/docs/frameworks`                          |
| `/docs/integration/nextjs`           | `/docs/frameworks/nextjs`                   |
| `/docs/integration/nuxt`             | `/docs/frameworks/nuxt`                     |
| `/docs/integration/sveltekit`        | `/docs/frameworks/sveltekit`                |
| `/docs/integration/tanstack-start`   | `/docs/frameworks/tanstack-start`           |
| `/docs/layout-engine`                | `/docs/guides/layout`                       |
| `/docs/typography-and-fonts`         | `/docs/guides/typography`                   |
| `/docs/tailwind-css`                 | `/docs/guides/styling`                      |
| `/docs/load-images`                  | `/docs/guides/images`                       |
| `/docs/keyframe-animation`           | `/docs/guides/animations`                   |
| `/docs/measure-api`                  | `/docs/guides/measure`                      |
| `/docs/performance-and-optimization` | `/docs/guides/performance`                  |
| `/docs/templates`                    | `/docs/recipes`                             |
| `/docs/reference`                    | `/docs/reference` (overview, content split) |
| `/docs/reference#*`                  | `/docs/reference` (drop hash)               |
| `/docs/architecture`                 | `/docs/architecture` (unchanged)            |
| `/docs/troubleshooting`              | `/docs/troubleshooting` (unchanged)         |
| `/docs/upgrade/v1`                   | `/docs/upgrade/v1` (unchanged)              |

Pages where the rename is the biggest leap (Styling, Animations, Images) get a `<Banner>` for the first few weeks: "Moved from `/docs/...`."

## Fumadocs leverage

Currently used: `Cards`, `Steps`, `Tabs`, `TypeTable`, `Callout`, `Accordion`, `Mermaid`, Twoslash, Orama search.

Adopted in this refactor:

- **AutoTypeTable** for every Reference page. Source of truth is the TS types in `takumi-js/src/**` and `@takumi-rs/core/index.d.ts`. Need a build step to make these readable by fumadocs — likely a small extraction script that emits .d.ts files into a known directory.
- **Files / Folder** in every Framework page for project layout.
- **Twoslash** turned on globally for `tsx`/`ts` blocks in Quickstart, Guides, Recipes. Hovering an import shows its real type from the package.
- **Banner** on renamed pages for the deprecation window.
- **lastModified** display in the page footer — plugin is already enabled, just not surfaced.
- **CodeBlock tabs (group)** for multi-runtime examples — same code, switch between Node / WASM / Cloudflare imports.

A couple of small custom MDX components, scoped to this project:

- `<NodeTree>` — pretty-print a Takumi node tree alongside the rendered output, for use in Recipes.
- `<StyleProperty>` — single-row CSS property card with "supported / partial / no" status and an example. Used in `reference/style.mdx`.

## Open questions

- The Recipes pages need rendered preview images. Generate them via Takumi at build time, or commit static PNGs? Generating at build time keeps them honest but adds a build dependency.
- `style.mdx` is the most ambitious page. If AutoTypeTable can't reach into the Rust style enums cleanly, fall back to hand-written. Decide during implementation, not now.
- Whether `helpers-jsx.mdx` and `helpers-emoji.mdx` are separate pages or sub-sections of `helpers.mdx`. Default to separate pages because their use cases are independent; revisit if they end up too thin.
