"use client";

import { type MotionValue, useScroll, useSpring } from "motion/react";
import type { RefObject } from "react";

/** Scroll progress through a pinned `track`, spring-eased like GSAP's `scrub` so scenes trail the wheel. */
export function useScrub(track: RefObject<HTMLElement | null>): MotionValue<number> {
  const { scrollYProgress } = useScroll({ target: track, offset: ["start start", "end end"] });

  return useSpring(scrollYProgress, { stiffness: 170, damping: 32, mass: 0.6 });
}
