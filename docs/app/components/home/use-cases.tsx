"use client";

import { useId, useMemo, useState } from "react";
import { sceneSteps, SvgIds } from "~/components/mdx/paint/paint-svg";
import { stepDetail } from "~/components/mdx/paint/parts";
import type { PaintScene } from "~/components/mdx/paint/scene";

/** Interactive draw-step replay bench driven by the real build-time `PaintScene`. */
export function PaintBench({ scene }: { scene: PaintScene }) {
  const drawIndices = useMemo(
    () => scene.steps.flatMap((step, index) => (step.type === "draw" ? [index] : [])),
    [scene.steps],
  );
  const totalDraws = drawIndices.length;
  const [drawNumber, setDrawNumber] = useState(totalDraws);
  const ids = new SvgIds(useId());
  const sceneCount =
    drawNumber >= totalDraws ? scene.steps.length : (drawIndices[drawNumber - 1] ?? 0) + 1;
  const currentStepIndex = drawIndices[drawNumber - 1] ?? scene.steps.length - 1;
  const current = scene.steps[currentStepIndex];
  const quad = current ? scene.nodes[current.node]?.quad : undefined;

  return (
    <div className="space-y-4">
      <div className="overflow-hidden rounded-2xl border border-foreground/15 bg-[#181310] shadow-lg">
        <svg
          role="img"
          aria-label={`Forged component replayed through draw step ${drawNumber} of ${totalDraws}`}
          viewBox={`0 0 ${scene.width} ${scene.height}`}
          className="h-auto w-full"
        >
          {sceneSteps(scene, sceneCount, ids)}
          {quad && (
            <polygon
              points={quad.map(({ x, y }) => `${x},${y}`).join(" ")}
              fill="rgba(255, 169, 68, 0.14)"
              stroke="#ffa944"
              strokeWidth="4"
              strokeDasharray="10 8"
            />
          )}
        </svg>
      </div>

      <div className="flex flex-wrap items-center justify-between gap-4">
        <label className="flex min-w-[16rem] flex-1 items-center gap-4 font-mono text-base text-foreground">
          <span className="shrink-0 font-semibold text-primary">
            step {String(drawNumber).padStart(2, "0")}/{String(totalDraws).padStart(2, "0")}
          </span>
          <input
            type="range"
            aria-label="Draw steps replayed"
            min={1}
            max={totalDraws}
            value={drawNumber}
            onChange={(event) => setDrawNumber(Number(event.target.value))}
            className="h-2 w-full cursor-pointer accent-primary"
          />
        </label>

        <span className="font-mono text-base text-muted-foreground">
          {current ? stepDetail(scene, current) : "complete"}
        </span>
      </div>
    </div>
  );
}
