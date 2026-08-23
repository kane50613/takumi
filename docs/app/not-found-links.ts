export const NOT_FOUND_LINKS = [
  { href: "/docs", label: "Documentation index", note: "every guide, grouped by topic" },
  { href: "/llms.txt", label: "llms.txt", note: "when to use Takumi, plus a linked outline" },
  { href: "/llms-full.txt", label: "llms-full.txt", note: "the whole documentation in one file" },
  { href: "/sitemap.xml", label: "sitemap.xml", note: "every URL this site serves" },
  { href: "/openapi.json", label: "openapi.json", note: "the endpoints, as OpenAPI 3.1" },
  { href: "/", label: "Home", note: "what Takumi is" },
];

export const NOT_FOUND_MARKDOWN = `# 404 Not Found

No page exists at this URL on takumi.kane.tw. It may have been renamed or removed.

## Where to look next

${NOT_FOUND_LINKS.map((link) => `- [${link.label}](${link.href}): ${link.note}`).join("\n")}

Any documentation page is also available as Markdown by appending \`.md\` to its path.
`;
