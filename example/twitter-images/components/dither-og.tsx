import type { GoogleFontFamily } from "takumi-js/helpers";

export const name = "dither-og";

export const width = 1200;
export const height = 630;

export const fonts = [];

export const googleFonts: GoogleFontFamily[] = [
  { name: "Silkscreen", weight: 400 },
  { name: "Silkscreen", weight: 700 },
];

const BACKDROP = "#0a0a0b";
const FG = "#ffffff";
const FILL = "#28D26E";
const STROKE = "#96FFB4";

const CELL = 3;

const BAYER = [
  [0, 8, 2, 10],
  [12, 4, 14, 6],
  [3, 11, 1, 9],
  [15, 7, 13, 5],
];

function dataUri(markup: string) {
  return `data:image/svg+xml,${encodeURIComponent(markup)}`;
}

const bayerTile = dataUri(
  `<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4">${BAYER.map((row, y) =>
    row
      .map((threshold, x) => {
        const value = Math.round(((threshold + 0.5) / 16) * 255);
        return `<rect x="${x}" y="${y}" width="1" height="1" fill="rgb(${value},${value},${value})"/>`;
      })
      .join(""),
  ).join("")}</svg>`,
);

function channels(from: string, to: string) {
  const stops = (offset: number) =>
    [from, to].map((hex) => Number.parseInt(hex.slice(1 + offset * 2, 3 + offset * 2), 16) / 255);
  return ["R", "G", "B"]
    .map(
      (channel, offset) =>
        `<feFunc${channel} type="table" tableValues="${stops(offset).join(" ")}"/>`,
    )
    .join("");
}

// 1-bit ordered dither: luma is the dot density, a tiled Bayer matrix is the
// threshold, and the surviving two states get a color each. Every cell is lit
// or unlit, so the output holds no continuous tone.
function ditherFilter(dark: string, bright: string) {
  const size = 4 * CELL;
  return `url("${dataUri(
    `<filter color-interpolation-filters="sRGB" x="0" y="0" width="100%" height="100%">` +
      `<feImage href="${bayerTile}" width="${size}" height="${size}" result="b"/>` +
      `<feTile in="b" result="tile"/>` +
      `<feColorMatrix in="SourceGraphic" type="matrix" result="luma" values="0.2126 0.7152 0.0722 0 0 0.2126 0.7152 0.0722 0 0 0.2126 0.7152 0.0722 0 0 0 0 0 1 0"/>` +
      `<feComposite in="luma" in2="tile" operator="arithmetic" k2="1" k3="1" k4="-0.5" result="noised"/>` +
      `<feComponentTransfer in="noised" result="bits">` +
      `<feFuncR type="discrete" tableValues="0 1"/><feFuncG type="discrete" tableValues="0 1"/><feFuncB type="discrete" tableValues="0 1"/>` +
      `</feComponentTransfer>` +
      `<feComponentTransfer in="bits" result="tinted">${channels(dark, bright)}</feComponentTransfer>` +
      `<feComposite in="tinted" in2="SourceAlpha" operator="in"/>` +
      `</filter>`,
  )}")`;
}

const CHART_HEIGHT = 340;
const STROKE_WIDTH = 3;

// Renders per second as the viewport gets wider: a climb with two pullbacks.
const SERIES = [0.22, 0.3, 0.19, 0.42, 0.36, 0.58, 0.5, 0.71, 0.66, 0.94];

function points(offset = 0) {
  return SERIES.map((value, index) => ({
    x: (index / (SERIES.length - 1)) * width,
    y: (1 - value) * CHART_HEIGHT + offset,
  }));
}

type Point = { x: number; y: number };

function round({ x, y }: Point) {
  return `${x.toFixed(1)} ${y.toFixed(1)}`;
}

// Catmull-Rom through every sample, converted to the cubic segments `path()`
// speaks: the tangent at each point is a sixth of the span its neighbours
// straddle.
function curve(samples: Point[]) {
  return samples
    .slice(1)
    .map((end, index) => {
      // Clamped, so the ends reuse their own point as the missing neighbour.
      const at = (offset: number) =>
        samples[Math.min(Math.max(offset, 0), samples.length - 1)] ?? end;
      const [before, start, after] = [at(index - 1), at(index), at(index + 2)];
      const control = [
        { x: start.x + (end.x - before.x) / 6, y: start.y + (end.y - before.y) / 6 },
        { x: end.x - (after.x - start.x) / 6, y: end.y - (after.y - start.y) / 6 },
      ];
      return `C ${control.map(round).join(", ")}, ${round(end)}`;
    })
    .join(" ");
}

function pathOf(samples: Point[]) {
  const [first] = samples;

  return first ? `M ${round(first)} ${curve(samples)}` : "";
}

// The fill fades downward, so the dither thins out into the backdrop instead of
// ending on a hard edge.
const area = {
  position: "absolute" as const,
  inset: 0,
  backgroundImage: `linear-gradient(to top, ${BACKDROP}, ${FILL} 88%)`,
  clipPath: `path("${pathOf(points())} L ${width} ${CHART_HEIGHT} L 0 ${CHART_HEIGHT} Z")`,
};

// A stroke the clip can express: the curve out, then the same curve shifted
// down and walked back.
const stroke = {
  position: "absolute" as const,
  inset: 0,
  backgroundColor: STROKE,
  clipPath: `path("${pathOf(points())} ${pathOf(points(STROKE_WIDTH).reverse()).replace("M", "L")} Z")`,
};

export default function DitherOg() {
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
      <div style={{ position: "absolute", left: 0, right: 0, bottom: 0, height: "58%" }}>
        <div style={{ ...area, filter: ditherFilter(BACKDROP, FILL) }} />
        <div style={stroke} />
        <div
          style={{
            ...area,
            filter: "blur(3px) brightness(1.35) saturate(1.4)",
            opacity: 0.7,
            mixBlendMode: "plus-lighter",
          }}
        />
      </div>

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
        <span>We have dither style</span>
        <span>in Takumi with SVG filter!</span>
      </h1>
    </div>
  );
}
