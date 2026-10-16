"use client";

import { Pause, Play, RotateCcw } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { Slider } from "~/components/ui/slider";
import { cn } from "~/lib/utils";
import { SvgIds, sceneSteps } from "./paint-svg";
import {
  Canvas,
  Footer,
  Panel,
  Stage,
  Surface,
  stepDetail,
  useAutoplayAllowed,
  useProgress,
} from "./parts";
import type { PaintScene } from "./scene";

const STEP_MS = 1100;

function nesting(scene: PaintScene) {
  let depth = 0;

  return scene.steps.map((step) => {
    if (step.type === "end-group" || step.type === "end-clip") depth--;
    const at = depth;
    if (step.type === "begin-group" || step.type === "begin-clip") depth++;
    return at;
  });
}

/** Replays a scene's steps one at a time, with the step list beside the canvas. */
export function StepReplayView({ scene }: { scene: PaintScene }) {
  const total = scene.steps.length;
  const [count, setCount] = useState(0);
  const [mode, setMode] = useState<"auto" | "playing" | "paused">("auto");
  const surface = useRef<HTMLElement>(null);
  const ids = new SvgIds(useId());
  const depths = nesting(scene);
  const autoplay = useAutoplayAllowed(surface);
  const finished = count >= total;
  const waiting = mode === "auto" && !autoplay;
  const playing = !finished && (mode === "playing" || (mode === "auto" && autoplay));
  const progress = useProgress({
    duration: STEP_MS,
    running: playing,
    key: count,
    onDone: () => setCount(count + 1),
  });
  const shown = waiting ? total : count;
  const current = scene.steps[shown - 1];
  const quad = current && scene.nodes[current.node]?.quad;

  const list = useRef<HTMLOListElement>(null);

  useEffect(() => {
    const container = list.current?.parentElement;
    const item = list.current?.children[shown - 1];

    if (!container || !(item instanceof HTMLElement)) return;
    container.scrollTo({
      top: item.offsetTop - container.clientHeight / 2,
      behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
    });
  }, [shown]);

  function pick(next: number) {
    setMode("paused");
    setCount(next);
  }

  return (
    <Surface ref={surface}>
      <div className="grid md:grid-cols-[minmax(0,1fr)_16rem]">
        <Stage>
          <Canvas scene={scene} label="The example card, drawn up to the selected step">
            {sceneSteps(scene, shown, ids, { className: "paint-in", stagger: 0 })}
            {quad && (
              <polygon
                key={shown}
                points={quad.map(({ x, y }) => `${x},${y}`).join(" ")}
                className="paint-in fill-fd-primary/10 stroke-fd-primary"
                strokeWidth={1.5}
                strokeDasharray="4 3"
              />
            )}
          </Canvas>
        </Stage>
        <Panel className="relative max-h-96 overflow-y-auto p-2">
          <ol ref={list} className="space-y-0.5 font-mono text-xs">
            {scene.steps.map((step, index) => (
              <li key={index}>
                <button
                  type="button"
                  onClick={() => pick(index + 1)}
                  aria-current={index === shown - 1 ? "step" : undefined}
                  style={{ paddingLeft: `${0.625 + (depths[index] ?? 0) * 0.75}rem` }}
                  className={cn(
                    "flex w-full min-w-0 flex-col items-stretch rounded-md py-1.5 pr-2 text-left transition-colors duration-150",
                    index === shown - 1
                      ? "bg-fd-primary/10 text-fd-primary"
                      : index < shown
                        ? "text-fd-foreground hover:bg-fd-muted"
                        : "text-fd-muted-foreground/60 hover:bg-fd-muted",
                  )}
                >
                  <span>{step.type}</span>
                  <span className="truncate opacity-70">{stepDetail(scene, step)}</span>
                </button>
              </li>
            ))}
          </ol>
        </Panel>
      </div>
      <Footer>
        <button
          type="button"
          aria-label={finished ? "Replay" : playing ? "Pause" : "Play"}
          onClick={() => {
            if (playing) return setMode("paused");
            if (finished || waiting) setCount(0);
            setMode("playing");
          }}
          className="flex size-8 shrink-0 items-center justify-center rounded-full bg-fd-primary text-fd-primary-foreground transition-transform duration-150 ease-out active:scale-[0.97]"
        >
          {finished ? (
            <RotateCcw className="size-3.5" />
          ) : playing ? (
            <Pause className="size-3.5" />
          ) : (
            <Play className="size-3.5" />
          )}
        </button>
        <Slider
          className="flex-1 [&_[data-slot=slider-range]]:bg-fd-primary [&_[data-slot=slider-thumb]]:border-fd-primary [&_[data-slot=slider-track]]:bg-fd-foreground/10"
          aria-label="Steps drawn"
          min={0}
          max={total}
          step={1}
          value={[playing ? count + progress : shown]}
          onValueChange={(value) =>
            pick(Math.round(Array.isArray(value) ? (value[0] ?? 0) : value))
          }
        />
      </Footer>
    </Surface>
  );
}
