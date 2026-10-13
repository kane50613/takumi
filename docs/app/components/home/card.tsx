/** The homepage card, laid out once while the site builds; the forge stage and paint workbench read from it. */
import { paint } from "takumi-paint";
import { type PaintScene, paintScene } from "~/components/mdx/paint/scene";
import DemoCard, {
  height,
  width,
} from "../../../../example/twitter-images/components/home-demo-card";

export const CARD = {
  width,
  height,
  image: "/images/twitter-images/home-demo-card@2x.webp",
  png: "/images/twitter-images/home-demo-card.png",
  svg: "/images/twitter-images/home-demo-card.svg",
  motion: "/images/twitter-images/home-demo-card-motion.webp",
  headline: "One component, every output.",
};

const tree = await paint(<DemoCard />, { width, height });

/** Complete serializable paint scene with layout bounds, shaped glyph outlines, and draw steps. */
export const scene: PaintScene = paintScene(tree);

export type ForgeBox = {
  index: number;
  depth: number;
  name: string;
  x: number;
  y: number;
  width: number;
  height: number;
  dx: number;
  dy: number;
  rotate: number;
};

export type ForgeGlyphRun = {
  id: string;
  text: string;
  x: number;
  y: number;
  width: number;
  ascent: number;
  descent: number;
  fontSize: number;
  outline: string;
  dx: number;
  dy: number;
  scale: number;
  accent: boolean;
};

const centerX = width / 2;
const centerY = height / 2;

/** Every box below the root, with the rectangle layout gave it and its outward forge trajectory. */
export const boxes: ForgeBox[] = scene.nodes
  .map((node, index) => ({ node, index }))
  .filter(({ node }) => node.kind === "box" && node.depth > 0)
  .map(({ node, index }, order) => {
    const { x, y, width: w, height: h } = node.bounds;
    const boxCenterX = x + w / 2;
    const boxCenterY = y + h / 2;
    const normX = (boxCenterX - centerX) / centerX;
    const normY = (boxCenterY - centerY) / centerY;
    const depthFactor = 0.65 + node.depth * 0.35;
    const staggerX = ((order % 5) - 2) * 55;
    const staggerY = ((order % 4) - 1.5) * 48;

    return {
      index,
      depth: node.depth,
      name: node.name,
      x,
      y,
      width: w,
      height: h,
      dx: Math.round(normX * 360 * depthFactor + staggerX),
      dy: Math.round(normY * 260 * depthFactor - 55 + staggerY),
      rotate: Math.round((normX * 11 + ((order % 3) - 1) * 4) * 10) / 10,
    };
  });

/** Hand-tuned outward offsets per run index so no two glyph runs overlap when exploded. */
const RUN_OFFSETS: readonly [number, number, number][] = [
  [-210, -165, 1.18], // takumi.kane.tw
  [-15, -105, 1.12], // PNG · WebP · SVG · PDF · Paint
  [-230, -55, 1.12], // One component,
  [-210, 45, 1.12], // every output.
  [-240, 155, 1.14], // JSX & CSS rendered without a browser
  [30, 165, 1.22], // 1200 × 630
  [125, -135, 1.18], // Crafted in
  [145, -35, 1.18], // Rust.
  [165, 85, 1.18], // Layout · Text · Paint
  [165, 175, 1.16], // Node · Bun · Wasm · Rust
];

/** Every shaped text run in card coordinates, with its SVG outline path and metrics. */
export const glyphRuns: ForgeGlyphRun[] = scene.nodes
  .flatMap((node, nodeIndex) => {
    if (node.kind !== "text") {
      return [];
    }

    return node.runs.map((run, runIndex) => ({
      node,
      nodeIndex,
      run,
      runIndex,
    }));
  })
  .map(({ node, nodeIndex, run, runIndex }, flatIndex) => {
    const x = node.bounds.x + run.x;
    const y = node.bounds.y + run.y;
    const isHeadline = run.fontSize >= 60;
    const isSubhead = run.fontSize >= 30 && run.fontSize < 60;
    const preset = RUN_OFFSETS[flatIndex] ?? [0, 0, 1.15];

    return {
      id: `${nodeIndex}-${runIndex}`,
      text: run.text.trim(),
      x: Math.round(x * 10) / 10,
      y: Math.round(y * 10) / 10,
      width: Math.round(run.width * 10) / 10,
      ascent: Math.round(run.ascent * 10) / 10,
      descent: Math.round(run.descent * 10) / 10,
      fontSize: Math.round(run.fontSize),
      outline: run.outline,
      dx: preset[0],
      dy: preset[1],
      scale: preset[2],
      accent: isHeadline || isSubhead || run.text.includes("1200"),
    };
  });
