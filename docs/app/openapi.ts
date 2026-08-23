import { SITE_URL } from "~/layout-config";
import { registryItems } from "~/registry/items";

const ERROR_SCHEMA = {
  type: "object",
  required: ["error"],
  properties: {
    error: {
      type: "object",
      required: ["code", "message"],
      properties: {
        code: { type: "string", description: "Stable machine-readable error code." },
        message: { type: "string", description: "Human-readable description of the failure." },
        resolution: { type: "string", description: "What to try instead." },
        documentation: {
          type: "string",
          format: "uri",
          description: "Where the fix is documented.",
        },
      },
    },
  },
} as const;

const notFound = {
  description: "The resource does not exist. The body is a JSON error object.",
  content: { "application/json": { schema: { $ref: "#/components/schemas/Error" } } },
};

const registryFile = {
  type: "object",
  required: ["path", "content", "type", "target"],
  properties: {
    path: { type: "string", description: "Path of the file inside the registry." },
    content: { type: "string", description: "Full source of the component." },
    type: { type: "string", enum: ["registry:ui", "registry:page"] },
    target: { type: "string", description: "Where the shadcn CLI writes the file." },
  },
} as const;

const registryItem = {
  type: "object",
  required: ["name", "type", "title", "description", "files"],
  properties: {
    $schema: { type: "string", format: "uri" },
    name: { type: "string", description: "Registry item name, e.g. `image/quote`." },
    type: { type: "string", enum: ["registry:ui", "registry:page", "registry:block"] },
    title: { type: "string" },
    description: { type: "string" },
    dependencies: { type: "array", items: { type: "string" } },
    registryDependencies: { type: "array", items: { type: "string" } },
    files: { type: "array", items: { $ref: "#/components/schemas/RegistryFile" } },
  },
} as const;

const itemNames = registryItems.map((item) => item.name.replace(/^image\//, ""));

const docsSlugs = {
  type: "string",
  description: "Documentation path without the leading `/docs/`, e.g. `tables` or `pdf/invoices`.",
} as const;

export const openApiDocument = {
  openapi: "3.1.0",
  info: {
    title: "Takumi documentation API",
    version: "1.0.0",
    summary:
      "Read-only endpoints that expose the Takumi docs, component registry, and search index.",
    description: [
      "Takumi renders JSX, HTML, and node trees into images, SVG, and PDF from Rust, without a headless browser.",
      "",
      "Every endpoint here is a public, unauthenticated `GET`. There is no rate limit and no API key.",
      "Agents should read `/llms.txt` first for a map of the documentation, then fetch a page as Markdown",
      "from `/docs/{slug}.md`. `/r/registry.json` lists installable Takumi templates for the shadcn CLI.",
    ].join("\n"),
    contact: { name: "Kane Wang", email: "me@kane.tw", url: "https://github.com/kane50613/takumi" },
    license: { name: "MIT OR Apache-2.0", identifier: "MIT OR Apache-2.0" },
  },
  servers: [{ url: SITE_URL, description: "Production" }],
  security: [],
  externalDocs: { description: "Takumi documentation", url: `${SITE_URL}/docs` },
  tags: [
    { name: "Documentation", description: "Machine-readable copies of the documentation." },
    { name: "Registry", description: "Templates the shadcn CLI can install." },
    { name: "Discovery", description: "Files that describe the site itself." },
  ],
  paths: {
    "/llms.txt": {
      get: {
        operationId: "getLlmsIndex",
        tags: ["Documentation"],
        summary: "Documentation outline for LLMs",
        description:
          "An llms.txt index: when to reach for Takumi, plus a linked outline of every documentation page.",
        responses: {
          200: {
            description: "The outline in Markdown.",
            content: { "text/plain": { schema: { type: "string" } } },
          },
          404: notFound,
        },
      },
    },
    "/llms-full.txt": {
      get: {
        operationId: "getLlmsFullText",
        tags: ["Documentation"],
        summary: "Full documentation as one file",
        description:
          "Every documentation page concatenated as Markdown, for loading into a context window.",
        responses: {
          200: {
            description: "The full documentation in Markdown.",
            content: { "text/plain": { schema: { type: "string" } } },
          },
          404: notFound,
        },
      },
    },
    "/docs/{slug}.md": {
      get: {
        operationId: "getDocumentationPage",
        tags: ["Documentation"],
        summary: "One documentation page as Markdown",
        description:
          "The Markdown source of a page. The same content is served from the HTML URL when the request sends `Accept: text/markdown`.",
        parameters: [{ name: "slug", in: "path", required: true, schema: docsSlugs }],
        responses: {
          200: {
            description: "The page in Markdown.",
            content: { "text/markdown": { schema: { type: "string" } } },
          },
          404: notFound,
        },
      },
    },
    "/r/registry.json": {
      get: {
        operationId: "getRegistryIndex",
        tags: ["Registry"],
        summary: "List every registry item",
        description:
          "The shadcn-compatible registry index, with the source of each template inlined.",
        responses: {
          200: {
            description: "The registry index.",
            content: {
              "application/json": { schema: { $ref: "#/components/schemas/Registry" } },
            },
          },
          404: notFound,
        },
      },
    },
    "/r/image/{name}.json": {
      get: {
        operationId: "getRegistryItem",
        tags: ["Registry"],
        summary: "Fetch one registry item",
        description: "One template, in the shape `shadcn add` expects.",
        parameters: [
          {
            name: "name",
            in: "path",
            required: true,
            schema: { type: "string", enum: itemNames },
          },
        ],
        responses: {
          200: {
            description: "The registry item.",
            content: {
              "application/json": { schema: { $ref: "#/components/schemas/RegistryItem" } },
            },
          },
          404: notFound,
        },
      },
    },
    "/api/search": {
      get: {
        operationId: "getSearchIndex",
        tags: ["Discovery"],
        summary: "Download the search index",
        description:
          "The serialized Orama index the site searches in the browser. Restore it with `@orama/orama`'s `load()` to query the docs offline.",
        responses: {
          200: {
            description: "The serialized Orama index.",
            content: { "application/json": { schema: { type: "object" } } },
          },
          404: notFound,
        },
      },
    },
    "/sitemap.xml": {
      get: {
        operationId: "getSitemap",
        tags: ["Discovery"],
        summary: "List every page URL",
        responses: {
          200: {
            description: "A sitemap in XML.",
            content: { "application/xml": { schema: { type: "string" } } },
          },
          404: notFound,
        },
      },
    },
    "/openapi.json": {
      get: {
        operationId: "getOpenApiDocument",
        tags: ["Discovery"],
        summary: "Fetch this specification",
        responses: {
          200: {
            description: "This document.",
            content: { "application/json": { schema: { type: "object" } } },
          },
          404: notFound,
        },
      },
    },
  },
  components: {
    schemas: {
      Error: ERROR_SCHEMA,
      RegistryFile: registryFile,
      RegistryItem: registryItem,
      Registry: {
        type: "object",
        required: ["name", "homepage", "items"],
        properties: {
          $schema: { type: "string", format: "uri" },
          name: { type: "string" },
          homepage: { type: "string", format: "uri" },
          items: { type: "array", items: { $ref: "#/components/schemas/RegistryItem" } },
        },
      },
    },
  },
};

export const notFoundError = {
  error: {
    code: "not_found",
    message: "No such resource on takumi.kane.tw.",
    resolution:
      "Check /openapi.json for the endpoints this site serves, or /llms.txt for the documentation outline.",
    documentation: `${SITE_URL}/openapi.json`,
  },
};
