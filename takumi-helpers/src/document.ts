import type { ReactNode } from "react";
import { fromJsx, type FromJsxOptions } from "./jsx";
import type { CssInput, Node, ReactElementLike } from "./types";

/** A document a takumi package renders: a node tree, JSX, or HTML markup. */
export type DocumentInput = Node | ReactNode | ReactElementLike | string;

/**
 * Whether `input` is a takumi node tree rather than JSX, which also carries a `type`.
 *
 * @internal
 */
export function isNode(input: unknown): input is Node {
  if (typeof input !== "object" || input === null || !("type" in input) || "$$typeof" in input) {
    return false;
  }

  return input.type === "container" || input.type === "text" || input.type === "image";
}

/**
 * `input` as a node tree or HTML a binding reads, with the CSS JSX brings: JSX rendered, the rest as is.
 *
 * @internal
 */
export async function resolveDocument(
  input: DocumentInput,
  jsx?: FromJsxOptions,
): Promise<{ node: Node | string; css: string[] }> {
  if (typeof input === "string" || isNode(input)) {
    return { node: input, css: [] };
  }

  return fromJsx(input, jsx);
}

/**
 * The `css` option followed by the sheets a document brings, or `undefined` for none.
 *
 * @internal
 */
export function sheetsOf(
  css: CssInput | readonly CssInput[] | undefined,
  documentCss: readonly CssInput[],
): CssInput[] | undefined {
  const own: readonly CssInput[] = css === undefined ? [] : isCssList(css) ? css : [css];
  const sheets = [...own, ...documentCss];

  return sheets.length > 0 ? sheets : undefined;
}

/** Narrows the `css` option, which takes one entry or a list of them. */
function isCssList(css: CssInput | readonly CssInput[]): css is readonly CssInput[] {
  return Array.isArray(css);
}
