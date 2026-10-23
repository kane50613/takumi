"use client";

import { useId } from "react";
import { SvgIds, sceneSteps } from "./paint-svg";
import { Boxes, Canvas } from "./parts";
import { Pipeline } from "./pipeline";
import type { PaintScene } from "./scene";

function Tree({ scene }: { scene: PaintScene }) {
  return (
    <ul className="w-full max-w-sm space-y-1.5 rounded-lg border bg-fd-card p-4 font-mono text-xs shadow-sm">
      {scene.nodes.map((node, index) => (
        <li
          key={index}
          className="paint-in flex flex-wrap items-center gap-1.5"
          style={{ paddingLeft: `${node.depth}rem`, animationDelay: `${index * 60}ms` }}
        >
          <span className="text-fd-muted-foreground">{node.kind}</span>
          <span>{node.name}</span>
          {node.drawables.map((drawable, at) => (
            <span key={at} className="rounded-md bg-fd-primary/10 px-1.5 text-fd-primary">
              {"role" in drawable ? drawable.role : drawable.type}
            </span>
          ))}
        </li>
      ))}
    </ul>
  );
}

/** What `paint()` does, one stage at a time. */
export function PaintPipelineView({ scene }: { scene: PaintScene }) {
  const ids = new SvgIds(useId());

  return (
    <Pipeline
      stages={[
        {
          title: "Lay out",
          caption: "Layout places every box, paragraph, and image as a rectangle on the canvas.",
          view: <Boxes scene={scene} />,
        },
        {
          title: "Paint tree",
          caption: "Each node lists what it draws, with every CSS value already resolved.",
          view: <Tree scene={scene} />,
        },
        {
          title: "Draw",
          caption: "Your renderer replays tree.steps in order, from the bottom up.",
          view: (
            <Canvas scene={scene} label="The card, drawn from the steps">
              {sceneSteps(scene, scene.steps.length, ids, { className: "paint-in", stagger: 140 })}
            </Canvas>
          ),
        },
      ]}
    />
  );
}
