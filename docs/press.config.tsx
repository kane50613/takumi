import type { ReactNode } from "react";
import { Analytics } from "@vercel/analytics/react";
import { getPageTreePeers } from "fumadocs-core/page-tree";
import { lucideIconsPlugin } from "fumadocs-core/source/plugins/lucide-icons";
import { Card, Cards } from "fumadocs-ui/components/card";
import { Step, Steps } from "fumadocs-ui/components/steps";
import { Tab, Tabs } from "fumadocs-ui/components/tabs";
import defaultMdxComponents, { createRelativeLink } from "fumadocs-ui/mdx";
import * as Twoslash from "fumadocs-twoslash/ui";
import { defineConfig } from "fumapress";
import { fumadocsMdx } from "fumapress/adapters/mdx";
import { createDocsLayoutPage } from "fumapress/layouts/docs";
import { flexsearchPlugin } from "fumapress/plugins/flexsearch";
import { llmsPlugin } from "fumapress/plugins/llms.txt";
import { sitemapPlugin } from "fumapress/plugins/sitemap";
import { takumiPlugin } from "fumapress/plugins/takumi";
import {
  ArrowBigRight,
  BookOpen,
  FileCode2,
  Hand,
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
import logo from "./public/logo.svg?raw";
import { baseOptions } from "./app/layout-config";
import { Accordion, Accordions } from "./app/components/accordion";
import { Mermaid } from "./app/components/mdx/mermaid";
import { TypeTable } from "./app/components/type-table";
import { Video } from "./app/components/video";

function OgCard({ title, description }: { title: string; description?: string }) {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: "100%",
        height: "100%",
        color: "white",
        padding: "4rem",
        backgroundColor: "#16130f",
        borderBottom: "18px solid rgba(255,53,53,0.3)",
        fontFamily: "Noto Serif",
      }}
    >
      <p style={{ fontWeight: 800, fontSize: "82px", margin: 0 }}>{title}</p>
      {description && (
        <p
          style={{
            fontSize: "48px",
            color: "rgba(240,240,240,0.8)",
            margin: 0,
            marginTop: "16px",
            paddingBottom: "28px",
            borderBottom: "10px dashed rgba(255,53,53,0.3)",
          }}
        >
          {description}
        </p>
      )}
      <div
        style={{
          display: "flex",
          flexDirection: "row",
          alignItems: "center",
          gap: "20px",
          marginTop: "auto",
          color: "#ff3535",
        }}
      >
        <img src="takumi.svg" alt="Takumi" style={{ width: "64px", height: "64px" }} />
        <p style={{ fontSize: "56px", fontWeight: 600, margin: 0 }}>Takumi</p>
      </div>
    </div>
  );
}

export default defineConfig({
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
          <meta charSet="utf-8" />
          <meta name="viewport" content="width=device-width, initial-scale=1" />
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
    flexsearchPlugin(),
    llmsPlugin(),
    sitemapPlugin(),
    takumiPlugin({
      generate(page): { node: ReactNode; options: Record<string, unknown> } {
        return {
          node: <OgCard title={page.data.title} description={page.data.description} />,
          options: {
            fonts: googleFonts([
              {
                name: "Noto Serif",
                weight: [600, 800],
              },
            ]),
            images: [{ src: "takumi.svg", data: Buffer.from(logo) }],
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
      async render() {
        const source = await this.getLoader();
        let tree = source.getPageTree(this.lang);

        for (const child of tree.children) {
          if (child.type === "folder" && child.$id === "docs") {
            tree = { ...tree, children: child.children };
          }
        }

        return {
          layoutProps: {
            tree,
            links: [
              { icon: <Shovel />, text: "Try in Playground", url: "/playground" },
              { icon: <Sparkles />, text: "Showcase", url: "/showcase" },
            ],
          },
        };
      },
    }),
  });
