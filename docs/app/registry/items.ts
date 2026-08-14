/// The registry the shadcn CLI reads. `/r/registry.json` lists these, and
/// `/r/image/<name>.json` serves one, inlining the file it points at.

export interface RegistryItem {
  name: string;
  title: string;
  description: string;
  /** Path under `docs/app/registry`. */
  source: string;
  /** Where the CLI writes the file in the user's project. */
  target: string;
  type: "registry:ui" | "registry:page" | "registry:block";
  fileType: "registry:ui" | "registry:page";
  registryDependencies?: string[];
}

export const REGISTRY_NAME = "takumi";
export const REGISTRY_HOMEPAGE = "https://takumi.kane.tw";

const card = (name: string, title: string, description: string): RegistryItem => ({
  name: `image/${name}`,
  title,
  description,
  source: `image/${name}.tsx`,
  target: `@components/takumi/image/${name}.tsx`,
  type: "registry:ui",
  fileType: "registry:ui",
});

export const registryItems: RegistryItem[] = [
  card(
    "blog-post",
    "Blog Post Card",
    "An open graph card for an article, with a category pill, an oversized title, and an author byline.",
  ),
  card(
    "changelog",
    "Changelog Card",
    "An open graph card for a release, with the version, the date, and a summary of what shipped.",
  ),
  card(
    "docs",
    "Docs Card",
    "An open graph card for a documentation page, tinted by a colour you pass and headed by the site's icon.",
  ),
  card(
    "event",
    "Event Card",
    "An open graph card for a talk or a conference, with the title above a when, where, and host footer.",
  ),
  card(
    "product-card",
    "Product Card",
    "An open graph card for a product, with its image, name, price, and a line of copy.",
  ),
  card(
    "quote",
    "Quote Card",
    "A high-contrast open graph card for a quote, attributed to a person and a company.",
  ),
  card(
    "repository",
    "Repository Card",
    "An open graph card for a repository, with its slug, description, and star, fork, and language counts.",
  ),
  {
    name: "image/og-route",
    title: "Open Graph Route",
    description:
      "A Next.js route that answers /og with a rendered card. Reads the title from the query string and falls back to a default.",
    source: "image/og-route.tsx",
    target: "app/og/route.tsx",
    type: "registry:block",
    fileType: "registry:page",
    registryDependencies: [`${REGISTRY_HOMEPAGE}/r/image/blog-post.json`],
  },
];
