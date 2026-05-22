/**
 * Renders preview images referenced from docs/content/docs/guides/*.mdx
 * into docs/public/guides/<slug>.webp.
 *
 * Run with: cd docs && bun scripts/render-guide-images.tsx
 */
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { Renderer } from "takumi-js/node";
import { fromJsx } from "takumi-js/helpers/jsx";
import type { ReactElement } from "react";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "../..");
const outDir = resolve(scriptDir, "../public/guides");

const fontDir = join(repoRoot, "assets/fonts");
const fonts = await Promise.all(
  [
    "geist/Geist[wght].woff2",
    "geist/GeistMono[wght].woff2",
    "twemoji/TwemojiMozilla-colr.woff2",
  ].map((rel) => readFile(join(fontDir, rel))),
);

const renderer = new Renderer({ fonts });

interface Preview {
  slug: string;
  width: number;
  height: number;
  jsx: ReactElement;
}

const previews: Preview[] = [
  {
    slug: "layout-flex",
    width: 1200,
    height: 300,
    jsx: (
      <div tw="flex items-center justify-between w-full h-full p-12 bg-slate-900 text-white">
        <div tw="flex items-center gap-4">
          <div tw="w-16 h-16 rounded-full bg-gradient-to-br from-indigo-500 to-pink-500" />
          <span tw="text-3xl font-semibold">Takumi</span>
        </div>
        <span tw="text-xl text-slate-400">v1.2</span>
      </div>
    ),
  },
  {
    slug: "layout-grid",
    width: 1200,
    height: 600,
    jsx: (
      <div
        tw="w-full h-full p-6 bg-slate-50 text-slate-900"
        style={{
          display: "grid",
          gridTemplateColumns: "200px 1fr 200px",
          gridTemplateRows: "80px 1fr",
          gridTemplateAreas: `"header header header" "sidebar main aside"`,
          gap: 16,
        }}
      >
        <div
          tw="flex items-center justify-center bg-indigo-500 text-white text-2xl rounded-xl"
          style={{ gridArea: "header" }}
        >
          header
        </div>
        <div
          tw="flex items-center justify-center bg-slate-200 rounded-xl text-xl"
          style={{ gridArea: "sidebar" }}
        >
          sidebar
        </div>
        <div
          tw="flex items-center justify-center bg-white border border-slate-300 rounded-xl text-2xl"
          style={{ gridArea: "main" }}
        >
          main
        </div>
        <div
          tw="flex items-center justify-center bg-slate-200 rounded-xl text-xl"
          style={{ gridArea: "aside" }}
        >
          aside
        </div>
      </div>
    ),
  },
  {
    slug: "colors-formats",
    width: 1200,
    height: 240,
    jsx: (
      <div tw="flex w-full h-full">
        {[
          { c: "#1a6ef5", label: "#hex" },
          { c: "rgb(26 110 245)", label: "rgb" },
          { c: "hsl(218 92% 53%)", label: "hsl" },
          { c: "oklch(0.65 0.18 250)", label: "oklch" },
          { c: "oklab(0.65 -0.05 -0.2)", label: "oklab" },
          { c: "color(display-p3 0.1 0.43 0.96)", label: "display-p3" },
        ].map(({ c, label }) => (
          <div
            key={label}
            tw="flex-1 flex flex-col items-center justify-end p-6 text-white"
            style={{ background: c }}
          >
            <span tw="text-xl font-mono">{label}</span>
          </div>
        ))}
      </div>
    ),
  },
  {
    slug: "colors-linear-gradient",
    width: 1200,
    height: 200,
    jsx: (
      <div
        tw="w-full h-full"
        style={{
          backgroundImage: "linear-gradient(135deg, #6366f1 0%, #ec4899 50%, #f59e0b 100%)",
        }}
      />
    ),
  },
  {
    slug: "colors-radial-gradient",
    width: 1200,
    height: 400,
    jsx: (
      <div
        tw="w-full h-full"
        style={{
          backgroundImage: "radial-gradient(circle at 30% 30%, #fbbf24, #ec4899 60%, #1e1b4b 100%)",
        }}
      />
    ),
  },
  {
    slug: "colors-conic-gradient",
    width: 800,
    height: 400,
    jsx: (
      <div tw="flex items-center justify-center w-full h-full bg-slate-950">
        <div
          tw="w-80 h-80"
          style={{
            borderRadius: "50%",
            backgroundImage:
              "conic-gradient(from 0deg, #ef4444, #f59e0b, #10b981, #3b82f6, #ef4444)",
          }}
        />
      </div>
    ),
  },
  {
    slug: "colors-color-mix",
    width: 1200,
    height: 200,
    jsx: (
      <div tw="flex w-full h-full">
        {[0, 25, 50, 75, 100].map((pct) => (
          <div
            key={pct}
            tw="flex-1 flex items-center justify-center text-white text-2xl font-mono"
            style={{ background: `color-mix(in oklch, #1a6ef5 ${100 - pct}%, #ec4899)` }}
          >
            {pct}%
          </div>
        ))}
      </div>
    ),
  },
  {
    slug: "effects-filters",
    width: 1200,
    height: 240,
    jsx: (
      <div tw="flex w-full h-full bg-slate-900">
        {[
          { f: "brightness(1)", label: "original" },
          { f: "brightness(1.3)", label: "brightness" },
          { f: "contrast(1.5)", label: "contrast" },
          { f: "grayscale(1)", label: "grayscale" },
          { f: "hue-rotate(120deg)", label: "hue-rotate" },
          { f: "saturate(2)", label: "saturate" },
          { f: "sepia(1)", label: "sepia" },
          { f: "blur(4px)", label: "blur" },
        ].map(({ f, label }) => (
          <div key={label} tw="flex-1 flex flex-col items-center justify-center gap-3">
            <div
              tw="w-20 h-20 rounded-xl"
              style={{ background: "linear-gradient(135deg, #6366f1, #ec4899)", filter: f }}
            />
            <span tw="text-white text-sm font-mono opacity-70">{label}</span>
          </div>
        ))}
      </div>
    ),
  },
  {
    slug: "effects-backdrop",
    width: 1200,
    height: 400,
    jsx: (
      <div
        tw="flex items-center justify-center w-full h-full"
        style={{
          backgroundImage:
            "linear-gradient(135deg, #f59e0b 0%, #ec4899 40%, #6366f1 80%, #0ea5e9 100%)",
        }}
      >
        <div
          tw="px-12 py-8 text-white text-4xl font-semibold"
          style={{
            backdropFilter: "blur(20px) saturate(180%)",
            background: "rgba(255,255,255,0.1)",
            borderRadius: 24,
            border: "1px solid rgba(255,255,255,0.2)",
          }}
        >
          Frosted glass
        </div>
      </div>
    ),
  },
  {
    slug: "effects-blend-modes",
    width: 1200,
    height: 600,
    jsx: (
      <div tw="flex flex-wrap w-full h-full bg-slate-50 p-4 gap-4">
        {[
          "multiply",
          "screen",
          "overlay",
          "darken",
          "lighten",
          "color-dodge",
          "color-burn",
          "hard-light",
          "soft-light",
          "difference",
          "exclusion",
          "hue",
          "saturation",
          "color",
          "luminosity",
          "normal",
        ].map((mode) => (
          <div
            key={mode}
            tw="relative flex items-end justify-center w-[270px] h-[130px] rounded-xl overflow-hidden"
            style={{ background: "linear-gradient(135deg, #f59e0b, #ec4899)" }}
          >
            <div
              tw="absolute top-3 left-3 w-16 h-16 rounded-full"
              style={{ background: "#6366f1", mixBlendMode: mode as never }}
            />
            <span tw="text-white text-sm font-mono p-2 bg-black/40 rounded mb-2">{mode}</span>
          </div>
        ))}
      </div>
    ),
  },
];

await mkdir(outDir, { recursive: true });

for (const p of previews) {
  const { node, stylesheets } = await fromJsx(p.jsx);
  const buf = await renderer.render(node, {
    width: p.width,
    height: p.height,
    format: "webp",
    stylesheets,
  });
  const dest = join(outDir, `${p.slug}.webp`);
  await writeFile(dest, buf);
  console.log(`wrote ${dest} (${buf.length} bytes)`);
}
