import { join } from "node:path";
import type { ReactNode } from "react";
import { renderAnimation, type RenderAnimationOptions } from "takumi-js";
import { Renderer } from "takumi-js/node";
import { type AssetModule, loadFonts, loadImages } from "./assets";

type AnimationModule = AssetModule & {
  name: string;
  width: number;
  height: number;
  animation: {
    durationMs: number;
    fps?: number;
    dpr?: number;
    quality?: number;
    lossless?: boolean;
  };
  css?: RenderAnimationOptions["css"];
  timeline?: (fps: number) => Array<{ ms: number; durationMs: number }>;
  frame?: (ms: number) => ReactNode;
  default: () => ReactNode;
};

const name = process.argv[2];

if (!name) {
  console.error("usage: bun animation.tsx <component>");
  process.exit(1);
}

const module: AnimationModule = await import(`./components/${name}`);

if (!module.animation) {
  console.error(`${name} does not export an \`animation\` config`);
  process.exit(1);
}

const { durationMs, fps = 20, dpr = 1, quality = 82, lossless = false } = module.animation;
const step = 1000 / fps;
const scenes =
  module.timeline?.(fps) ??
  Array.from({ length: Math.round((durationMs * fps) / 1000) }, (_, index) => ({
    ms: index * step,
    durationMs: Math.round((index + 1) * step) - Math.round(index * step),
  }));
const frameAt = module.frame ?? (() => <module.default />);
// A component that animates through `css` keyframes renders once; the engine
// samples the timeline and the writer merges the frames that come out equal.
const drivenByCss = !module.frame && !module.timeline;
const renderer = new Renderer();
const [fonts, images] = await Promise.all([loadFonts(module), loadImages(module)]);

for (const { key: _key, data, ...details } of fonts) {
  await renderer.registerFont({
    ...details,
    data: typeof data === "function" ? await data() : data,
  });
}

const start = performance.now();
const buffer = await renderAnimation({
  renderer,
  width: module.width * dpr,
  height: module.height * dpr,
  devicePixelRatio: dpr,
  fps,
  format: "webp",
  quality,
  lossless,
  images,
  css: module.css,
  scenes: drivenByCss
    ? [{ node: <module.default />, durationMs }]
    : scenes.map(({ ms, durationMs }) => ({ node: frameAt(ms), durationMs })),
});

const output = join("output", `${module.name}.webp`);

await Bun.write(output, buffer);
console.log(
  `${output} — ${scenes.length} frames @ ${fps}fps, ${(buffer.length / 1024).toFixed(0)} KB, ${Math.round(performance.now() - start)}ms`,
);
