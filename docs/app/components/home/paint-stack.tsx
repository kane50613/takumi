"use client";

import { type ReactNode, useEffect, useId, useRef, useState } from "react";
import { SvgIds, sceneSteps } from "~/components/mdx/paint/paint-svg";
import type { PaintScene } from "~/components/mdx/paint/scene";
import { cn } from "~/lib/utils";

/** A draw step: `step` indexes the scene's steps, `order` the paint tree's. */
export type Layer = {
  step: number;
  order: number;
  name: string;
  label: string;
  value: string;
  swatch?: string;
};

const TURN = { x: 52, z: -32 };

function clamp(value: number) {
  return Math.min(Math.max(value, 0), 1);
}

/** 0 before `from`, 1 after `to`, eased in and out between them. */
function span(progress: number, from: number, to: number) {
  const t = clamp((progress - from) / (to - from));

  return t * t * (3 - 2 * t);
}

/**
 * The scene's draw steps as a stack of layers. Scrolling turns the stack and pulls the layers
 * apart; leader lines tie each one to what it draws.
 */
export function PaintStack({
  scene,
  layers,
  children,
}: {
  scene: PaintScene;
  layers: Layer[];
  children: ReactNode;
}) {
  const track = useRef<HTMLDivElement>(null);
  const stage = useRef<HTMLDivElement>(null);
  const stack = useRef<HTMLDivElement>(null);
  const overlay = useRef<SVGSVGElement>(null);
  const planes = useRef<(HTMLDivElement | null)[]>([]);
  const outlines = useRef<(HTMLDivElement | null)[]>([]);
  const anchors = useRef<(HTMLSpanElement | null)[]>([]);
  const labels = useRef<(HTMLLIElement | null)[]>([]);
  const leaders = useRef<(SVGPathElement | null)[]>([]);
  const [active, setActive] = useState<number>();
  const ids = new SvgIds(useId());

  useEffect(() => {
    const trackElement = track.current;
    const stageElement = stage.current;

    if (!trackElement || !stageElement) {
      return;
    }

    const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)");
    const pinned = matchMedia("(min-width: 64rem)");
    let frame = 0;

    const progress = () => {
      if (reduceMotion.matches) {
        return 1;
      }
      if (pinned.matches) {
        const { top, height } = trackElement.getBoundingClientRect();

        return clamp(-top / (height - innerHeight));
      }

      const { top } = stageElement.getBoundingClientRect();

      return clamp((innerHeight * 0.95 - top) / (innerHeight * 0.75));
    };

    const apply = () => {
      frame = 0;

      const at = progress();
      const turn = span(at, 0.05, 0.4);
      const explode = span(at, 0.2, 0.7);
      const gap = stageElement.clientWidth * 0.12 * explode;
      const middle = (layers.length - 1) / 2;
      const box = overlay.current?.getBoundingClientRect();

      stack.current?.style.setProperty(
        "transform",
        `rotateX(${TURN.x * turn}deg) rotateZ(${TURN.z * turn}deg) scale(${1 - 0.22 * turn})`,
      );
      layers.forEach((_, index) => {
        planes.current[index]?.style.setProperty(
          "transform",
          `translateZ(${(index - middle) * gap}px)`,
        );
        outlines.current[index]?.style.setProperty("opacity", String(explode));
      });
      // Read every rectangle after all the writes above, so layout runs once.
      layers.forEach((_, index) => {
        const reveal = span(at, 0.6 + index * 0.04, 0.75 + index * 0.04);
        const anchor = anchors.current[index]?.getBoundingClientRect();
        const label = labels.current[index];
        const labelBox = label?.getBoundingClientRect();
        const leader = leaders.current[index];

        label?.style.setProperty("opacity", String(reveal));
        if (!leader || !anchor || !labelBox || !box) {
          return;
        }

        const ax = anchor.left + anchor.width / 2 - box.left;
        const ay = anchor.top + anchor.height / 2 - box.top;
        const lx = labelBox.left - box.left - 12;
        const ly = labelBox.top + labelBox.height / 2 - box.top;

        leader.setAttribute("d", `M${ax},${ay} L${lx - 28},${ly} H${lx}`);
        leader.style.opacity = String(reveal);
      });
    };
    const schedule = () => {
      if (frame === 0) {
        frame = requestAnimationFrame(apply);
      }
    };
    const resizeObserver = new ResizeObserver(schedule);

    apply();
    resizeObserver.observe(stageElement);
    reduceMotion.addEventListener("change", schedule);
    pinned.addEventListener("change", schedule);
    addEventListener("scroll", schedule, { passive: true });

    return () => {
      cancelAnimationFrame(frame);
      resizeObserver.disconnect();
      reduceMotion.removeEventListener("change", schedule);
      pinned.removeEventListener("change", schedule);
      removeEventListener("scroll", schedule);
    };
  }, [layers]);

  return (
    <div ref={track} className="relative lg:motion-safe:h-[260vh]">
      <div className="px-6 pt-28 pb-20 max-sm:pt-16 max-sm:pb-12 lg:motion-safe:sticky lg:motion-safe:top-0 lg:motion-safe:h-screen lg:motion-safe:py-0 lg:flex lg:flex-col lg:justify-center">
        <div className="relative max-w-275 w-full mx-auto">
          <div className="grid lg:grid-cols-[minmax(0,1fr)_24rem] gap-x-14 gap-y-12 items-center">
            <div className="lg:col-start-2 lg:row-start-1 lg:self-end">{children}</div>
            <div
              ref={stage}
              className="relative mx-auto w-full max-w-160 max-lg:my-20 [perspective:2400px] lg:col-start-1 lg:row-start-1 lg:row-span-2"
              style={{ aspectRatio: `${scene.width} / ${scene.height}` }}
            >
              <div
                ref={stack}
                className="absolute inset-0 [transform-style:preserve-3d] will-change-transform"
              >
                {layers.map((layer, index) => {
                  const step = scene.steps[layer.step];

                  return (
                    <div
                      key={layer.step}
                      ref={(element) => {
                        planes.current[index] = element;
                      }}
                      data-dim={active !== undefined && active !== index}
                      className="absolute inset-0 transition-opacity duration-200 ease-out will-change-transform data-[dim=true]:opacity-15"
                    >
                      <svg
                        viewBox={`0 0 ${scene.width} ${scene.height}`}
                        className="absolute inset-0 size-full overflow-visible"
                        aria-hidden
                      >
                        {step && sceneSteps({ ...scene, steps: [step] }, 1, ids)}
                      </svg>
                      <div
                        ref={(element) => {
                          outlines.current[index] = element;
                        }}
                        className={cn(
                          "absolute inset-0 border transition-colors duration-200 ease-out",
                          active === index ? "border-primary" : "border-white/20",
                        )}
                        style={{ opacity: 0 }}
                      >
                        <span
                          ref={(element) => {
                            anchors.current[index] = element;
                          }}
                          className="absolute right-0 bottom-0 size-1.5 translate-1/2 rounded-full bg-primary"
                        />
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>

            <ol
              aria-label="tree.steps, top layer first"
              className="flex flex-col-reverse gap-4 font-mono text-xs lg:col-start-2 lg:row-start-2 lg:self-start"
              onPointerLeave={() => setActive(undefined)}
            >
              {layers.map((layer, index) => (
                <li
                  key={layer.step}
                  ref={(element) => {
                    labels.current[index] = element;
                  }}
                  onPointerEnter={(event) => {
                    if (event.pointerType === "mouse") {
                      setActive(index);
                    }
                  }}
                  className="cursor-default"
                >
                  <div className="flex items-center gap-2 text-[#f5f1ea]">
                    <span className="text-[#a8a29a] tabular-nums">steps[{layer.order}]</span>
                    <span className="truncate">{layer.name}</span>
                  </div>
                  <div className="mt-1 flex items-center gap-2 text-[#a8a29a]">
                    {layer.swatch && (
                      <span
                        aria-hidden
                        className="size-2.5 shrink-0 rounded-[2px] ring-1 ring-white/20"
                        style={{ background: layer.swatch }}
                      />
                    )}
                    <span className="truncate">
                      {layer.label} · {layer.value}
                    </span>
                  </div>
                </li>
              ))}
            </ol>
          </div>

          <svg
            ref={overlay}
            aria-hidden
            className="pointer-events-none absolute inset-0 size-full overflow-visible max-lg:hidden"
          >
            {layers.map((layer, index) => (
              <path
                key={layer.step}
                ref={(element) => {
                  leaders.current[index] = element;
                }}
                fill="none"
                strokeWidth={1}
                className={cn(
                  "transition-[stroke] duration-200 ease-out",
                  active === index ? "stroke-primary" : "stroke-white/30",
                )}
                style={{ opacity: 0 }}
              />
            ))}
          </svg>
        </div>
      </div>
    </div>
  );
}
