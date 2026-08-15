import { REGISTRY_HOMEPAGE, REGISTRY_NAME, registryItems } from "./items";
import type { RegistryItem } from "./items";

const ITEM_SCHEMA = "https://ui.shadcn.com/schema/registry-item.json";
const REGISTRY_SCHEMA = "https://ui.shadcn.com/schema/registry.json";

// The CLI serves each item's source inline, so it is read at build time rather
// than from disk: the bundled route has no source tree to read from.
const sources = import.meta.glob<string>("./image/*.tsx", {
  query: "?raw",
  import: "default",
  eager: true,
});

export const buildItem = (item: RegistryItem) => ({
  $schema: ITEM_SCHEMA,
  name: item.name,
  type: item.type,
  title: item.title,
  description: item.description,
  dependencies: ["takumi-js"],
  ...(item.registryDependencies && {
    registryDependencies: item.registryDependencies,
  }),
  files: [
    {
      path: item.source,
      content: sources[`./${item.source}`],
      type: item.fileType,
      target: item.target,
    },
  ],
});

export const buildRegistry = () => ({
  $schema: REGISTRY_SCHEMA,
  name: REGISTRY_NAME,
  homepage: REGISTRY_HOMEPAGE,
  items: registryItems.map(buildItem),
});

export const findItem = (name: string) =>
  registryItems.find((item) => item.name === `image/${name}`);
