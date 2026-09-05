import type { ReactNode } from "react";
import { Analytics } from "@vercel/analytics/react";
import { getPageTreePeers } from "fumadocs-core/page-tree";
import { lucideIconsPlugin } from "fumadocs-core/source/plugins/lucide-icons";
import { Card, Cards } from "fumadocs-ui/components/card";
import { Step, Steps } from "fumadocs-ui/components/steps";
import { Tab, Tabs } from "fumadocs-ui/components/tabs";
import defaultMdxComponents, { createRelativeLink } from "fumadocs-ui/mdx";
import { generate as generateOgNode } from "fumadocs-ui/og/takumi";
import * as Twoslash from "fumadocs-twoslash/ui";
import { defineConfig } from "fumapress";
import { fumadocsMdx } from "fumapress/adapters/mdx";
import { createDocsLayoutPage } from "fumapress/layouts/docs";
import { createRootLayout } from "fumapress/layouts/root";
import { oramaSearchPlugin } from "fumapress/plugins/orama-search";
import { linkValidationPlugin } from "fumapress/plugins/link-validation";
import { takumiPlugin } from "fumapress/plugins/takumi";
import {
  ArrowBigRight,
  BookOpen,
  FileCode2,
  FileText,
  Hand,
  Image as ImageIcon,
  Palette,
  Shovel,
  Sparkles,
  ToyBrick,
  Type,
  Wrench,
  Zap,
} from "lucide-react";
import { googleFonts } from "takumi-js/helpers";
import wasmModule from "takumi-js/wasm";
import { docs } from "./.source/server";
import sticker from "./public/sticker.svg?raw";
import { baseOptions, SITE_URL } from "./app/layout-config";
import { Accordion, Accordions } from "./app/components/accordion";
import { Mermaid } from "./app/components/mdx/mermaid";
import { TypeTable } from "./app/components/type-table";
import { Video } from "./app/components/video";

const PDF_ACCENT = "#3b82f6";

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

const rootLayout = createRootLayout({
  providerProps: {
    theme: { hotKey: false },
  },
});

export default defineConfig({
  mode: "static",
  renderRoot: rootLayout,
  content: {
    docs: docs.toFumadocsSource({ baseDir: "docs" }),
  },
  loaderOptions: {
    plugins: [lucideIconsPlugin()],
  },
  site: {
    name: "Takumi",
    baseUrl: "https://takumi.kane.tw",
    git: {
      user: "kane50613",
      repo: "takumi",
      branch: "master",
      rootDir: "docs",
    },
  },
  meta: {
    root() {
      return (
        <>
          <meta name="twitter:card" content="summary_large_image" />
          <meta name="twitter:image:width" content="1200" />
          <meta name="twitter:image:height" content="630" />
          <meta name="twitter:creator" content="@kanewang_" />
          <meta name="twitter:site" content="@kanewang_" />
          <meta property="og:site_name" content="Takumi" />
          <meta property="og:type" content="website" />
          <link rel="icon" type="image/svg+xml" href="/logo.svg" />
          <link rel="preconnect" href="https://fonts.googleapis.com" />
          <link rel="preconnect" href="https://fonts.gstatic.com" crossOrigin="anonymous" />
          <link
            rel="stylesheet"
            href="https://fonts.googleapis.com/css2?family=Noto+Serif:ital,wght@0,400..800;1,400..800&family=Geist+Mono:wght@400&display=swap"
          />
          <Analytics />
        </>
      );
    },
    page(page) {
      const url = `${SITE_URL}${page.url}`;

      return (
        <>
          {page.data.description && <meta name="description" content={page.data.description} />}
          <meta property="og:url" content={url} />
          <meta property="og:image:alt" content={`${page.data.title} — Takumi`} />
          <link rel="canonical" href={url} />
        </>
      );
    },
  },
})
  .adapters(
    fumadocsMdx({
      async getMdxComponents(page) {
        const source = await this.getLoader();

        function DocsCategory() {
          return (
            <Cards>
              {getPageTreePeers(source.getPageTree(page.locale), page.url).map((peer) => (
                <Card key={peer.url} title={peer.name} href={peer.url}>
                  {peer.description}
                </Card>
              ))}
            </Cards>
          );
        }

        return {
          ...defaultMdxComponents,
          ...Twoslash,
          a: createRelativeLink(source, page),
          Hand,
          BookOpen,
          FileCode2,
          Wrench,
          Shovel,
          ToyBrick,
          ArrowBigRight,
          Zap,
          Palette,
          Type,
          Mermaid,
          Step,
          Steps,
          Card,
          Cards,
          Tab,
          Tabs,
          Accordion,
          Accordions,
          TypeTable,
          Video,
          DocsCategory,
        } satisfies Record<string, unknown>;
      },
    }),
  )
  .plugins(
    oramaSearchPlugin(),
    linkValidationPlugin(),
    takumiPlugin({
      generate(page) {
        return {
          node: generateOgNode({
            title: page.data.title,
            description: page.data.description,
            primaryColor: "hsla(354, 90%, 54%, 0.3)",
            primaryTextColor: "#ff3535",
            icon: <img src={sticker} alt="Takumi" style={{ height: "4rem" }} />,
          }),
          options: {
            fonts: googleFonts([{ name: "Noto Serif", weight: [600, 800] }]),
            module: wasmModule,
          },
        };
      },
    }),
  )
  .layouts({
    defaultProps() {
      return {
        nav: baseOptions.nav,
        githubUrl: baseOptions.githubUrl,
      };
    },
    page: createDocsLayoutPage({
      async render(page) {
        const source = await this.getLoader();
        let tree = source.getPageTree(this.lang);

        for (const child of tree.children) {
          if (child.type === "folder" && child.$id === "docs") {
            tree = { ...tree, children: child.children };
          }
        }
        const inPdfSection = page.url === "/docs/pdf" || page.url.startsWith("/docs/pdf/");

        return {
          layoutProps: {
            tree,
            containerProps: inPdfSection ? { className: "pdf-section" } : undefined,
            sidebar: {
              // Active tab resolves by findLast + prefix match, so the nested
              // PDF root must come after the whole-docs tab.
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
              ],
            },
            links: [
              { icon: <Shovel />, text: "Try in Playground", url: "/playground" },
              { icon: <Sparkles />, text: "Showcase", url: "/showcase" },
            ],
          },
        };
      },
    }),
  });
