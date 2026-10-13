/** The homepage card, painted once while the site builds; every chapter of the scene reads from it. */
import { readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { paint } from "takumi-paint";
import { paintScene } from "~/components/mdx/paint/scene";
import { inspectPaint } from "~/playground/inspect-paint";
import DemoCard, {
  height,
  width,
} from "../../../../example/twitter-images/components/home-demo-card";
import { animation } from "../../../../example/twitter-images/components/home-demo-card-motion";

const SOURCE_PATH = "../example/twitter-images/components/home-demo-card.tsx";

export const CARD = {
  width,
  height,
  image: "/images/twitter-images/home-demo-card@2x.webp",
  motion: "/images/twitter-images/home-demo-card-motion.webp",
  svg: "/images/twitter-images/home-demo-card.svg",
  frames: (animation.durationMs * animation.fps) / 1000,
};

/** The size of a file under `public`, such as `32 KB`. */
export function kilobytes(path: string) {
  return `${Math.round(statSync(join("public", path)).size / 1024)} KB`;
}

const sourceFile = readFileSync(SOURCE_PATH, "utf8");

/** The card component's source, from `export default function` to the end of the file. */
export const source = sourceFile
  .slice(sourceFile.indexOf("export default function"))
  .trimEnd()
  .split("\n");

const tree = await paint(<DemoCard />, { width, height });

export const scene = paintScene(tree);

let sceneStep = -1;

/** The scene's draw steps, each named by its index in `tree.steps` and its node. */
export const layers = tree.steps.flatMap((step, order) => {
  // paintScene drops draws with nothing to draw, so the scene's step indices skip them too.
  if (step.type === "draw" && step.drawables.length === 0) {
    return [];
  }
  sceneStep++;

  const name = step.node.type === "box" ? (step.node.element?.tagName ?? "box") : step.node.type;

  return step.type === "draw" ? [{ step: sceneStep, label: `steps[${order}] ${name}` }] : [];
});

const nodes = inspectPaint(tree).nodes.filter((node) => node.kind === "box" && node.depth > 0);

/** Every box below the root, with the rectangle layout gave it. */
export const boxes = nodes.map(({ name, depth, bounds }) => ({
  label: `${name} ${Math.round(bounds.width)}×${Math.round(bounds.height)}`,
  depth,
  ...bounds,
  leaf: !nodes.some(
    (other) =>
      other.depth > depth &&
      other.bounds.x >= bounds.x &&
      other.bounds.y >= bounds.y &&
      other.bounds.x + other.bounds.width <= bounds.x + bounds.width &&
      other.bounds.y + other.bounds.height <= bounds.y + bounds.height,
  ),
}));
