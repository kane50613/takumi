import type { ReactNode } from "react";
import { Analytics } from "@vercel/analytics/react";
import { generate as generateOgNode } from "fumadocs-ui/og/takumi";
import { createPress, recommendedPreset, selectTreeRoot, type HeadTag } from "fumapress";
import { lucideIconsPlugin } from "fumadocs-core/source/plugins/lucide-icons";
import { linkValidationPlugin } from "fumapress/plugins/link-validation";
import { oramaSearchPlugin } from "fumapress/plugins/orama-search";
import { Brush, FileText, Image as ImageIcon, Shovel, Sparkles, Terminal } from "lucide-react";
import { googleFonts } from "takumi-js/helpers";
import wasmModule from "takumi-js/wasm";
import { docs } from "./.source/server";
import sticker from "./public/sticker.svg?raw";
import { baseOptions, SITE_NAME, SITE_URL } from "./app/layout-config";
import { SearchDialog } from "./app/components/search/dialog";

const PDF_ACCENT = "#3b82f6";
const PAINT_ACCENT = "#8b5cf6";
const CLI_ACCENT = "#10b981";

const SECTION_CLASSES = [
  ["/docs/pdf", "pdf-section"],
  ["/docs/paint", "paint-section"],
  ["/docs/cli", "cli-section"],
] as const;

function sectionClass(url: string) {
  return SECTION_CLASSES.find(([root]) => url === root || url.startsWith(`${root}/`))?.[1];
}

function tabIcon(icon: ReactNode, color: string) {
  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        color,
      }}
    >
      {icon}
    </div>
  );
}

const SITE_HEAD: HeadTag[] = [
  { tag: "meta", attrs: { name: "twitter:card", content: "summary_large_image" } },
  { tag: "meta", attrs: { name: "twitter:image:width", content: "1200" } },
  { tag: "meta", attrs: { name: "twitter:image:height", content: "630" } },
  { tag: "meta", attrs: { name: "twitter:creator", content: "@kanewang_" } },
  { tag: "meta", attrs: { name: "twitter:site", content: "@kanewang_" } },
  { tag: "meta", attrs: { property: "og:site_name", content: SITE_NAME } },
  { tag: "meta", attrs: { property: "og:type", content: "website" } },
  { tag: "link", attrs: { rel: "icon", type: "image/svg+xml", href: "/logo.svg" } },
  { tag: "link", attrs: { rel: "preconnect", href: "https://fonts.googleapis.com" } },
  {
    tag: "link",
    attrs: { rel: "preconnect", href: "https://fonts.gstatic.com", crossorigin: "anonymous" },
  },
  {
    tag: "link",
    attrs: {
      rel: "stylesheet",
      href: "https://fonts.googleapis.com/css2?family=Noto+Serif:ital,wght@0,400..800;1,400..800&family=Geist+Mono:wght@400&display=swap",
    },
  },
];

export const app = createPress(
  { docs: docs.toFumadocsSource({ baseDir: "docs" }) },
  { loaderPlugins: [lucideIconsPlugin()] },
).plugins(oramaSearchPlugin(), linkValidationPlugin());

export default app.config({
  preset: recommendedPreset({
    search: false,
    ogImage: {
      getImage(page) {
        return {
          node: generateOgNode({
            title: page.data.title,
            description: page.data.description,
            primaryColor: "hsla(354, 90%, 54%, 0.3)",
            primaryTextColor: "#ff3535",
            icon: <img src={sticker} alt="Takumi" style={{ height: "4rem" }} />,
          }),
        };
      },
      options: {
        fonts: googleFonts([{ name: "Noto Serif", weight: [600, 800] }]),
        module: wasmModule,
      },
    },
  }),
  site: {
    name: SITE_NAME,
    baseUrl: SITE_URL,
    git: {
      user: "kane50613",
      repo: "takumi",
      branch: "master",
      rootDir: "docs",
    },
  },
  head({ page }) {
    if (page) {
      return [
        {
          tag: "meta",
          attrs: { property: "og:image:alt", content: `${page.data.title} — ${SITE_NAME}` },
        },
      ];
    }

    return SITE_HEAD;
  },
  ui: {
    layoutProps: {
      nav: baseOptions.nav,
      githubUrl: baseOptions.githubUrl,
    },
    providerProps: {
      theme: { hotKey: false },
      search: { enabled: true, SearchDialog },
    },
    renderRoot({ lang, head, children }) {
      return (
        <html lang={lang ?? "en"} suppressHydrationWarning>
          <head>{head}</head>
          <body className="flex flex-col min-h-screen">
            {children}
            <Analytics />
          </body>
        </html>
      );
    },
    renderPage(props) {
      if (props.type !== "docs") {
        return props.renderDefault();
      }

      const className = sectionClass(props.page.url);

      return props.renderDefault({
        layout: "spacious",
        layoutProps: {
          tree: selectTreeRoot(props.layoutProps.tree, "docs"),
          // Active tab resolves by findLast + prefix match, so the nested
          // roots must come after the whole-docs tab.
          tabs: [
            {
              title: "Image",
              description: "OG images, animations & SVG",
              url: "/docs",
              icon: tabIcon(<ImageIcon size="100%" />, "#e11d48"),
            },
            {
              title: "PDF",
              description: "Paged documents with takumi-pdf",
              url: "/docs/pdf",
              icon: tabIcon(<FileText size="100%" />, PDF_ACCENT),
            },
            {
              title: "Paint",
              description: "Boxes, images, and text runs with takumi-paint",
              url: "/docs/paint",
              icon: tabIcon(<Brush size="100%" />, PAINT_ACCENT),
            },
            {
              title: "CLI",
              description: "Render HTML files from a terminal",
              url: "/docs/cli",
              icon: tabIcon(<Terminal size="100%" />, CLI_ACCENT),
            },
          ],
          links: [
            { icon: <Shovel />, text: "Try in Playground", url: "/playground" },
            { icon: <Sparkles />, text: "Showcase", url: "/showcase" },
          ],
        },
        pageProps: className ? { className } : undefined,
      });
    },
  },
});
