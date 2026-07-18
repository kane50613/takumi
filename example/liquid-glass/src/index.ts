import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { render } from "takumi-js";
import { container, googleFonts, image, text } from "takumi-js/helpers";
import { applyLiquidGlass } from "./liquid-glass.ts";
import { applyLiquidGlassCpu } from "./liquid-glass-cpu.ts";

const width = 1200;
const height = 630;
const dpr = 2;
const glass = { x: 280, y: 320, width: 640, height: 240, radius: 48 };

const assets = join(import.meta.dirname, "../../../assets");
const fonts = await googleFonts(["Instrument Sans", "Noto Serif", "Noto Sans TC"]);
const images = [
  {
    src: "wallpaper",
    data: await readFile(join(assets, "images/benjamin-voros-phIFdC6lA4E-unsplash.jpg")),
  },
];

const scene = image({
  src: "wallpaper",
  style: { width: "100%", height: "100%", objectFit: "cover" },
});

const raw = await render(scene, {
  width: width * dpr,
  height: height * dpr,
  format: "raw",
  devicePixelRatio: dpr,
  fonts,
  images,
});
const useCpu = process.argv.includes("--cpu");
const glassPx = {
  x: glass.x * dpr,
  y: glass.y * dpr,
  width: glass.width * dpr,
  height: glass.height * dpr,
  radius: glass.radius * dpr,
};
const thickness = 20 * dpr;

const start = performance.now();
const processed = useCpu
  ? applyLiquidGlassCpu(new Uint8Array(raw), width * dpr, height * dpr, glassPx, thickness)
  : await applyLiquidGlass(new Uint8Array(raw), width * dpr, height * dpr, glassPx, thickness);

console.log(`${useCpu ? "cpu" : "gpu"} filter: ${(performance.now() - start).toFixed(0)}ms`);

const progress = 0.42;

const widget = container({
  style: {
    position: "absolute",
    left: glass.x,
    top: glass.y,
    width: glass.width,
    height: glass.height,
    display: "flex",
    flexDirection: "column",
    justifyContent: "space-between",
    padding: 36,
  },
  children: [
    container({
      style: { display: "flex", alignItems: "center", gap: 24 },
      children: [
        container({
          style: {
            width: 96,
            height: 96,
            borderRadius: 24,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            backgroundImage: "linear-gradient(160deg, #f2903d 0%, #c33764 100%)",
            boxShadow: "0 4px 16px rgb(0 0 0 / 0.25)",
          },
          children: [
            text({
              text: "匠",
              style: { fontSize: 48, fontWeight: 600, color: "rgb(255 255 255 / 0.95)" },
            }),
          ],
        }),
        container({
          style: { display: "flex", flexDirection: "column", gap: 6 },
          children: [
            text({
              text: "Glasswork",
              style: {
                fontFamily: "Noto Serif",
                fontSize: 42,
                letterSpacing: "0.01em",
                color: "white",
              },
            }),
            text({
              text: "Takumi",
              style: { fontSize: 20, fontWeight: 500, color: "rgb(255 255 255 / 0.72)" },
            }),
          ],
        }),
      ],
    }),
    container({
      style: { display: "flex", flexDirection: "column", gap: 10 },
      children: [
        container({
          style: {
            width: "100%",
            height: 5,
            borderRadius: 3,
            backgroundColor: "rgb(255 255 255 / 0.28)",
            display: "flex",
          },
          children: [
            container({
              style: {
                width: `${progress * 100}%`,
                height: "100%",
                borderRadius: 3,
                backgroundColor: "rgb(255 255 255 / 0.92)",
              },
            }),
          ],
        }),
        container({
          style: { display: "flex", justifyContent: "space-between" },
          children: [
            text({
              text: "1:07",
              style: { fontSize: 15, fontWeight: 500, color: "rgb(255 255 255 / 0.6)" },
            }),
            text({
              text: "-1:33",
              style: { fontSize: 15, fontWeight: 500, color: "rgb(255 255 255 / 0.6)" },
            }),
          ],
        }),
      ],
    }),
  ],
});

const composed = container({
  style: { width: "100%", height: "100%", position: "relative" },
  children: [
    image({
      src: { width: width * dpr, height: height * dpr, data: processed },
      style: { position: "absolute", left: 0, top: 0, width, height },
    }),
    widget,
  ],
});

const webp = await render(composed, {
  width: width * dpr,
  height: height * dpr,
  format: "webp",
  quality: 90,
  devicePixelRatio: dpr,
  fonts,
});
const outDir = join(import.meta.dirname, "../output");

const outFile = useCpu ? "liquid-glass-cpu.webp" : "liquid-glass.webp";

await mkdir(outDir, { recursive: true });
await writeFile(join(outDir, outFile), webp);
console.log(`wrote output/${outFile}`);
