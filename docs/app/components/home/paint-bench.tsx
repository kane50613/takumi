"use client";

import {
  useMotionValueEvent,
  useReducedMotion,
  useScroll,
  useTransform,
  motion,
} from "motion/react";
import { type ReactNode, useId, useMemo, useRef, useState } from "react";
import { sceneSteps, SvgIds } from "~/components/mdx/paint/paint-svg";
import { stepDetail } from "~/components/mdx/paint/parts";
import type { PaintScene } from "~/components/mdx/paint/scene";
import { cn } from "~/lib/utils";

/** Draw-step replay of the real build-time `PaintScene` under `title`, advanced by scrolling past it. */
export function PaintBench({ scene, title }: { scene: PaintScene; title: ReactNode }) {
  const drawIndices = useMemo(
    () => scene.steps.flatMap((step, index) => (step.type === "draw" ? [index] : [])),
    [scene.steps],
  );
  const totalDraws = drawIndices.length;
  const reduceMotion = useReducedMotion();
  const track = useRef<HTMLDivElement>(null);
  const { scrollYProgress } = useScroll({ target: track, offset: ["start start", "end end"] });
  const transform = useTransform(scrollYProgress, (value) => `scaleX(${value})`);
  const [drawNumber, setDrawNumber] = useState(1);
  const ids = new SvgIds(useId());

  useMotionValueEvent(scrollYProgress, "change", (value) => {
    setDrawNumber(Math.min(totalDraws, Math.max(1, Math.ceil(value * totalDraws))));
  });

  const shown = reduceMotion ? totalDraws : drawNumber;
  const sceneCount = shown >= totalDraws ? scene.steps.length : (drawIndices[shown - 1] ?? 0) + 1;
  const current = scene.steps[drawIndices[shown - 1] ?? scene.steps.length - 1];
  const quad = current ? scene.nodes[current.node]?.quad : undefined;

  return (
    <div ref={track} className={cn("relative", !reduceMotion && "h-[340svh]")}>
      <div
        className={cn(
          !reduceMotion && "sticky top-14 flex h-[calc(100svh-3.5rem)] flex-col justify-center",
        )}
      >
        <div className="mx-auto w-full max-w-[min(64rem,calc((100svh-20rem)*1200/630))] space-y-5">
          {title}
          <div className="overflow-hidden rounded-lg border border-foreground/12 bg-[#181310]">
            <svg
              role="img"
              aria-label={`Forged component replayed through draw step ${shown} of ${totalDraws}`}
              viewBox={`0 0 ${scene.width} ${scene.height}`}
              className="h-auto w-full"
            >
              {sceneSteps(scene, sceneCount, ids, { className: "paint-reveal", stagger: 0 })}
              {quad && shown < totalDraws && (
                <polygon
                  points={quad.map(({ x, y }) => `${x},${y}`).join(" ")}
                  fill="rgba(255, 93, 46, 0.14)"
                  stroke="#ff5d2e"
                  strokeWidth="4"
                  strokeDasharray="10 8"
                />
              )}
            </svg>
          </div>

          <div className="flex flex-wrap items-center justify-between gap-4 font-sans text-base tabular-nums">
            <div className="flex min-w-[16rem] flex-1 items-center gap-4">
              <span className="shrink-0 font-semibold text-primary tabular-nums">
                step {String(shown).padStart(2, "0")}/{String(totalDraws).padStart(2, "0")}
              </span>
              <div
                aria-hidden
                className="h-1.5 w-full overflow-hidden rounded-full bg-foreground/10"
              >
                <motion.div
                  className="h-full origin-left rounded-full bg-primary"
                  style={{ transform: reduceMotion ? "scaleX(1)" : transform }}
                />
              </div>
            </div>

            <span className="text-muted-foreground">
              {current && shown < totalDraws ? stepDetail(scene, current) : "complete"}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
