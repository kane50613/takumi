import type { GoogleFontFamily } from "takumi-js/helpers";
import { Bitmap } from "takumi-js/helpers/jsx";

export const name = "dither-og";

export const width = 1200;

export const height = 630;

export const fonts = [];

export const googleFonts: GoogleFontFamily[] = [
  { name: "Silkscreen", weight: 400 },
  { name: "Silkscreen", weight: 700 },
];

export const video = { durationMs: 8000, fps: 30 };

const BACKDROP = "#0a0a0b";
const FG = "#ffffff";
const FILL = { r: 0x28, g: 0xd2, b: 0x6e };

const PX = 6;
const COLS = width / PX;
const ROWS = 61;
const CHART_HEIGHT = ROWS * PX;

// dither-kit's paintColumn constants (tripwire.sh/dither-kit): normalized 4×4
// Bayer thresholds, a faint tint for "off" cells instead of holes, and a
// just-under-solid border capping the value line.
const BAYER = [
  [0, 8, 2, 10],
  [12, 4, 14, 6],
  [3, 11, 1, 9],
  [15, 7, 13, 5],
].map((row) => row.map((value) => (value + 0.5) / 16));
const OFF_TIER = 0.4;
const BORDER_ALPHA = 0.72;

// Snapshots the chart eases between, one per segment; the last leads back to
// the first, so the loop is seamless.
const SNAPSHOTS = [
  [0.22, 0.3, 0.19, 0.42, 0.36, 0.58, 0.5, 0.71, 0.66, 0.94],
  [0.55, 0.38, 0.62, 0.3, 0.52, 0.34, 0.66, 0.48, 0.82, 0.6],
  [0.12, 0.2, 0.34, 0.26, 0.48, 0.62, 0.44, 0.36, 0.52, 0.4],
  [0.7, 0.55, 0.78, 0.62, 0.4, 0.5, 0.28, 0.44, 0.2, 0.32],
];

// Leading fraction of each segment that morphs; the rest holds on the snapshot.
const MOVE = 0.65;

function easeInOutCubic(t: number) {
  return t < 0.5 ? 4 * t ** 3 : 1 - (-2 * t + 2) ** 3 / 2;
}

// Clamped Catmull-Rom through the samples, evaluated in index space.
function curveAt(values: number[], u: number) {
  const at = (k: number) => values[Math.min(Math.max(k, 0), values.length - 1)] ?? 0;
  const i = Math.floor(u);
  const t = u - i;
  const [p0, p1, p2, p3] = [at(i - 1), at(i), at(i + 1), at(i + 2)];

  return (
    0.5 *
    (2 * p1 +
      (p2 - p0) * t +
      (2 * p0 - 5 * p1 + 4 * p2 - p3) * t * t +
      (3 * p1 - p0 - 3 * p2 + p3) * t ** 3)
  );
}

// Per-column top row of the value line at `ms`: lerp the current snapshot pair,
// then sample the smooth curve at each backing column.
function surface(ms: number) {
  const segment = video.durationMs / SNAPSHOTS.length;
  const index = Math.floor(ms / segment) % SNAPSHOTS.length;
  const from = SNAPSHOTS[index] ?? [];
  const to = SNAPSHOTS[(index + 1) % SNAPSHOTS.length] ?? from;
  const t = easeInOutCubic(Math.min(1, (ms % segment) / segment / MOVE));
  const values = from.map((value, k) => value + ((to[k] ?? value) - value) * t);

  return Array.from({ length: COLS }, (_, column) => {
    const u = (column / (COLS - 1)) * (values.length - 1);
    const value = Math.min(1, Math.max(0, curveAt(values, u)));

    return Math.min(ROWS - 2, Math.max(0, Math.round((1 - value) * (ROWS - 4)) + 2));
  });
}

// dither-kit's deterministic star field, wink frequency in whole cycles per
// loop so the video loops clean.
const STARS = Array.from({ length: Math.max(4, Math.round(COLS / 20)) }, (_, i) => {
  const seed = i * 67 + 13;

  return {
    column: Math.round(((seed % 10) / 9) * (COLS - 1)),
    depth: ((seed * 53 + 7) % 100) / 100,
    phase: ((seed * 41) % 360) * (Math.PI / 180),
    cycles: 2 + (seed % 2),
  };
});

