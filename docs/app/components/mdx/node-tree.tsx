import type { Node } from "takumi-js";

export interface NodeTreeProps {
  node: Node;
  image?: string;
  caption?: string;
}

export function NodeTree({ node, image, caption }: NodeTreeProps) {
  return (
    <div className="my-6 grid gap-4 rounded-xl border border-fd-border bg-fd-card p-4 md:grid-cols-2">
      <pre className="overflow-auto text-xs leading-relaxed">
        <code>{JSON.stringify(node, null, 2)}</code>
      </pre>
      {image ? (
        <figure className="flex flex-col gap-2">
          <img
            src={image}
            alt={caption ?? "Node tree render"}
            className="rounded-lg border border-fd-border"
          />
          {caption ? (
            <figcaption className="text-xs text-fd-muted-foreground">{caption}</figcaption>
          ) : null}
        </figure>
      ) : null}
    </div>
  );
}
