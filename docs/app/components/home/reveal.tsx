"use client";

import { motion, useReducedMotion } from "motion/react";
import type { ReactNode } from "react";

const EASE_OUT_STRONG = [0.23, 1, 0.32, 1] as const;

/** Children unmasked from the bottom up once they scroll into view, `delay` seconds after others. */
export function Reveal({
  children,
  delay = 0,
  className,
}: {
  children: ReactNode;
  delay?: number;
  className?: string;
}) {
  const reduceMotion = useReducedMotion();

  if (reduceMotion) {
    return <div className={className}>{children}</div>;
  }

  return (
    <motion.div
      className={className}
      initial={{ clipPath: "inset(100% 0% 0% 0%)", transform: "translateY(72px)" }}
      whileInView={{ clipPath: "inset(0% 0% 0% 0%)", transform: "translateY(0px)" }}
      viewport={{ once: true, margin: "0px 0px -12% 0px" }}
      transition={{ duration: 1.2, ease: EASE_OUT_STRONG, delay }}
    >
      {children}
    </motion.div>
  );
}
