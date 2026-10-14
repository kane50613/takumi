"use client";

import { motion, useReducedMotion, useScroll, useTransform } from "motion/react";
import { Children, type ReactNode, useRef } from "react";
import { cn } from "~/lib/utils";
import { GUTTER } from "./typography";

/** Panels pinned under the header that slide sideways as the page scrolls past them. */
export function Gallery({ children }: { children: ReactNode }) {
  const reduceMotion = useReducedMotion();
  const track = useRef<HTMLDivElement>(null);
  const { scrollYProgress } = useScroll({ target: track, offset: ["start start", "end end"] });
  const transform = useTransform(
    scrollYProgress,
    (progress) => `translateX(calc(${progress} * (100vw - 100%)))`,
  );

  if (reduceMotion) {
    return <div className={cn("flex flex-col gap-[7svh]", GUTTER)}>{children}</div>;
  }

  return (
    <div ref={track} style={{ height: `${Children.count(children) * 80}svh` }}>
      <div className="sticky top-14 flex h-[calc(100svh-3.5rem)] items-center overflow-hidden">
        <motion.div
          className={cn("flex w-max items-center gap-[6vw]", GUTTER)}
          style={{ transform }}
        >
          {children}
        </motion.div>
      </div>
    </div>
  );
}