// dither-kit's paintColumn: density runs 0 at the value line to 1 at the floor
// — solid at the bottom, dissolving upward into the line — and every pixel is
// the one fill colour with only its alpha varying, so the scatter reads the
// same against any backdrop.
function paint(ms: number) {
  const tops = surface(ms);
  const rgba = new Uint8Array(width * CHART_HEIGHT * 4);

  // Premultiplied directly, so the renderer takes the buffer as-is.
  const cell = (cx: number, cy: number, alpha: number) => {
    if (cx < 0 || cx >= COLS || cy < 0 || cy >= ROWS) return;
    const a = Math.round(Math.min(1, Math.max(0, alpha)) * 255);
    const r = Math.round((FILL.r * a) / 255);
    const g = Math.round((FILL.g * a) / 255);
    const b = Math.round((FILL.b * a) / 255);

    for (let y = cy * PX; y < (cy + 1) * PX; y++) {
      let offset = (y * width + cx * PX) * 4;

      for (let x = 0; x < PX; x++) {
        rgba[offset] = r;
        rgba[offset + 1] = g;
        rgba[offset + 2] = b;
        rgba[offset + 3] = a;
        offset += 4;
      }
    }
  };

  for (let cx = 0; cx < COLS; cx++) {
    const top = tops[cx] ?? 0;
    const depth = ROWS - top;

    for (let cy = top; cy < ROWS; cy++) {
      const density = (cy - top) / depth;
      const lit = density > (BAYER[cy & 3]?.[cx & 3] ?? 0);
      const k = 0.3 + density * 0.7;

      cell(cx, cy, lit ? k : k * OFF_TIER);
    }
    cell(cx, top, BORDER_ALPHA);
    cell(cx, top + 1, BORDER_ALPHA * 0.5);
  }

  for (const star of STARS) {
    const top = tops[star.column] ?? 0;
    const cy = Math.round(top + star.depth * (ROWS - top));
    const wink =
      (Math.sin(2 * Math.PI * (ms / video.durationMs) * star.cycles + star.phase) + 1) / 2;
    const lift = wink * 0.7;

    if (lift < 0.55) continue;
    cell(star.column, cy, lift);
    // At the peak of a wink the star flares into a 4-point glint.
    if (wink > 0.9) {
      const glint = lift * 0.6 * (wink - 0.9) * 10;

      cell(star.column - 1, cy, glint);
      cell(star.column + 1, cy, glint);
      cell(star.column, cy - 1, glint);
      cell(star.column, cy + 1, glint);
    }
  }

  return rgba;
}

export function frame(ms: number) {
  return <DitherOg ms={ms} />;
}

export default function DitherOgStill() {
  return <DitherOg ms={0} />;
}

function DitherOg({ ms }: { ms: number }) {
  const chart = {
    width,
    height: CHART_HEIGHT,
    data: paint(ms),
    premultiplied: true,
  };
  const layer = {
    position: "absolute" as const,
    left: 0,
    bottom: 0,
    width: `${width}px`,
    height: `${CHART_HEIGHT}px`,
    imageRendering: "pixelated" as const,
  };

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        padding: "72px",
        backgroundColor: BACKDROP,
        color: FG,
        isolation: "isolate",
      }}
    >
      <Bitmap {...chart} style={layer} />
      <Bitmap
        {...chart}
        style={{
          ...layer,
          filter: "blur(3px) brightness(1.35) saturate(1.4)",
          opacity: 0.7,
          mixBlendMode: "plus-lighter",
        }}
      />

      <h1
        style={{
          display: "flex",
          flexDirection: "column",
          fontFamily: "Silkscreen",
          fontWeight: 400,
          fontSize: 56,
          lineHeight: 1.4,
          margin: 0,
        }}
      >
        <span>We have dither charts</span>
        <span>in Takumi, dither-kit style!</span>
      </h1>
    </div>
  );
}
