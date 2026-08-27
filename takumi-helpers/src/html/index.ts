import { hideStylesheetsAlias, warnStylesheetsDeprecated } from "../deprecation";
import { container, percentage } from "../helpers";
import { fromStaticMarkup } from "./markup";
import type { Node } from "../types";

export interface FromHtmlResult {
  node: Node;
  css: string[];
  /** @deprecated Use `css` instead. */
  stylesheets: string[];
}

const isWhitespaceOnlyText = (node: Node): boolean =>
  "text" in node && typeof node.text === "string" && node.text.trim() === "";

export function fromHtml(html: string): FromHtmlResult {
  const { nodes, css } = fromStaticMarkup(html);

  while (nodes[0] && isWhitespaceOnlyText(nodes[0])) {
    nodes.shift();
  }
  while (nodes.at(-1) && isWhitespaceOnlyText(nodes.at(-1) as Node)) {
    nodes.pop();
  }

  let node: Node;
  if (nodes.length === 0) {
    node = container({});
  } else if (nodes.length === 1 && nodes[0]) {
    node = nodes[0];
  } else {
    node = container({
      style: {
        display: "block",
        width: percentage(100),
        height: percentage(100),
      },
      children: nodes,
    });
  }

  const aliased: FromHtmlResult = {
    node,
    css,
    get stylesheets() {
      warnStylesheetsDeprecated();
      return css;
    },
  };

  hideStylesheetsAlias(aliased);
  return aliased;
}
