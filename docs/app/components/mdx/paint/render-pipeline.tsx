"use client";

import { useId } from "react";
import { SvgIds, sceneSteps } from "./paint-svg";
import { Boxes, Canvas } from "./parts";
import { Pipeline } from "./pipeline";
import type { PaintScene } from "./scene";

const OUTPUTS = [
  { pkg: "takumi-js", output: "Image" },
  { pkg: "takumi-pdf", output: "PDF" },
  { pkg: "takumi-paint", output: "Paint tree" },
];

function Nodes({ scene }: { scene: PaintScene }) {
  return (
    <ul className="w-full max-w-sm space-y-1.5 rounded-lg border bg-fd-card p-4 font-mono text-xs shadow-sm">
      {scene.nodes.map((node, index) => (
        <li
          key={index}
          className="paint-in flex items-center gap-1.5"
          style={{ paddingLeft: `${node.depth}rem`, animationDelay: `${index * 60}ms` }}
        >
          <span className="rounded-md bg-fd-primary/10 px-1.5 text-fd-primary">{node.kind}</span>
          <span>{node.name}</span>
        </li>
      ))}
    </ul>
  );
}

function Output({
  scene,
  pkg,
  output,
  index,
}: {
  scene: PaintScene;
  pkg: string;
  output: string;
  index: number;
}) {
  const ids = new SvgIds(useId());

  return (
    <figure
      className="paint-in flex flex-col items-center gap-2"
      style={{ animationDelay: `${index * 80}ms` }}
    >
      <Canvas scene={scene} label={`The card as ${output.toLowerCase()} output`}>
        {sceneSteps(scene, scene.steps.length, ids)}
      </Canvas>
      <figcaption className="font-mono text-xs">
        <span className="text-fd-primary">{pkg}</span>
        <span className="text-fd-muted-foreground"> · {output}</span>
      </figcaption>
    </figure>
  );
}

/** How one render goes from input to the three outputs, one stage at a time. */
export function RenderPipelineView({ scene }: { scene: PaintScene }) {
  return (
    <Pipeline
      stages={[
        {
          title: "Input",
          caption:
            "A render starts from JSX, an HTML string, or a node tree of container, text, and image nodes.",
          view: <Nodes scene={scene} />,
        },
        {
          title: "Lay out",
          caption:
            "Takumi applies the styles, then lays out every box with Flexbox, Grid, block, float, table, or inline layout.",
          view: <Boxes scene={scene} />,
        },
        {
          title: "Output",
          caption: "The same layout becomes an image, a PDF, or a paint tree.",
          view: (
            <div className="grid w-full grid-cols-1 gap-4 sm:grid-cols-3">
              {OUTPUTS.map((item, index) => (
                <Output
                  key={item.pkg}
                  scene={scene}
                  pkg={item.pkg}
                  output={item.output}
                  index={index}
                />
              ))}
            </div>
          ),
        },
      ]}
    />
  );
}
