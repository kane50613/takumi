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

/** Stages that play in turn while on screen, until the reader picks one or focuses the controls. */
export function Pipeline({ stages }: { stages: PipelineStage[] }) {
  const [stage, setStage] = useState(0);
  const [picked, setPicked] = useState(false);
  const [focused, setFocused] = useState(false);
  const surface = useRef<HTMLElement>(null);
  const autoplay = useAutoplayAllowed(surface);
  const progress = useProgress({
    duration: STAGE_MS,
    running: autoplay && !picked && !focused,
    key: stage,
    onDone: () => setStage((stage + 1) % stages.length),
  });

  return (
    <Surface ref={surface}>
      <div
        className="grid gap-3 border-b px-4 pt-3 pb-2.5"
        style={{ gridTemplateColumns: `repeat(${stages.length}, minmax(0, 1fr))` }}
        onFocus={() => setFocused(true)}
        onBlur={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget)) {
            setFocused(false);
          }
        }}
      >
        {stages.map((item, index) => (
          <button
            key={item.title}
            type="button"
            aria-pressed={index === stage}
            onClick={() => {
              setStage(index);
              setPicked(true);
            }}
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
              value={
                index < stage || (index === stage && picked)
                  ? 100
                  : index === stage
                    ? progress * 100
                    : 0
              }
              className="w-full [&_[data-slot=progress-indicator]]:bg-fd-primary [&_[data-slot=progress-indicator]]:transition-none [&_[data-slot=progress-track]]:bg-fd-foreground/10"
            />
          </button>
        ))}
      </div>
      <Stage className="grid grid-cols-1">
        {stages.map((item, index) => (
          <Layer key={`${index}-${index === stage}`} active={index === stage}>
            <div className="flex w-full justify-center">{item.view}</div>
          </Layer>
        ))}
      </Stage>
      <figcaption className="grid border-t px-4 py-3 text-xs text-fd-muted-foreground">
        {stages.map((item, index) => (
          <Layer key={item.title} active={index === stage}>
            {item.caption}
          </Layer>
        ))}
      </figcaption>
    </Surface>
  );
}

/** One of several children stacked in the same grid cell, so the tallest sets the height. */
function Layer({ active, children }: { active: boolean; children: ReactNode }) {
  return (
    <div
      inert={!active}
      className={cn("col-start-1 row-start-1 self-center", active ? "paint-in" : "invisible")}
    >
      {children}
    </div>
  );
}
