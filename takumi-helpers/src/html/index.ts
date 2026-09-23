import { fromStaticMarkup } from "./markup";
import { rootResult } from "../root";
import type { Node } from "../types";

export interface FromHtmlResult {
  node: Node;
  css: string[];
  /** @deprecated Use `css` instead. */
  stylesheets: string[];
}

const isWhitespaceOnlyText = (node: Node | undefined): boolean =>
  node !== undefined && "text" in node && typeof node.text === "string" && node.text.trim() === "";

export function fromHtml(html: string): FromHtmlResult {
  const { nodes, css } = fromStaticMarkup(html);

  while (isWhitespaceOnlyText(nodes[0])) {
    nodes.shift();
  }

  while (isWhitespaceOnlyText(nodes.at(-1))) {
    nodes.pop();
  }

  return rootResult(nodes, css);
}
