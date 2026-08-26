import { join } from "node:path";
import type { ReactNode } from "react";
import { fromJsx } from "takumi-js/helpers/jsx";
import { Renderer } from "takumi-js/node";
import { type AssetModule, loadFonts, loadImages } from "./assets";

type VideoModule = AssetModule & {
  name: string;
  width: number;
  height: number;
  video: { durationMs: number; fps?: number; dpr?: number };
  stylesheets?: string[];
  frame?: (ms: number) => ReactNode;
  fontFamilies?: (ms: number) => string[];
  default: () => ReactNode;
};

const name = process.argv[2];

if (!name) {
  console.error("usage: bun video.tsx <component>");
  process.exit(1);
}

const module: VideoModule = await import(`./components/${name}`);

if (!module.video) {
  console.error(`${name} does not export a \`video\` config`);
  process.exit(1);
}

const { durationMs, fps = 30, dpr = 1 } = module.video;
const frameCount = Math.round((fps * durationMs) / 1000);
const outWidth = module.width * dpr;
const outHeight = module.height * dpr;
const [fonts, images] = await Promise.all([loadFonts(module), loadImages(module)]);
const renderer = new Renderer();

// CSS animations drive a plain component through timeMs; a `frame` export
// rebuilds the tree per frame instead.
const frameAt = module.frame ?? (() => <module.default />);
const output = join("output", `${module.name}.mp4`);

const ff = Bun.spawn(
  [
    "ffmpeg",
    "-y",
    "-loglevel",
    "error",
    "-f",
    "rawvideo",
    "-pixel_format",
    "rgba",
    "-video_size",
    `${outWidth}x${outHeight}`,
    "-framerate",
    String(fps),
    "-i",
    "-",
    "-r",
    String(fps),
    "-crf",
    "18",
    "-pix_fmt",
    "yuv420p",
    "-c:v",
    "libx264",
    "-movflags",
    "+faststart",
    output,
  ],
  { stdin: "pipe", stdout: "ignore", stderr: "inherit" },
);

// Frames render in parallel batches; writes stay in order for ffmpeg.
const BATCH = 8;

for (let start = 0; start < frameCount; start += BATCH) {
  const buffers = await Promise.all(
    Array.from({ length: Math.min(BATCH, frameCount - start) }, async (_, offset) => {
      const ms = ((start + offset) * 1000) / fps;
      const { node, stylesheets } = await fromJsx(frameAt(ms));

      return renderer.render(node, {
        width: outWidth,
        height: outHeight,
        devicePixelRatio: dpr,
        format: "raw",
        css: [...stylesheets, ...(module.stylesheets ?? [])],
        images,
        fonts: fonts.length > 0 ? fonts : undefined,
        fontFamilies: module.fontFamilies?.(ms),
        timeMs: ms,
      });
    }),
  );

  for (const buffer of buffers) {
    ff.stdin.write(buffer);
    await ff.stdin.flush();
  }
}

ff.stdin.end();
const code = await ff.exited;

if (code !== 0) {
  console.error(`ffmpeg exited with code ${code}`);
  process.exit(1);
}
console.log(`${output} — ${frameCount} frames @ ${fps}fps`);
