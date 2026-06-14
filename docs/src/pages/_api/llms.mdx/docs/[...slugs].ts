import type { ApiContext } from "waku/router";
import { getLLMText } from "~/lib/get-llm-text";
import { source } from "~/source";

export async function GET(_: Request, { params }: ApiContext<"/llms.mdx/docs/[...slugs]">) {
  const page = source.getPage(params.slugs ?? []);

  if (!page) {
    return new Response(undefined, { status: 404 });
  }

  return new Response(await getLLMText(page), {
    headers: { "Content-Type": "text/markdown" },
  });
}

export async function getConfig() {
  return {
    render: "static" as const,
    staticPaths: source.getPages().map((page) => page.slugs ?? []),
  };
}
