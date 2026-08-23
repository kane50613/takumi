/// The routes from `patch-vercel-output.ts` only exist in a deployment, so they are checked
/// against one rather than against a local build.
const base = process.argv[2]?.replace(/\/$/, "");

if (!base) throw new Error("usage: bun scripts/verify-agent-endpoints.ts <deployment-url>");

const MARKDOWN = { Accept: "text/markdown" };

const failures: string[] = [];

async function check(name: string, run: () => Promise<void>) {
  try {
    await run();
    console.log(`ok   ${name}`);
  } catch (error) {
    failures.push(`${name}: ${error instanceof Error ? error.message : error}`);
    console.log(`FAIL ${name}: ${error}`);
  }
}

function expect(actual: unknown, expected: unknown, what: string) {
  if (actual !== expected) throw new Error(`${what} is ${actual}, expected ${expected}`);
}

function expectContains(actual: string | null, needle: string, what: string) {
  if (actual?.toLowerCase().includes(needle.toLowerCase())) return;

  const shown = actual === null ? "missing" : JSON.stringify(actual.slice(0, 120));

  throw new Error(`${what} is ${shown}, expected it to contain ${needle}`);
}

await check("a documentation page serves Markdown to a client that asks for it", async () => {
  const response = await fetch(`${base}/docs/tables`, { headers: MARKDOWN });

  expect(response.status, 200, "status");
  expectContains(response.headers.get("content-type"), "text/markdown", "content-type");
  expectContains(response.headers.get("vary"), "accept", "vary");
  expectContains(await response.text(), "# ", "body");
});

await check("the same page still serves HTML to a browser", async () => {
  const response = await fetch(`${base}/docs/tables`);

  expect(response.status, 200, "status");
  expectContains(response.headers.get("content-type"), "text/html", "content-type");
});

await check("a missing registry item answers with the JSON error", async () => {
  const response = await fetch(`${base}/r/image/nope.json`);

  expect(response.status, 404, "status");
  expectContains(response.headers.get("content-type"), "application/json", "content-type");
  expect((await response.json()).error?.code, "not_found", "error code");
});

await check("a retired documentation URL redirects straight to its target", async () => {
  const response = await fetch(`${base}/docs/quickstart`, { redirect: "manual" });

  expect(response.status, 301, "status");
  expect(response.headers.get("location"), "/docs", "location");
});

await check("the OpenAPI document is served", async () => {
  const response = await fetch(`${base}/openapi.json`);

  expect(response.status, 200, "status");
  expect((await response.json()).openapi, "3.1.0", "openapi version");
});

await check("llms.txt opens with the agent guide", async () => {
  const response = await fetch(`${base}/llms.txt`);

  expect(response.status, 200, "status");
  expectContains(await response.text(), "## When to use Takumi", "body");
});

await check("an unknown path answers 404 in Markdown when asked", async () => {
  const response = await fetch(`${base}/no-such-path-here`, { headers: MARKDOWN });

  expect(response.status, 404, "status");
  expectContains(response.headers.get("content-type"), "text/markdown", "content-type");
  expectContains(await response.text(), "# 404 Not Found", "body");
});

await check("an unknown path answers 404 in HTML otherwise", async () => {
  const response = await fetch(`${base}/no-such-path-here`);

  expect(response.status, 404, "status");
  expectContains(response.headers.get("content-type"), "text/html", "content-type");
});

if (failures.length) {
  console.error(`\n${failures.length} check(s) failed on ${base}:`);
  for (const failure of failures) console.error(`  ${failure}`);
  process.exit(1);
}

console.log(`\nall checks passed on ${base}`);

export {};
