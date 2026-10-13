/** The homepage card, rendered and painted once while the site builds; every section reads from it. */
import { readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { fromJsx } from "takumi-js/helpers/jsx";
import { type Drawable, type Paint, paint } from "takumi-paint";
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
  png: "/images/twitter-images/home-demo-card.png",
  motion: "/images/twitter-images/home-demo-card-motion.webp",
  svg: "/images/twitter-images/home-demo-card.svg",
  frames: (animation.durationMs * animation.fps) / 1000,
  headline: "This card is the code beside it.",
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

/** The card as a node tree, which the live demo edits and renders again. */
export const { node } = await fromJsx(<DemoCard />);

const tree = await paint(<DemoCard />, { width, height });

function hex([red, green, blue]: readonly number[]) {
  return `#${[red, green, blue].map((channel) => Math.round(channel).toString(16).padStart(2, "0")).join("")}`;
}

function paintLabel(value: Paint) {
  return value.type === "color" ? hex(value.color) : value.type;
}

function drawableLabel(drawable: Drawable) {
  switch (drawable.type) {
    case "fill":
      return `fill ${drawable.shape.type} ${paintLabel(drawable.paint)}`;
    case "glyphs":
      return `glyphs “${drawable.run.text.trim()}” ${paintLabel(drawable.paint)}`;
    default:
      return drawable.type;
  }
}

/** The draw steps takumi-paint returns for the card, one line each, such as `steps[0] div fill rect #16130f`. */
export const paintSteps = tree.steps.flatMap((step, index) => {
  if (step.type !== "draw" || step.drawables.length === 0) {
    return [];
  }

  const name = step.node.type === "box" ? (step.node.element?.tagName ?? "box") : step.node.type;

  return step.drawables.map((drawable) => `steps[${index}] ${name} ${drawableLabel(drawable)}`);
});

const nodes = inspectPaint(tree).nodes.filter((entry) => entry.kind === "box" && entry.depth > 0);

/** Every box below the root, with the rectangle layout gave it. */
export const boxes = nodes.map(({ depth, bounds }) => ({ depth, ...bounds }));
