import { codeToTokens, type ThemedToken } from "shiki";

export const name = "readme-banner";

// GitHub renders README content at 838 CSS px; the banner is designed 1:1 for
// that width and rendered at dpr 2.
export const width = 840;
export const height = 220;

export const images = [{ src: "takumi.svg", path: "takumi.svg" }];

export const fonts = ["geist/Geist[wght].woff2", "geist/GeistMono[wght].woff2"];

export const animation = { durationMs: 10000, fps: 30, dpr: 2, lossless: true };

const SNIPPETS = [
  `// the code behind this banner
await render(<Banner />, {
  width: ${width},
  height: ${height},
});`,
  `// paged PDF from the same JSX
await render(<Invoice />, {
  size: "a4",
  footer: <PageNumber />,
});`,
  `// or an animated WebP
await renderAnimation({
  fps: 30, format: "webp",
  scenes: [{ node: <Spinner /> }],
});`,
  `// next/og drop-in, Tailwind included
new ImageResponse(
  <h1 tw="text-6xl">Hello</h1>,
  { width: 1200, height: 630 },
);`,
];

const SLIDES: ThemedToken[][][] = await Promise.all(
  SNIPPETS.map(
    async (code) =>
      (await codeToTokens(code, { lang: "tsx", theme: "github-dark-default" })).tokens,
  ),
);

const FEATURES = ["Images", "PDFs", "Animations", "next/og"];
const LINE_HEIGHT = 30;
const SLIDE_HEIGHT = LINE_HEIGHT * 5;
const SLIDE_MS = animation.durationMs / SNIPPETS.length;

export const css = [
  {
    selector: ".banner",
    style: {
      display: "flex",
      alignItems: "center",
      justifyContent: "space-between",
      width: "100%",
      height: "100%",
      padding: "0 3rem 0 3.5rem",
      backgroundColor: "#0d1117",
      fontFamily: "Geist, sans-serif",
    },
  },
  { selector: ".brand", style: { display: "flex", flexDirection: "column" } },
  {
    selector: ".lockup",
    style: { display: "flex", alignItems: "center" },
  },
  {
    selector: ".features",
    style: {
      display: "flex",
      gap: "22px",
      marginTop: "16px",
      marginLeft: "4px",
      fontSize: "16px",
      fontWeight: 500,
      letterSpacing: "-0.005em",
    },
  },
  {
    selector: ".feature",
    style: {
      display: "flex",
      color: "#484f58",
      animation: `active ${animation.durationMs}ms ease-in-out infinite`,
    },
  },
  { selector: ".logo", style: { width: "64px", height: "64px" } },
  {
    selector: ".wordmark",
    style: {
      marginLeft: "14px",
      fontSize: "56px",
      fontWeight: 750,
      letterSpacing: "-0.05em",
      color: "#e6edf3",
    },
  },
  {
    selector: ".window",
    style: {
      display: "flex",
      position: "relative",
      width: "368px",
      height: `${SLIDE_HEIGHT}px`,
      fontFamily: '"Geist Mono", monospace',
      fontSize: "16px",
      lineHeight: `${LINE_HEIGHT}px`,
      whiteSpace: "pre",
    },
  },
  {
    selector: ".slide",
    style: {
      display: "flex",
      position: "absolute",
      inset: 0,
      flexDirection: "column",
      justifyContent: "center",
      opacity: 0,
      animation: `fade ${animation.durationMs}ms ease-in-out infinite`,
    },
  },
  { selector: ".slide span", style: { display: "flex" } },
  {
    selector: ".caret",
    style: {
      width: "9px",
      height: "20px",
      marginTop: "5px",
      backgroundColor: "#e6edf3",
      animation: "blink 1.25s step-end infinite",
    },
  },
  // Each slide holds a quarter of the loop; fades overlap by 3% (0.3s) as a
  // crossfade, wrapping across the loop boundary through negative delays.
  {
    keyframes: "fade",
    steps: [
      { offset: "0%", style: { opacity: 0 } },
      { offset: "3%", style: { opacity: 1 } },
      { offset: "25%", style: { opacity: 1 } },
      { offset: "28%", style: { opacity: 0 } },
      { offset: "to", style: { opacity: 0 } },
    ],
  },
  // Shares the slides' phase: the label whose snippet is showing lights up.
  {
    keyframes: "active",
    steps: [
      { offset: "0%", style: { color: "#484f58" } },
      { offset: "3%", style: { color: "#e6edf3" } },
      { offset: "25%", style: { color: "#e6edf3" } },
      { offset: "28%", style: { color: "#484f58" } },
      { offset: "to", style: { color: "#484f58" } },
    ],
  },
  {
    keyframes: "blink",
    steps: [
      { offset: "0%", style: { opacity: 1 } },
      { offset: "50%", style: { opacity: 0 } },
      { offset: "to", style: { opacity: 1 } },
    ],
  },
];

// Phase-shifted so frame 0 shows the first slide fully settled.
function Slide({ lines, index }: { lines: ThemedToken[][]; index: number }) {
  return (
    <div
      className="slide"
      style={{ animationDelay: `${index * SLIDE_MS - animation.durationMs - 300}ms` }}
    >
      {lines.map((line, row) => (
        <span key={row}>
          {line.map((token, column) => (
            <span key={column} style={{ color: token.color }}>
              {token.content}
            </span>
          ))}
          {row === lines.length - 1 && <span className="caret" />}
        </span>
      ))}
    </div>
  );
}

export default function Banner() {
  return (
    <div className="banner">
      <div className="brand">
        <div className="lockup">
          <img className="logo" src="takumi.svg" />
          <span className="wordmark">Takumi</span>
        </div>
        <div className="features">
          {FEATURES.map((label, index) => (
            <span
              key={label}
              className="feature"
              style={{ animationDelay: `${index * SLIDE_MS - animation.durationMs - 300}ms` }}
            >
              {label}
            </span>
          ))}
        </div>
      </div>
      <div className="window">
        {SLIDES.map((lines, index) => (
          <Slide key={index} lines={lines} index={index} />
        ))}
      </div>
    </div>
  );
}
