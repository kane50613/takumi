import { container, percentage } from "../helpers";
import { fromStaticMarkup } from "./markup";

export function fromHtml(html: string) {
  const { nodes, stylesheets } = fromStaticMarkup(html);

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
