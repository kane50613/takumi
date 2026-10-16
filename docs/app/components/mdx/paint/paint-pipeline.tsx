"use client";

import { useId, useRef, useState } from "react";
import { Progress } from "~/components/ui/progress";
import { cn } from "~/lib/utils";
import { SvgIds, sceneSteps } from "./paint-svg";
import { Canvas, Stage, Surface, useAutoplayAllowed, useProgress } from "./parts";
import type { PaintScene } from "./scene";

const STAGES = [
  {
    title: "Lay out",
    caption: "Layout places every box, paragraph, and image as a rectangle on the canvas.",
  },
  {
    title: "Paint tree",
    caption: "Each node lists what it draws, with every CSS value already resolved.",
  },
  {
    title: "Draw",
    caption: "Your renderer replays tree.steps in order, from the bottom up.",
  },
] as const;

const STAGE_MS = 4500;

function Layout({ scene }: { scene: PaintScene }) {
  return (
    <Canvas scene={scene} label="Every node's rectangle on the canvas">
      {scene.nodes.map((node, index) => (
        <polygon
          key={index}
          points={node.quad.map(({ x, y }) => `${x},${y}`).join(" ")}
          className="paint-in fill-fd-primary/5 stroke-fd-primary"
          style={{ animationDelay: `${index * 80}ms` }}
          strokeWidth={1}
          strokeDasharray={node.kind === "text" ? "3 3" : undefined}
        />
      ))}
    </Canvas>
  );
}

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
  const [stage, setStage] = useState(0);
  const surface = useRef<HTMLElement>(null);
  const ids = new SvgIds(useId());
  const autoplay = useAutoplayAllowed(surface);
  const progress = useProgress({
    duration: STAGE_MS,
    running: autoplay,
    key: stage,
    onDone: () => setStage((stage + 1) % STAGES.length),
  });

  return (
    <Surface ref={surface}>
      <div className="grid grid-cols-3 gap-3 border-b px-4 pt-3 pb-2.5">
        {STAGES.map((item, index) => (
          <button
            key={item.title}
            type="button"
            aria-pressed={index === stage}
            onClick={() => setStage(index)}
            className="group flex w-full flex-col items-stretch gap-2 text-left"
          >
            <span
              className={cn(
                "font-mono text-xs transition-colors duration-150",
                index === stage
                  ? "text-fd-primary"
                  : "text-fd-muted-foreground group-hover:text-fd-foreground",
              )}
            >
              {index + 1}. {item.title}
            </span>
            <Progress
              aria-label={`${item.title} progress`}
              value={index < stage ? 100 : index === stage ? progress * 100 : 0}
              className="w-full [&_[data-slot=progress-indicator]]:bg-fd-primary [&_[data-slot=progress-indicator]]:transition-none [&_[data-slot=progress-track]]:bg-fd-foreground/10"
            />
          </button>
        ))}
      </div>
      <Stage className="min-h-80">
        <div key={stage} className="paint-in flex w-full justify-center">
          {stage === 0 && <Layout scene={scene} />}
          {stage === 1 && <Tree scene={scene} />}
          {stage === 2 && (
            <Canvas scene={scene} label="The card, drawn from the steps">
              {sceneSteps(scene, scene.steps.length, ids, { className: "paint-in", stagger: 140 })}
            </Canvas>
          )}
        </div>
      </Stage>
      <figcaption className="border-t px-4 py-3 text-xs text-fd-muted-foreground">
        {STAGES[stage]?.caption}
      </figcaption>
    </Surface>
  );
}
