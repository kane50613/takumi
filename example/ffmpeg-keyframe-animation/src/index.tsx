import { resolve } from "node:path";
import { spawn } from "bun";
import { getHighlighterTokens, keyframes, Scene } from "./Scene";
import { render } from "takumi-js";

const fps = 30;
const durationSeconds = 4;
const totalFrames = fps * durationSeconds;
const devicePixelRatio = 1.2;
const width = 1200 * devicePixelRatio;
const height = 630 * devicePixelRatio;
const outputPath = resolve(import.meta.dir, "../output/animation.mp4");

const tokens = await getHighlighterTokens();

const ffmpeg = spawn(
  [
    "ffmpeg",
    "-y",
    "-f",
    "rawvideo",
    "-pixel_format",
    "rgba",
    "-video_size",
    `${width}x${height}`,
    "-framerate",
    `${fps}`,
    "-i",
    "pipe:0",
    "-vf",
    "format=yuv420p10le",
    "-c:v",
    "libx265",
    "-crf",
    "16",
    "-preset",
    "medium",
    "-tag:v",
    "hvc1",
    outputPath,
  ],
  { stdin: "pipe", stdout: "ignore", stderr: "ignore" },
);

const images = await Promise.all(
  [
    { src: "logo.svg", path: "takumi.svg" },
    { src: "background.jpg", path: "martin-martz-W0NRebXbsjM-unsplash.jpg" },
  ].map(async ({ src, path }) => ({
    src,
    data: await Bun.file(resolve(import.meta.dir, "../../../assets/images", path)).arrayBuffer(),
  })),
);

const thumbnailPath = resolve(import.meta.dir, "../output/thumbnail.webp");
console.log(`Rendering thumbnail to ${thumbnailPath}...`);

// Generate thumbnail at 2.5s where things are stable
const thumbnailFrame = await render(<Scene tokens={tokens} showPlayButton={true} />, {
  width,
  height,
  devicePixelRatio,
  format: "webp",
  keyframes,
  images,
  timeMs: 2500,
});

if (!thumbnailFrame) throw new Error("Thumbnail frame is undefined");
await Bun.write(thumbnailPath, thumbnailFrame);
console.log(`Success! Thumbnail saved to ${thumbnailPath}`);

console.log(`Rendering ${totalFrames} frames to ${outputPath}...`);

const scene = <Scene tokens={tokens} />;

const framePromises = Array.from({ length: totalFrames }, (_, i) => {
  const timeMs = (i / fps) * 1000;
  return render(scene, {
    width,
    height,
    devicePixelRatio,
    format: "raw",
    keyframes,
    images,
    timeMs,
  });
});

for (let i = 0; i < totalFrames; i++) {
  const frame = await framePromises[i];
  if (!frame) throw new Error("Frame is undefined");

  ffmpeg.stdin.write(frame);
  if (i % fps === 0)
    console.log(
      `  Progress: ${Math.round((i / totalFrames) * 100)
        .toString()
        .padStart(3)}%`,
    );
}

ffmpeg.stdin.end();
const exitCode = await ffmpeg.exited;

if (exitCode === 0) {
  console.log(`\nSuccess! Video saved to ${outputPath}`);
} else {
  console.error(`ffmpeg failed with exit code ${exitCode}`);
  process.exit(1);
}
