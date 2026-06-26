// Add your showcase projects here!
// If no `title` provided, the hostname will be used as the title (or github owner/repo name).
export const showcaseProjects: Project[] = [
  {
    image: "/images/dcard-post-260376394.webp",
    url: "https://dcard.tw",
    width: 1200,
    height: 630,
  },
  {
    title: "TanStack",
    image: "https://tanstack.com/api/og/query.png",
    url: "https://tanstack.com",
    width: 1200,
    height: 630,
  },
  {
    image: "https://www.fumadocs.dev/og/image.webp",
    url: "https://fumadocs.dev/",
    width: 1200,
    height: 630,
  },
  {
    title: "Wotaku",
    image: "https://wotaku.wiki/__og_image__/og.png",
    url: "https://wotaku.wiki",
    width: 1200,
    height: 630,
  },
  {
    title: "Stepperize",
    image:
      "https://stepperize.com/api/og?title=Stepperize&description=The+type-safe+way+to+build+multi-step+experiences+in+React.&section=Documentation&variant=website",
    url: "https://stepperize.com",
    width: 1200,
    height: 630,
  },
  {
    title: "Swetrix",
    image:
      "https://swetrix.com/api/og-image.png?title=Swetrix&description=Open+source,+privacy-first+web+analytics",
    url: "https://swetrix.com",
    width: 1200,
    height: 630,
  },
  {
    image: "https://raw.githubusercontent.com/pi0/shiki-image/main/test/.snapshot/image.webp",
    url: "https://github.com/pi0/shiki-image",
    width: 1200,
    height: 630,
  },
  {
    title: "Bookhive",
    image: "https://bookhive.buzz/og/marketing",
    url: "https://bookhive.buzz",
    width: 1200,
    height: 630,
  },
  {
    title: "Nakafa",
    image: "https://nakafa.com/og/en/image.png",
    url: "https://nakafa.com",
    width: 1200,
    height: 630,
  },
  {
    image: "https://shotwell.app/og-image.png",
    url: "https://shotwell.app/?utm_source=takumi&utm_medium=showcase&utm_campaign=launch",
    width: 1200,
    height: 630,
  },
  {
    image: "https://prmpt.bio/og/smart-poster.jpg",
    url: "https://prmpt.bio/smart-poster?utm_source=takumi",
    width: 1200,
    height: 630,
  },
  {
    image: "https://www.motion-gpu.dev/docs/og",
    url: "https://www.motion-gpu.dev/",
    width: 1200,
    height: 630,
  },
  {
    image: "https://petit-meme.io/api/og?type=home&locale=en&v=1",
    url: "https://petit-meme.io",
    width: 1200,
    height: 630,
  },
  {
    image: "https://bannx.com/og/default.png",
    url: "https://bannx.com",
    width: 1200,
    height: 630,
  },
  {
    image: "https://res.cloudinary.com/alfanjauhari/image/upload/og/works/gcbc.webp",
    url: "https://www.alfanjauhari.com/",
    width: 1200,
    height: 630,
  },
  {
    url: "https://who-to-bother-at.com",
    image: "https://who-to-bother-at.com/og/vercel",
    width: 1200,
    height: 630,
  },
  {
    image:
      "https://image-bench.kane.tw/render?provider=takumi-webp&template=gradients&width=800&height=400",
    url: "https://image-bench.kane.tw",
    width: 800,
    height: 400,
  },
];

export const showcaseTemplates: Template[] = [
  {
    title: "Blog Post",
    image: "/templates/previews/blog-post-template.svg",
    href: "/docs/templates#blog-post-template",
    color: "from-orange-500/20 to-red-500/20",
  },
  {
    title: "Documentation",
    image: "/templates/previews/docs-template.svg",
    href: "/docs/templates#docs-template",
    color: "from-blue-500/20 to-cyan-500/20",
  },
  {
    title: "Product Card",
    image: "/templates/previews/product-card-template.svg",
    href: "/docs/templates#product-card-template",
    color: "from-green-500/20 to-emerald-500/20",
  },
  {
    title: "Event",
    image: "/templates/previews/event-template.svg",
    href: "/docs/templates#event-template",
    color: "from-amber-500/20 to-red-500/20",
  },
  {
    title: "Quote",
    image: "/templates/previews/quote-template.svg",
    href: "/docs/templates#quote-template",
    color: "from-rose-500/20 to-orange-500/20",
  },
  {
    title: "Repository",
    image: "/templates/previews/repository-template.svg",
    href: "/docs/templates#repository-template",
    color: "from-slate-500/20 to-zinc-500/20",
  },
  {
    title: "Changelog",
    image: "/templates/previews/changelog-template.svg",
    href: "/docs/templates#changelog-template",
    color: "from-emerald-500/20 to-green-500/20",
  },
];

export interface Project {
  title?: string;
  image: string;
  url: string;
  width: number;
  height: number;
}

export interface Template {
  title: string;
  image: string;
  href: string;
  color: string;
}
