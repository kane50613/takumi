import { source } from "~/source";

export function GET() {
  const scanned = ["# Takumi Docs"];
  const groups = new Map<string, string[]>();

  for (const page of source.getPages()) {
    const dir = page.slugs?.[0] ?? "docs";
    const list = groups.get(dir) ?? [];
    const title = page.data.title ?? page.url;
    const description = page.data.description ?? "";

    list.push(`- [${title}](${page.url}): ${description}`);
    groups.set(dir, list);
  }

  for (const [key, value] of groups) {
    scanned.push(`## ${key}`);
    scanned.push(value.join("\n"));
  }

  return new Response(scanned.join("\n\n"));
}

export async function getConfig() {
  return {
    render: "static" as const,
  };
}
