/** The homepage card, laid out once while the site builds; the stage and the live demo read from it. */
import { fromJsx } from "takumi-js/helpers/jsx";
import { paint } from "takumi-paint";
import { inspectPaint } from "~/playground/inspect-paint";
import DemoCard, {
  height,
  width,
} from "../../../../example/twitter-images/components/home-demo-card";

export const CARD = {
  width,
  height,
  image: "/images/twitter-images/home-demo-card@2x.webp",
  headline: "One component, every output.",
};

/** The card as a node tree, which the live demo edits and renders again. */
export const { node } = await fromJsx(<DemoCard />);

const tree = await paint(<DemoCard />, { width, height });
const nodes = inspectPaint(tree).nodes.filter((entry) => entry.kind === "box" && entry.depth > 0);

/** Every box below the root, with the rectangle layout gave it. */
export const boxes = nodes.map(({ depth, bounds }) => ({ depth, ...bounds }));
