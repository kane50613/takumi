import { createSearchAPI } from "fumadocs-core/search/server";
import { source } from "~/source";

const server = createSearchAPI("advanced", {
  indexes: source.getPages().map((page) => ({
    id: page.url,
    url: page.url,
    title: page.data.title ?? "",
    description: page.data.description,
    structuredData: page.data.structuredData,
  })),
});

export function GET() {
  return server.staticGET();
}

export async function getConfig() {
  return {
    render: "static" as const,
  };
}
