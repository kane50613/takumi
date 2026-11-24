import { isNode, type Node, type ReactElementLike } from "@takumi-rs/helpers";
import { fromJsx } from "@takumi-rs/helpers/jsx";
import type { ReactNode } from "react";

/**
 * A document a takumi package renders: a node tree, JSX, or HTML markup.
 *
 * @internal
 */
export type DocumentInput = Node | ReactNode | ReactElementLike | string;

/**
 * `input` as a node tree or HTML a binding reads, with the CSS JSX brings: JSX rendered, the rest as is.
 *
 * @internal
 */
export async function resolveDocument(
  input: DocumentInput,
): Promise<{ node: Node | string; css: string[] }> {
  if (typeof input === "string" || isNode(input)) {
    return { node: input, css: [] };
  }

  return fromJsx(input);
}
