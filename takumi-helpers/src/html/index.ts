import type { FromJsxResult } from "../jsx";
import { fromStaticMarkup } from "./markup";
import { rootResult } from "../root";
import type { Node } from "../types";

export interface FromHtmlResult extends FromJsxResult {}

const isWhitespaceOnlyText = (node: Node | undefined): boolean =>
  node !== undefined && "text" in node && typeof node.text === "string" && node.text.trim() === "";

export function fromHtml(html: string): FromHtmlResult {
  const converted = fromStaticMarkup(html);

  while (isWhitespaceOnlyText(converted.nodes[0])) {
    converted.nodes.shift();
  }

  while (isWhitespaceOnlyText(converted.nodes.at(-1))) {
    converted.nodes.pop();
  }

  return rootResult(converted);
}
