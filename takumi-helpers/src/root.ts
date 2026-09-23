import { container, percentage } from "./helpers";
import type { Node } from "./types";

let warnedStylesheets = false;

/** Wraps converted nodes in one root, with `stylesheets` as a hidden, deprecated alias of `css`. */
export function rootResult(
  nodes: Node[],
  css: string[],
): { node: Node; css: string[]; stylesheets: string[] } {
  const result = {
    node: rootNode(nodes),
    css,
    get stylesheets() {
      if (!warnedStylesheets) {
        warnedStylesheets = true;
        console.warn("takumi: the `stylesheets` result field is deprecated, use `css` instead.");
      }

      return css;
    },
  };

  Object.defineProperty(result, "stylesheets", { enumerable: false });

  return result;
}

function rootNode(nodes: Node[]): Node {
  if (nodes.length === 1 && nodes[0]) return nodes[0];

  if (nodes.length === 0) return container({});

  return container({
    children: nodes,
    style: {
      display: "block",
      width: percentage(100),
      height: percentage(100),
    },
  });
}
