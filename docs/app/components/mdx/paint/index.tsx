/** Paint explainers for MDX. Each paints its example with takumi-paint while the site builds. */
import { type LocalSpaceFrame, LocalSpaceView } from "./local-space";
import { PaintPipelineView } from "./paint-pipeline";
import { RenderPipelineView } from "./render-pipeline";
import { type PaintScene, paintScene } from "./scene";
import { StepReplayView } from "./step-replay";
import { TextRunsView } from "./text-runs";
import { tree as card } from "./examples/card";
import { tree as paragraph } from "./examples/paragraph";
import { paintRotated, ROTATIONS } from "./examples/rotated";

const cardScene = paintScene(card);

export function PaintPipeline() {
  return <PaintPipelineView scene={cardScene} />;
}

export function RenderPipeline() {
  return <RenderPipelineView scene={cardScene} />;
}

export function StepReplay() {
  return <StepReplayView scene={cardScene} />;
}

export async function LocalSpace() {
  const scenes: PaintScene[] = [];

  for (const degrees of ROTATIONS) scenes.push(paintScene(await paintRotated(degrees)));

  const [first] = scenes;
  const badge = first?.nodes.findIndex((node) => node.name === "div#badge") ?? -1;

  if (!first || badge < 0) throw new Error("The rotated example has no #badge node");

  const frames = scenes.flatMap((scene, index): LocalSpaceFrame[] => {
    const node = scene.nodes[badge];
    const degrees = ROTATIONS[index];

    return node && degrees !== undefined
      ? [
          {
            degrees,
            transform: node.transform,
            width: node.width,
            height: node.height,
            bounds: node.bounds,
            drawables: node.drawables,
          },
        ]
      : [];
  });
  const backdrop = { ...first, steps: first.steps.filter((step) => step.node !== badge) };

  return <LocalSpaceView backdrop={backdrop} frames={frames} />;
}

export function TextRuns() {
  return <TextRunsView scene={paintScene(paragraph)} />;
}
