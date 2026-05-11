import {
  defaultStringifier,
  rehypeCodeDefaultOptions,
  remarkMdxMermaid,
} from "fumadocs-core/mdx-plugins";
import { defineConfig, defineDocs } from "fumadocs-mdx/config";
import lastModified from "fumadocs-mdx/plugins/last-modified";
import { metaSchema, pageSchema } from "fumadocs-core/source/schema";
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

const structureStringifier = defaultStringifier({
  handlers: {
    table: (node, _, state, info) => state.containerFlow(node, info),
    tableRow: (node, _, state, info) => state.containerFlow(node, info),
    tableCell: (node, _, state, info) => state.containerPhrasing(node, info),
  },
});

export default defineConfig({
  plugins: [lastModified()],
  mdxOptions: {
    remarkPlugins: [remarkMdxMermaid],
    remarkStructureOptions: {
      mdxTypes: (node) =>
        node.name === "td" || node.name === "th" || !node.children || node.children.length === 0,
      stringify(node, ctx) {
        return structureStringifier.call(
          this,
          node.type === "mdxJsxFlowElement" && (node.name === "td" || node.name === "th")
            ? ({ ...node, type: "mdxJsxTextElement" } as Parameters<typeof structureStringifier>[0])
            : node,
          ctx,
        );
      },
    },
    rehypeCodeOptions: {
      themes: {
        light: "github-light",
        dark: "github-dark",
      },
      langs: ["ts", "tsx", "js", "svelte"],
      transformers: [
        ...(rehypeCodeDefaultOptions.transformers ?? []),
        transformerTwoslash({
          twoslashOptions: {
            compilerOptions: {
              moduleResolution: 100, // ts.ModuleResolutionKind.Bundler
              jsx: 1, // ts.JsxEmit.Preserve
              baseUrl: undefined,
            },
          },
          typesCache: createFileSystemTypesCache({
            dir: ".react-router/twoslash",
            cwd: process.cwd(),
          }),
        }) as ShikiTransformer,
      ],
    },
  },
});
