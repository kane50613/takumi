import { container, percentage } from "../helpers";
import { fromStaticMarkup } from "./markup";
import type { Node } from "../types";

const isWhitespaceOnlyText = (node: Node): boolean =>
  "text" in node && typeof node.text === "string" && node.text.trim() === "";

export function fromHtml(html: string) {
  const { nodes: parsed, stylesheets } = fromStaticMarkup(html);
  // A template literal leaves whitespace around the markup, which parses into
  // text roots. Dropping them keeps a newline-wrapped element a single root,
  // the way `from_html` does in the Rust crate (kane50613/takumi#1283).
  const nodes = [...parsed];

  while (nodes[0] && isWhitespaceOnlyText(nodes[0])) {
    nodes.shift();
  }
  while (nodes.at(-1) && isWhitespaceOnlyText(nodes.at(-1) as Node)) {
    nodes.pop();
  }

  if (nodes.length === 0) {
    return {
      node: container({}),
      stylesheets,
    };
  }

  if (nodes.length === 1 && nodes[0]) {
    return {
      node: nodes[0],
      stylesheets,
    };
  }

  return {
    node: container({
      style: {
        width: percentage(100),
        height: percentage(100),
      },
      children: nodes,
    }),
    stylesheets,
  };
}
