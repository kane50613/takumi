import { buildItem, findItem } from "./build";

/// One registry item, served at its own URL because the shadcn CLI fetches a
/// single item per install.
export const itemRoute = (name: string) => ({
  GET: () => {
    const item = findItem(name);

    return item ? Response.json(buildItem(item)) : new Response("Not found", { status: 404 });
  },
  getConfig: async () => ({ render: "static" }) as const,
});
