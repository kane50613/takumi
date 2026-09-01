import type { ReactNode } from "react";
import type { GoogleFontFamily } from "takumi-js/helpers";

export const name = "offset-path";

export const width = 1200;

export const height = 1200;

export const fonts = [];

export const googleFonts: GoogleFontFamily[] = [{ name: "Space Grotesk", weight: 700 }];

export const images = [{ src: "logo", path: "takumi.svg" }];

export const video = { durationMs: 16000, fps: 60, dpr: 2 };

type Stops = ReadonlyArray<readonly [number, number, number]>;

// The two gradients straight off the logo (takumi.svg): the handle's amber→red
// and the head's red→crimson.
const HANDLE_STOPS: Stops = [
  [255, 169, 68],
  [255, 51, 0],
];
const HEAD_STOPS: Stops = [
  [255, 53, 53],
  [215, 29, 54],
];

// Continuous gradient sampled per glyph (background-clip:text would re-run the
// whole gradient inside each glyph box, so sample a solid colour per token).
function gradient(t: number, stops: Stops) {
  const at = (k: number) => stops[Math.max(0, Math.min(stops.length - 1, k))] ?? [0, 0, 0];
  const x = Math.max(0, Math.min(1, t)) * (stops.length - 1);
  const i = Math.floor(x);
  const f = x - i;
  const [ar, ag, ab] = at(i);
  const [br, bg, bb] = at(i + 1);
  const mix = (p: number, q: number) => Math.round(p + (q - p) * f);
  return `rgb(${mix(ar, br)} ${mix(ag, bg)} ${mix(ab, bb)})`;
}

// The two paths that make up the Takumi hammer logo (assets/images/takumi.svg,
// viewBox 0 0 128 128): the handle and the head. The text rides these, so it
// traces the logo's own silhouette.
const LOGO_HANDLE =
  "M114.3 14.1c1.1.9 3.2 2.7 4.2 4.5s.9 3.5.8 4.8-.4 2.3-2 4.3c-1.7 2-4.7 5-12.8 13.7-8.1 8.8-21.4 23.4-35.3 38.4s-28.6 30.5-36.4 38.7-8.8 8.8-10 9.2-2.5.4-4.1 0-3.5-1.2-6.5-3.8-7.1-6.9-9.4-10S.1 108.8.1 107c0-1.7.4-3.4 5.3-8.6s14.3-13.9 30.5-28.9c16.1-14.9 39-36.1 51-47.2s13.2-12 14.7-12.7 3.2-1.1 4.8-.9 2.8 1 3.9 1.9a32 32 0 0 1 2.5 2.1l.4.5z";
const LOGO_HEAD =
  "M79 .5C65.3 3.1 46.9 23.4 56.8 36.3c3.3 4.3 5.1 6.7 9.3 9.7 10.2 7.3 39.1 31 53.1 26.9 12-3.5 9.4-16.9 5.6-25.8-1.3-3-25.7-52.8-45.8-46.6";

// Minimal SVG path sampler — handles the M/L/C/S/A/Z commands these two paths
// use (the one tiny arc is approximated by its endpoint), enough to sample them
// into a polyline. Returns absolute points.
function samplePath(d: string): [number, number][] {
  const toks = d.match(/-?\d*\.?\d+(?:e[+-]?\d+)?|[a-zA-Z]/g) ?? [];
  let i = 0;
  const num = () => Number(toks[i++]);
  const pts: [number, number][] = [];
  let cx = 0,
    cy = 0,
    sx = 0,
    sy = 0,
    px = 0,
    py = 0,
    cmd = "";
  const cubic = (x1: number, y1: number, x2: number, y2: number, x: number, y: number) => {
    const steps = 16;
    for (let k = 1; k <= steps; k++) {
      const t = k / steps;
      const u = 1 - t;
      pts.push([
        u * u * u * cx + 3 * u * u * t * x1 + 3 * u * t * t * x2 + t * t * t * x,
        u * u * u * cy + 3 * u * u * t * y1 + 3 * u * t * t * y2 + t * t * t * y,
      ]);
    }
    cx = x;
    cy = y;
  };
  while (i < toks.length) {
    if (/[a-zA-Z]/.test(toks[i] ?? "")) cmd = toks[i++] ?? "";
    const rel = cmd === cmd.toLowerCase();
    const C = cmd.toUpperCase();
    const ox = rel ? cx : 0;
    const oy = rel ? cy : 0;
    if (C === "M") {
      cx = ox + num();
      cy = oy + num();
      sx = cx;
      sy = cy;
      pts.push([cx, cy]);
      cmd = rel ? "l" : "L";
    } else if (C === "L") {
      cx = ox + num();
      cy = oy + num();
      pts.push([cx, cy]);
    } else if (C === "C") {
      const x1 = ox + num(),
        y1 = oy + num(),
        x2 = ox + num(),
        y2 = oy + num(),
        x = ox + num(),
        y = oy + num();
      px = x2;
      py = y2;
      cubic(x1, y1, x2, y2, x, y);
    } else if (C === "S") {
      const x2 = ox + num(),
        y2 = oy + num(),
        x = ox + num(),
        y = oy + num();
      px = 2 * cx - px;
      py = 2 * cy - py;
      cubic(px, py, x2, y2, x, y);
      px = x2;
      py = y2;
    } else if (C === "A") {
      i += 5;
      cx = ox + num();
      cy = oy + num();
      pts.push([cx, cy]);
    } else if (C === "Z") {
      cx = sx;
      cy = sy;
      pts.push([cx, cy]);
    } else {
      i++;
    }
  }
  return pts;
}

