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
const output = join("output", `${name}.mp4`);

const ff = Bun.spawn(
  [
    "ffmpeg",
    "-y",
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
  { stdin: "pipe", stdout: "ignore", stderr: "ignore" },
);

for (let f = 0; f < frameCount; f++) {
  const ms = (f * 1000) / fps;
  const { node, stylesheets } = await fromJsx(frameAt(ms));

  const buffer = await renderer.render(node, {
    width: outWidth,
    height: outHeight,
    devicePixelRatio: dpr,
    format: "raw",
    stylesheets: [...stylesheets, ...(module.stylesheets ?? [])],
    images,
    fonts: fonts.length > 0 ? fonts : undefined,
    timeMs: ms,
  });

  ff.stdin.write(buffer);
  await ff.stdin.flush();
}

ff.stdin.end();
await ff.exited;
console.log(`${output} — ${frameCount} frames @ ${fps}fps`);
