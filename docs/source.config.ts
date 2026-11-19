import { transformerMetaHighlight } from "@shikijs/transformers";
import { rehypeCodeDefaultOptions, remarkMdxMermaid } from "fumadocs-core/mdx-plugins";
import { defineConfig, defineDocs } from "fumadocs-mdx/config";
import lastModified from "fumadocs-mdx/plugins/last-modified";
import { metaSchema, pageSchema } from "fumapress/schema";
import { transformerTwoslash } from "fumadocs-twoslash";
import { createFileSystemTypesCache } from "fumadocs-twoslash/cache-fs";
import type { ShikiTransformer } from "shiki";
import * as z from "zod";

export const docs = defineDocs({
  dir: "content/docs",
  docs: {
    schema: pageSchema.extend({
      index: z.boolean().optional(),
    }),
    postprocess: {
      includeProcessedMarkdown: true,
    },
  },
  meta: {
    schema: metaSchema,
  },
});

export default defineConfig({
  plugins: [lastModified()],
  mdxOptions: {
    remarkPlugins: [remarkMdxMermaid],
    remarkStructureOptions: {
      mdxTypes: (node) =>
        node.name === "td" || node.name === "th" || !node.children || node.children.length === 0,
    },
    rehypeCodeOptions: {
      themes: {
        light: "github-light",
        dark: "github-dark",
      },
      langs: ["ts", "tsx", "js", "svelte", "astro"],
      transformers: [
        ...(rehypeCodeDefaultOptions.transformers ?? []),
        transformerMetaHighlight(),
        transformerTwoslash({
          twoslashOptions: {
            compilerOptions: {
              moduleResolution: "bundler",
              jsx: "preserve",
              baseUrl: undefined,
              types: ["node"],
            },
          },
          typesCache: createFileSystemTypesCache({
            dir: ".waku/twoslash",
            cwd: process.cwd(),
          }),
        }) as ShikiTransformer,
      ],
    },
  },
});
