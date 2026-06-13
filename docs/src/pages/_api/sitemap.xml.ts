import { source } from "~/source";

export function GET() {
  const pages = [
    page({
      path: "/",
      change: "monthly",
      priority: 1,
    }),
    page({
      path: "/playground/",
      change: "monthly",
      priority: 0.8,
    }),
    page({
      path: "/showcase/",
      change: "monthly",
      priority: 0.8,
    }),
  ];

  pages.push(
    ...source.getPages().map(({ url, data }) =>
      page({
        path: `${url}/`,
        priority: 0.5,
        change: "daily",
        lastModified: data.lastModified,
      }),
    ),
  );

  return new Response(
    `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  ${pages.join("\n")}
</urlset>`,
    {
      headers: {
        "content-type": "application/xml",
      },
    },
  );
}

function page({
  path,
  change,
  priority,
  lastModified,
}: {
  path: string;
  change: "monthly" | "weekly" | "daily";
  priority: number;
  lastModified?: Date;
}) {
  let content = `<url>
  <loc>https://takumi.kane.tw${path}</loc>
  <changefreq>${change}</changefreq>
  <priority>${priority}</priority>
`;

  if (lastModified) {
    const date = lastModified instanceof Date ? lastModified : new Date(lastModified);
    content += `<lastmod>${date.toISOString()}</lastmod>`;
  }

  content += "</url>";

  return content;
}

export async function getConfig() {
  return {
    render: "static" as const,
  };
}
