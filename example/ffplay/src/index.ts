import { type Node, Renderer } from "takumi-js/node";
import { container, text } from "takumi-js/helpers";
import { spawn } from "bun";

const fps = 60;
const width = 960;
const height = 540;

const renderer = new Renderer();

// Start ffplay with proper flags for raw RGBA input and low-latency optimization
const ffplay = spawn(
  [
    "ffplay",
    "-f",
    "rawvideo",
    "-pixel_format",
    "rgba",
    "-video_size",
    `${width}x${height}`,
    "-framerate",
    `${fps}`,
    // Optimization parameters for smooth playback
    "-fflags",
    "nobuffer", // Reduce buffering delay
    "-flags",
    "low_delay", // Low delay mode
    "-framedrop", // Allow frame dropping to maintain sync
    "-sync",
    "video", // Sync to video stream
    "-vf",
    "setpts=N/FRAME_RATE/TB", // Reset presentation timestamps
    "-probesize",
    "32", // Reduce probe size
    "-analyzeduration",
    "0", // Don't analyze stream
    "-i",
    "pipe:0",
  ],
  {
    stdin: "pipe",
  },
);

console.log("Starting ffplay timer...");
console.log(`Resolution: ${width}x${height} @ ${fps}fps`);

// Auto-quit bun process when ffplay exits
ffplay.exited.then(() => {
  console.log("ffplay exited, cleaning up...");
  cleanup();
});

const renderOptions = { width, height, format: "raw" } as const;
const frameInterval = 1000 / fps;

// Backpressure-driven loop: render the next frame while the current one is
// being written, and let stdin.flush() throttle production to ffplay's pace.
async function run() {
  const start = Date.now();

  let frameIndex = 0;
  let pending = renderer.render(createFrame(), renderOptions);

  while (!ffplay.killed) {
    const frame = await pending;

    pending = renderer.render(createFrame(), renderOptions);

    ffplay.stdin.write(frame);
    await ffplay.stdin.flush();

    frameIndex++;
    const nextDeadline = start + frameIndex * frameInterval;

    await Bun.sleep(Math.max(0, nextDeadline - Date.now()));
  }
}

// Cleanup on exit
function cleanup() {
  ffplay.stdin.end();
  ffplay.kill();
  process.exit(0);
}

process.on("SIGINT", cleanup);
process.on("SIGTERM", cleanup);

// DVD logo bounce: position derived from wall-clock time so frame jitter
// never distorts the trajectory
const speedX = 180; // px/s
const speedY = 120;

function bounce(distance: number, range: number): number {
  const phase = distance % (2 * range);
  return phase < range ? phase : 2 * range - phase;
}

function getTextMeasurement(time: number) {
  return renderer.measure(
    text({
      tw: "text-white text-7xl font-semibold font-mono",
      text: formatTime(time),
    }),
  );
}

const { width: textWidth, height: textHeight } = await getTextMeasurement(Date.now());
const animationStart = Date.now();

run().catch((error) => {
  console.error("Error rendering frame:", error);
  cleanup();
});

function createFrame(time = Date.now()): Node {
  const elapsed = (time - animationStart) / 1000;
  const posX = bounce(width / 2 + elapsed * speedX, width - textWidth);
  const posY = bounce(height / 2 + elapsed * speedY, height - textHeight);

  // Calculate hue rotation based on time for visible smooth color animation
  const hue = ((time / 1000) * 36) % 360; // Rotate through full color spectrum every 10 seconds
  const angle = ((time / 1000) * 10) % 360; // Rotate gradient angle every 36 seconds

  // Vibrant chroma gradient using HSL colors with good saturation
  const color1 = `hsl(${hue}, 80%, 45%)`; // Saturated color
  const color2 = `hsl(${(hue + 120) % 360}, 80%, 55%)`; // Complementary brighter color
  const color3 = `hsl(${(hue + 240) % 360}, 80%, 35%)`; // Third color

  return container({
    tw: "w-full h-full relative bg-gray-950",
    style: {
      backgroundImage: `linear-gradient(${angle}deg, ${color1} 0%, ${color2} 50%, ${color3} 100%)`,
    },
    children: [
      text({
        tw: "text-white text-7xl font-semibold font-mono absolute",
        style: {
          left: posX,
          top: posY,
          textShadow: "0 0 10px rgb(0 0 0 / 0.5)",
        },
        text: formatTime(time),
      }),
    ],
  });
}

// Format time with milliseconds
function formatTime(timestamp: number): string {
  const date = new Date(timestamp);
  const hours = String(date.getHours()).padStart(2, "0");
  const minutes = String(date.getMinutes()).padStart(2, "0");
  const seconds = String(date.getSeconds()).padStart(2, "0");
  const milliseconds = String(date.getMilliseconds()).padStart(3, "0");
  return `${hours}:${minutes}:${seconds}.${milliseconds}`;
}
