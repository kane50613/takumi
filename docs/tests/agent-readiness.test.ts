import { describe, expect, test } from "bun:test";
import { AGENT_GUIDE } from "../app/agent-guide";
import { NOT_FOUND_MARKDOWN } from "../app/not-found-links";
import { notFoundError, openApiDocument } from "../app/openapi";
import { registryItems } from "../app/registry/items";
import { withAgentRoutes } from "../scripts/patch-vercel-output";

type Operation = {
  operationId: string;
  summary: string;
  description?: string;
  responses: Record<string, unknown>;
};

const operations = Object.entries(openApiDocument.paths).map(
  ([path, item]) => [path, (item as { get: Operation }).get] as const,
);

describe("openapi", () => {
  test("declares the spec version and the production server", () => {
    expect(openApiDocument.openapi).toBe("3.1.0");
    expect(openApiDocument.servers[0]?.url).toBe("https://takumi.kane.tw");
  });

  test("every operation has a unique operationId", () => {
    const ids = operations.map(([, operation]) => operation.operationId);

    expect(ids.every(Boolean)).toBe(true);
    expect(new Set(ids).size).toBe(ids.length);
  });

  test("every operation describes itself and its success response", () => {
    for (const [path, operation] of operations) {
      expect(operation.summary, path).toBeTruthy();
      expect(operation.responses[200], path).toBeDefined();
    }
  });

  test("every $ref resolves", () => {
    const schemas = openApiDocument.components.schemas as Record<string, unknown>;

    for (const ref of JSON.stringify(openApiDocument).matchAll(
      /"#\/components\/schemas\/(\w+)"/g,
    )) {
      expect(schemas).toHaveProperty(ref[1] as string);
    }
  });

  test("the registry item names match the registry", () => {
    const parameter = openApiDocument.paths["/r/image/{name}.json"].get.parameters[0];

    expect(parameter.schema.enum).toEqual(
      registryItems.map((item) => item.name.replace(/^image\//, "")),
    );
  });

  test("is valid JSON", () => {
    expect(() => JSON.parse(JSON.stringify(openApiDocument))).not.toThrow();
  });
});

test("the JSON error carries a code, a message, and a way out", () => {
  expect(notFoundError.error.code).toBe("not_found");
  expect(notFoundError.error.message).toBeTruthy();
  expect(notFoundError.error.resolution).toContain("/openapi.json");
});

test("the 404 markdown points at the machine-readable entry points", () => {
  expect(NOT_FOUND_MARKDOWN).toStartWith("# 404 Not Found");
  for (const target of ["/docs", "/llms.txt", "/sitemap.xml", "/openapi.json"]) {
    expect(NOT_FOUND_MARKDOWN).toContain(`(${target})`);
  }
});

test("the llms.txt preamble says when to use Takumi and how to call it", () => {
  expect(AGENT_GUIDE).toContain("## When to use Takumi");
  expect(AGENT_GUIDE).toContain("## How to call it");
  expect(AGENT_GUIDE).toContain("https://takumi.kane.tw/openapi.json");
});

describe("vercel routes", () => {
  const routes = withAgentRoutes([{ src: "^/assets/(.*)$" }]) as {
    src?: string;
    dest?: string;
    status?: number;
    handle?: string;
    has?: { value: string }[];
    headers?: Record<string, string>;
  }[];

  const find = (predicate: (route: (typeof routes)[number]) => boolean) => {
    const route = routes.find(predicate);

    if (!route) throw new Error("no such route");
    return route;
  };

  test("keeps the routes waku generated, between the rewrites and the error handler", () => {
    const wakuIndex = routes.findIndex((route) => route.src === "^/assets/(.*)$");
    const errorIndex = routes.findIndex((route) => route.handle === "error");

    expect(wakuIndex).toBeGreaterThan(0);
    expect(errorIndex).toBeGreaterThan(wakuIndex);
  });

  test("joins an error phase that already exists instead of declaring a second one", () => {
    const merged = withAgentRoutes([
      { src: "^/assets/(.*)$" },
      { handle: "filesystem" },
      { handle: "error" },
      { src: "^/.*$", status: 404, dest: "/legacy.html" },
    ]) as typeof routes;

    expect(merged.filter((route) => route.handle === "error")).toHaveLength(1);
    expect(merged.at(-1)).toMatchObject({ dest: "/legacy.html" });
    expect(merged.findIndex((route) => route.handle === "filesystem")).toBeLessThan(
      merged.findIndex((route) => route.handle === "error"),
    );
  });

  test("varies on Accept so a cache cannot mix the HTML and Markdown variants", () => {
    expect(find((route) => route.headers?.Vary !== undefined).headers?.Vary).toBe(
      "Accept, Accept-Encoding",
    );
  });

  test("rewrites documentation paths to Markdown when the request asks for it", () => {
    const route = find((route) => route.dest === "/$1.md");
    const pattern = new RegExp(route.src as string);

    expect("/docs".replace(pattern, route.dest as string)).toBe("/docs.md");
    expect("/docs/tables".replace(pattern, route.dest as string)).toBe("/docs/tables.md");
    expect("/docs/pdf/invoices/".replace(pattern, route.dest as string)).toBe(
      "/docs/pdf/invoices.md",
    );
    expect(pattern.test("/docs/tables.md")).toBe(false);
    expect(pattern.test("/playground")).toBe(false);
    expect(new RegExp(route.has?.[0]?.value as string).test("text/markdown")).toBe(true);
    expect(new RegExp(route.has?.[0]?.value as string).test("text/html")).toBe(false);
  });

  test("answers a missing API path with the JSON error", () => {
    const route = find((route) => route.dest === "/errors/not-found.json");

    expect(route.status).toBe(404);
    expect(new RegExp(route.src as string).test("/r/image/nope.json")).toBe(true);
    expect(new RegExp(route.src as string).test("/r")).toBe(true);
    expect(new RegExp(route.src as string).test("/api")).toBe(true);
    expect(new RegExp(route.src as string).test("/docs/nope")).toBe(false);
  });

  test("keeps the HTML 404 as the last resort", () => {
    expect(routes.at(-1)).toMatchObject({ status: 404, dest: "/404.html" });
  });

  test("redirects a retired path straight to its final target", () => {
    const route = find((route) => route.headers?.Location === "/docs/reference");

    expect(route.status).toBe(301);
    expect(new RegExp(route.src as string).test("/docs/deep-dives/stylesheets")).toBe(true);
  });
});
