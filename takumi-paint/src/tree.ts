import type { PaintNode, PaintTextRun, PaintTree } from "../pkg/takumi_paint_wasm";

/** Yields every node in paint order, starting at the root. */
export function* walk(tree: PaintTree | PaintNode): Generator<PaintNode> {
  const node = "root" in tree ? tree.root : tree;
  yield node;
  for (const child of node.children ?? []) yield* walk(child);
}

/** Every text run in paint order, each paired with the node that paints it. */
export function* textRuns(
  tree: PaintTree | PaintNode,
): Generator<{ run: PaintTextRun; node: PaintNode }> {
  for (const node of walk(tree)) {
    for (const run of node.textRuns ?? []) yield { run, node };
  }
}

/** The first node whose source `id` matches. */
export function find(tree: PaintTree | PaintNode, id: string): PaintNode | undefined {
  for (const node of walk(tree)) {
    if (node.source?.id === id) return node;
  }
  return undefined;
}
