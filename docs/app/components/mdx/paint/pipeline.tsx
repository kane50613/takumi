"use client";

import { type ReactNode, useRef, useState } from "react";
import { Progress } from "~/components/ui/progress";
import { cn } from "~/lib/utils";
import { Stage, Surface, useAutoplayAllowed, useProgress } from "./parts";

export interface PipelineStage {
  title: string;
  caption: string;
  view: ReactNode;
}

const STAGE_MS = 4500;

/** Stages that play in turn while on screen, each with a title, a view, and a caption. */
export function Pipeline({ stages }: { stages: PipelineStage[] }) {
  const [stage, setStage] = useState(0);
  const surface = useRef<HTMLElement>(null);
  const autoplay = useAutoplayAllowed(surface);
  const progress = useProgress({
    duration: STAGE_MS,
    running: autoplay,
    key: stage,
    onDone: () => setStage((stage + 1) % stages.length),
  });

  return (
    <Surface ref={surface}>
      <div
        className="grid gap-3 border-b px-4 pt-3 pb-2.5"
        style={{ gridTemplateColumns: `repeat(${stages.length}, minmax(0, 1fr))` }}
      >
        {stages.map((item, index) => (
          <button
            key={item.title}
            type="button"
            aria-pressed={index === stage}
            onClick={() => setStage(index)}
            className="group flex w-full flex-col items-stretch gap-2 text-left transition-transform duration-150 ease-out active:scale-[0.97]"
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
          {stages[stage]?.view}
        </div>
      </Stage>
      <figcaption className="border-t px-4 py-3 text-xs text-fd-muted-foreground">
        {stages[stage]?.caption}
      </figcaption>
    </Surface>
  );
}
