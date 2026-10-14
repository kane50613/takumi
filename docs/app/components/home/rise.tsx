"use client";

import { motion, useReducedMotion, useScroll, useTransform } from "motion/react";
import { type ReactNode, useRef } from "react";

/** Children rising into place as they scroll into view, starting `lag` pixels lower than others. */
export function Rise({
  children,
  lag = 0,
  className,
}: {
  children: ReactNode;
  lag?: number;
  className?: string;
}) {
  const reduceMotion = useReducedMotion();
  const ref = useRef<HTMLDivElement>(null);
  const { scrollYProgress } = useScroll({ target: ref, offset: ["start end", "start 55%"] });
  const transform = useTransform(
    scrollYProgress,
    (progress) => `translateY(${(1 - progress) * (96 + lag)}px)`,
  );

  return (
    <motion.div
      ref={ref}
      className={className}
      style={reduceMotion ? undefined : { transform, opacity: scrollYProgress }}
    >
      {children}
    </motion.div>
  );
}
