/** The homepage card, painted once while the site builds; every stage of the story reads from it. */
import { type Drawable, paint } from "takumi-paint";
import { paintScene } from "~/components/mdx/paint/scene";
import { describeDrawable, describePaint, inspectPaint, round } from "~/playground/inspect-paint";
import DemoCard, {
  height,
  width,
} from "../../../../example/twitter-images/components/home-demo-card";

export const CARD = {
  width,
  height,
  image: "/images/twitter-images/home-demo-card@2x.webp",
  imageSmall: "/images/twitter-images/home-demo-card.png",
  motion: "/images/twitter-images/home-demo-card-motion.webp",
};

const tree = await paint(<DemoCard />, { width, height });

export const scene = paintScene(tree);

/** A draw step: `step` indexes the scene's steps, `order` the paint tree's. */
export type Layer = {
  step: number;
  order: number;
  name: string;
  label: string;
  value: string;
  swatch?: string;
};

function describeLayer(drawable: Drawable): Pick<Layer, "label" | "value" | "swatch"> {
  switch (drawable.type) {
    case "glyphs": {
      const { font, fontSize } = drawable.run;
      const { value, swatch } = describePaint(drawable.paint);

      return {
        label: drawable.role,
        value: `${font.family ?? "unnamed"} ${font.weight} · ${round(fontSize)}px · ${value}`,
        swatch,
      };
    }
    case "masked": {
      const [content] = drawable.content;
      const [mask] = drawable.mask;

      if (content?.type === "fill" && mask) {
        const { value, swatch } = describePaint(content.paint);

        return {
          label: drawable.role,
          value: `${value} · mask: ${mask.type === "fill" ? mask.shape.type : mask.type}`,
          swatch,
        };
      }
      break;
    }
  }

  return describeDrawable(drawable);
}

let sceneStep = -1;

export const layers = tree.steps.flatMap((step, order): Layer[] => {
  // paintScene drops draws with nothing to draw, so the scene's step indices skip them too.
  if (step.type === "draw" && step.drawables.length === 0) {
    return [];
  }
  sceneStep++;

  const [first] = step.type === "draw" ? step.drawables : [];
  const node = scene.nodes[scene.steps[sceneStep]?.node ?? -1];

  return first
    ? [{ step: sceneStep, order, name: node?.name ?? step.node.type, ...describeLayer(first) }]
    : [];
});

/** Every box below the root, with the rectangle layout gave it. */
export const boxes = inspectPaint(tree).nodes.flatMap((node) =>
  node.kind === "box" && node.depth > 0
    ? [{ name: node.name, depth: node.depth, bounds: node.bounds }]
    : [],
);
