"use client";

import { motion, type MotionValue, useReducedMotion, useScroll, useTransform } from "motion/react";
import { Children, type ReactNode, useRef } from "react";
import { cn } from "~/lib/utils";
import { GUTTER } from "./typography";

/** Scroll held at each end before the first wipe starts and after the last one lands, in panels. */
const HOLD = 0.35;

/** Panels pinned under the header, each wiped up over the one before as the page scrolls. */
export function Gallery({ children }: { children: ReactNode }) {
  const reduceMotion = useReducedMotion();
  const track = useRef<HTMLDivElement>(null);
  const { scrollYProgress } = useScroll({ target: track, offset: ["start start", "end end"] });
  const panels = Children.toArray(children);
  const turns = useTransform(
    scrollYProgress,
    (progress) => progress * (panels.length - 1 + 2 * HOLD) - HOLD,
  );

  if (reduceMotion) {
    return <div className={cn("flex flex-col gap-[10svh]", GUTTER)}>{panels}</div>;
  }

  return (
    <div ref={track} style={{ height: `${panels.length * 130}svh` }}>
      <div className="sticky top-14 h-[calc(100svh-3.5rem)] overflow-hidden">
        {panels.map((panel, index) => (
          <GalleryPanel key={index} turns={turns} index={index} last={index === panels.length - 1}>
            {panel}
          </GalleryPanel>
        ))}
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
  const clipPath = useTransform(reveal, (t) => `inset(${(1 - easeOut(t)) * 100}% 0 0 0)`);
  const transform = useTransform(
    [reveal, recede],
    ([t, r]: number[]) =>
      `translateY(${(1 - easeOut(t ?? 1)) * 18}svh) scale(${1 - easeOut(r ?? 0) * 0.08})`,
  );
  const opacity = useTransform(recede, (r) => 1 - easeOut(r) * 0.65);

  return (
    <motion.div className="absolute inset-0 bg-background" style={{ clipPath }}>
      <motion.div
        className={cn("flex size-full items-center justify-center", GUTTER)}
        style={{ transform, opacity }}
      >
        {children}
      </motion.div>
    </motion.div>
  );
}

function clamp01(value: number) {
  return Math.min(Math.max(value, 0), 1);
}

function easeOut(t: number) {
  return 1 - (1 - t) ** 3;
}
