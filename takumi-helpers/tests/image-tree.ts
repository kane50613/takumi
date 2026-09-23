import type { Node } from "../src/types";

/** A container holding one image node per `src`. */
export const imageTree = (...srcs: string[]): Node => ({
  type: "container",
  children: srcs.map((src) => ({ type: "image", src })),
});
