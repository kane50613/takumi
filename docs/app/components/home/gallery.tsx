"use client";

import {
  motion,
  type MotionValue,
  useMotionValue,
  useReducedMotion,
  useTransform,
} from "motion/react";
import { Children, createContext, type ReactNode, useContext, useRef } from "react";
import { cn } from "~/lib/utils";
import { useScrub } from "./scrub";
import { GUTTER } from "./typography";

/** Scroll held at each end before the first wipe starts and after the last one lands, in panels. */
const HOLD = 0.35;
/** Scroll length each panel takes. */
const PANEL_SVH = 90;

/** The enclosing panel's progress: 0 as it starts wiping in, 1 once settled, 2 once covered. */
const PanelProgress = createContext<MotionValue<number> | null>(null);

/** Panels pinned under `title`, each wiped up over the one before as the page scrolls. */
export function Gallery({ title, children }: { title: ReactNode; children: ReactNode }) {
  const reduceMotion = useReducedMotion();
  const track = useRef<HTMLDivElement>(null);
  const scrollYProgress = useScrub(track);
  const panels = Children.toArray(children);
  const turns = useTransform(
    scrollYProgress,
    (progress) => progress * (panels.length - 1 + 2 * HOLD) - HOLD,
  );

  if (reduceMotion) {
    return (
      <div className={cn("flex flex-col gap-[10svh]", GUTTER)}>
        {title}
        {panels}
      </div>
    );
  }

  return (
    <div ref={track} className="relative" style={{ height: `${panels.length * PANEL_SVH}svh` }}>
      {panels.map((_, index) => (
        <div
          key={index}
          aria-hidden
          className="pointer-events-none absolute inset-x-0 h-px snap-start"
          style={{
            top: `calc(${(index + HOLD) / (panels.length - 1 + 2 * HOLD)} * (${panels.length * PANEL_SVH}svh - 100svh + 3.5rem))`,
          }}
        />
      ))}
      <div className="sticky top-14 flex h-[calc(100svh-3.5rem)] flex-col overflow-hidden">
        <div className={cn("pt-[5svh]", GUTTER)}>{title}</div>
        <div className="relative flex-1">
          {panels.map((panel, index) => (
            <GalleryPanel
              key={index}
              turns={turns}
              index={index}
              last={index === panels.length - 1}
            >
              {panel}
            </GalleryPanel>
          ))}
        </div>
      </div>
    </div>
  );
}

/** One stacked panel, wiped in as `turns` reaches `index`, then pushed back unless `last`. */
function GalleryPanel({
  turns,
  index,
  last,
  children,
}: {
  turns: MotionValue<number>;
  index: number;
  last: boolean;
  children: ReactNode;
}) {
  const reveal = useTransform(turns, (turn) => (index === 0 ? 1 : clamp01(turn - index + 1)));
  const recede = useTransform(turns, (turn) => (last ? 0 : clamp01(turn - index)));
  const clipPath = useTransform(reveal, (t) => `inset(${(1 - smooth(t)) * 100}% 0 0 0)`);
  const transform = useTransform(
    [reveal, recede],
    ([t, r]: number[]) =>
      `translateY(${(1 - smooth(t ?? 1)) * 8}svh) scale(${1 - smooth(r ?? 0) * 0.08})`,
  );
  const opacity = useTransform([reveal, recede], ([t, r]: number[]) =>
    Math.min(clamp01((t ?? 1) * 2), 1 - smooth(r ?? 0) * 0.65),
  );
  const progress = useTransform(turns, (turn) => Math.min(Math.max(turn - index + 1, 0), 2));

  return (
    <motion.div className="absolute inset-0 bg-background" style={{ clipPath }}>
      <motion.div
        className={cn("flex size-full items-center justify-center", GUTTER)}
        style={{ transform, opacity }}
      >
        <PanelProgress.Provider value={progress}>{children}</PanelProgress.Provider>
      </motion.div>
    </motion.div>
  );
}

/** Drifts children by `depth` against the enclosing panel's scroll, so its layers slide apart. */
export function Parallax({
  depth,
  className,
  children,
}: {
  depth: number;
  className?: string;
  children: ReactNode;
}) {
  const settled = useMotionValue(1);
  const progress = useContext(PanelProgress) ?? settled;
  const transform = useTransform(
    progress,
    (t) => `translateY(${((1 - t) * depth * 10).toFixed(2)}svh)`,
  );

  return (
    <motion.div className={className} style={{ transform }}>
      {children}
    </motion.div>
  );
}

function clamp01(value: number) {
  return Math.min(Math.max(value, 0), 1);
}

function smooth(t: number) {
  return t * t * (3 - 2 * t);
}
