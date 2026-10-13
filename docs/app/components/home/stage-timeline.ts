/** Where each phase of the homepage stage sits along its scroll progress, from 0 to 1. */
export const STAGE = {
  heroOut: [0, 0.08],
  steps: [0.16, 0.24],
  /** The source text hands over to the particles drawn over it. */
  handoff: 0.012,
  flyStart: 0.02,
  /** The last character leaves this long after the first. */
  flySpread: 0.18,
  flyDuration: 0.12,
  layout: [0.28, 0.38],
  tiltIn: [0.3, 0.42],
  paint: [0.5, 0.6],
  separate: [0.52, 0.64],
  merge: [0.72, 0.82],
  tiltOut: [0.74, 0.86],
  fill: [0.8, 0.88],
  image: [0.86, 0.93],
} as const;

/** The progress at which each step of the pipeline becomes the current one. */
export const STEP_STARTS = [0, 0.28, 0.5, 0.76];

/** 0 before `from`, 1 after `to`, eased in and out between. */
export function ramp(progress: number, [from, to]: readonly [number, number]) {
  const t = Math.min(Math.max((progress - from) / (to - from), 0), 1);

  return t * t * (3 - 2 * t);
}
