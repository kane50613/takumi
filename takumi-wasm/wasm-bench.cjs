// Wasm-level render+encode benchmark. Drives the real shipped Renderer so we can
// measure wasm-opt -O4 vs -Oz on the hot paths (layout, text shaping, gradient
// raster, png/webp encode). Build the pkg first:
//   wasm-pack build --no-pack --release --out-dir pkg-node --target nodejs
// then: bun wasm-bench.cjs   (or: node wasm-bench.cjs)
const fs = require("node:fs");
const { container, text } = require("@takumi-rs/helpers");
const { Renderer } = require("./pkg-node/takumi_wasm.js");

const FONT_DIR = `${__dirname}/../assets/fonts`;
const geist = new Uint8Array(fs.readFileSync(`${FONT_DIR}/geist/Geist[wght].woff2`));
const twemoji = new Uint8Array(fs.readFileSync(`${FONT_DIR}/twemoji/TwemojiMozilla-colr.woff2`));

const renderer = new Renderer();
renderer.loadFont({ name: "Geist", data: geist });
renderer.loadFont({ name: "Twemoji Mozilla", data: twemoji });

const PARAGRAPH =
  "Typography is the art and technique of arranging type to make written language legible, readable and appealing when displayed. The arrangement of type involves selecting typefaces, point sizes, line lengths, line-spacing and letter-spacing, and adjusting the space between pairs of letters.";

const node = container({
  style: {
    display: "flex",
    flexDirection: "column",
    width: "100%",
    height: "100%",
    padding: 48,
    rowGap: 16,
    fontFamily: "Geist",
    backgroundImage: "linear-gradient(135deg, #f8fafc 0%, #e2e8f0 45%, #cbd5e1 100%)",
  },
  children: [
    text("Takumi WASM render + encode benchmark", {
      fontSize: 56,
      fontWeight: 800,
      color: "#0f172a",
    }),
    text(PARAGRAPH, { fontSize: 28, color: "#334155" }),
    container({
      style: {
        display: "flex",
        width: 280,
        height: 280,
        borderRadius: 40,
        backgroundImage: "linear-gradient(90deg, #ff3b30, #ffcc00, #34c759, #007aff, #5856d6)",
      },
      children: [],
    }),
  ],
});

const W = 1200;
const H = 630;
const run = (format, quality) => renderer.render(node, { width: W, height: H, format, quality });

const bench = (label, fn, warm = 12, iters = 80) => {
  for (let i = 0; i < warm; i++) fn();
  const samples = [];
  for (let i = 0; i < iters; i++) {
    const start = performance.now();
    fn();
    samples.push(performance.now() - start);
  }
  samples.sort((a, b) => a - b);
  const median = samples[Math.floor(samples.length / 2)];
  console.log(
    `${label.padEnd(18)} median ${median.toFixed(3)} ms   min ${samples[0].toFixed(3)} ms`,
  );
};

const png = run("png", 75);
console.log(`sanity: png output ${png.length} bytes\n`);

bench("render raw", () => run("raw"));
bench("render png q75", () => run("png", 75));
bench("render png q100", () => run("png", 100));
bench("render webp q75", () => run("webp", 75));