// Sample, then scale + centre so the logo fills the middle of the frame.
// Closed with Z so offset-distance wraps instead of clamping.
function logoPath(d: string, scale: number, tx: number, ty: number) {
  const pts = samplePath(d).map(([x, y]) => [x * scale + tx, y * scale + ty] as [number, number]);
  let len = 0;
  for (let k = 1; k < pts.length; k++) {
    const [x0, y0] = pts[k - 1] ?? [0, 0];
    const [x1, y1] = pts[k] ?? [0, 0];
    len += Math.hypot(x1 - x0, y1 - y0);
  }
  const path = `M ${pts.map(([x, y]) => `${x.toFixed(1)} ${y.toFixed(1)}`).join(" L ")} Z`;

  return { d: path, len };
}

type Token = { char?: string; logo?: boolean };

// "TAKUMI" then the logo, repeated to fill the whole tape at a comfortable pace.
function tokens(len: number, font: number): Token[] {
  const unit: Token[] = [..."TAKUMI"].map((char) => ({ char }));
  unit.push({ char: " " }, { logo: true }, { char: " " });
  const target = Math.max(unit.length, Math.round(len / (font * 0.82)));
  const repeats = Math.max(1, Math.round(target / unit.length));
  return Array.from({ length: repeats }, () => unit).flat();
}

type Tape = {
  d: string;
  toks: Token[];
  font: number;
  lapMs: number;
  dir: 1 | -1;
  stops: Stops;
};

// Both logo strokes scaled up and centred. The text rides them, gradient-tinted,
// so the moving type draws the Takumi mark.
const SCALE = 7;
const TX = 600 - 63 * SCALE;
const TY = 600 - 64 * SCALE;
const HANDLE = logoPath(LOGO_HANDLE, SCALE, TX, TY);
const HEAD = logoPath(LOGO_HEAD, SCALE, TX, TY);

const HANDLE_TAPE: Tape = {
  d: HANDLE.d,
  toks: tokens(HANDLE.len, 40),
  font: 40,
  lapMs: 14000,
  dir: 1,
  stops: HANDLE_STOPS,
};
const HEAD_TAPE: Tape = {
  d: HEAD.d,
  toks: tokens(HEAD.len, 40),
  font: 40,
  lapMs: 10000,
  dir: -1,
  stops: HEAD_STOPS,
};

export const css = [
  `@keyframes ride-fwd { from { offset-distance: 0%; } to { offset-distance: 100%; } }`,
  `@keyframes ride-rev { from { offset-distance: 100%; } to { offset-distance: 0%; } }`,
];

function rider(tape: Tape, phase: number, child: ReactNode) {
  return (
    <div
      style={{
        position: "absolute",
        left: 0,
        top: 0,
        display: "flex",
        offsetPath: `path('${tape.d}')`,
        offsetRotate: "auto",
        offsetAnchor: "50% 50%",
        animation: `${tape.dir === 1 ? "ride-fwd" : "ride-rev"} ${tape.lapMs}ms linear infinite`,
        animationDelay: `${-phase * tape.lapMs}ms`,
      }}
    >
      {child}
    </div>
  );
}

function tapeNodes(tape: Tape, key: string) {
  const nodes = tape.toks.map((tok, i) => {
    // Continuous gradient along the tape using the logo's own colours.
    const color = gradient(i / (tape.toks.length - 1), tape.stops);
    const child = tok.logo ? (
      // Logo recoloured to match the text via an alpha mask.
      <div
        style={{
          display: "flex",
          width: `${tape.font}px`,
          height: `${tape.font}px`,
          backgroundColor: color,
          maskImage: "url(logo)",
          maskSize: "contain",
          maskRepeat: "no-repeat",
          maskPosition: "center",
        }}
      />
    ) : (
      <span style={{ fontSize: `${tape.font}px`, fontWeight: 700, color, lineHeight: 1 }}>
        {tok.char}
      </span>
    );
    return rider(tape, i / tape.toks.length, child);
  });

  return (
    <div key={key} style={{ position: "absolute", left: 0, top: 0, display: "flex" }}>
      {nodes}
    </div>
  );
}

export default function OffsetPath() {
  return (
    <div
      style={{
        position: "relative",
        display: "flex",
        width: "100%",
        height: "100%",
        backgroundColor: "#ffffff",
        backgroundImage: "radial-gradient(120% 120% at 50% 40%, #ffffff 0%, #f1f1f4 100%)",
        fontFamily: '"Space Grotesk", sans-serif',
      }}
    >
      {tapeNodes(HANDLE_TAPE, "handle")}
      {tapeNodes(HEAD_TAPE, "head")}
    </div>
  );
}
