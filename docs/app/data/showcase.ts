import type { LucideIcon } from "lucide-react";
import { LayoutTemplate, Type, Zap } from "lucide-react";

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
    image: "https://www.fumadocs.dev/og/image.webp",
    url: "https://fumadocs.dev/",
    width: 1200,
    height: 630,
  },
  {
    image:
      "https://raw.githubusercontent.com/pi0/shiki-image/main/test/.snapshot/image.webp",
    url: "https://github.com/pi0/shiki-image",
    width: 1200,
    height: 630,
  },
  {
    image:
      "https://res.cloudinary.com/alfanjauhari/image/upload/og/works/gcbc.webp",
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
  {
    image: "https://shotwell.app/og-image.png",
    url: "https://shotwell.app/?utm_source=takumi&utm_medium=showcase&utm_campaign=launch",
    width: 1200,
    height: 630,
  },
];

export const showcaseTemplates: Template[] = [
  {
    title: "Blog Post",
    image: "/templates/previews/blog-post-template.webp",
    href: "/docs/templates#blog-post-template",
    color: "from-orange-500/20 to-red-500/20",
  },
  {
    title: "Documentation",
    image: "/templates/previews/docs-template.webp",
    href: "/docs/templates#docs-template",
    color: "from-blue-500/20 to-cyan-500/20",
  },
  {
    title: "Product Card",
    image: "/templates/previews/product-card-template.webp",
    href: "/docs/templates#product-card-template",
    color: "from-green-500/20 to-emerald-500/20",
  },
];

export const showcaseFeatures: Feature[] = [
  {
    title: "Advanced Typography",
    description: "Variable fonts, RTL, complex text decorations, and more.",
    icon: Type,
  },
  {
    title: "Satori Compatibility",
    description: "Supports nearly all Satori features.",
    icon: LayoutTemplate,
  },
  {
    title: "Tailwind CSS First",
    description: "Native Tailwind parser built-in for maximum performance.",
    icon: Zap,
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

export interface Feature {
  title: string;
  description: string;
  icon: LucideIcon;
}
