import { afterAll, beforeAll, describe, expect, test } from "vitest";
import { execFileSync, spawn, type ChildProcess } from "node:child_process";

const HOST = "127.0.0.1";
const PORT = 8788;
const MIN_IMAGE_BYTES = 1024;
let workerProcess: ChildProcess | null = null;

function runWranglerBuild() {
  execFileSync("bun", ["run", "build"], {
    stdio: "pipe",
    encoding: "utf8",
  });
}

function startWranglerDev() {
  workerProcess = spawn("bunx", ["wrangler", "dev", "--port", `${PORT}`, "--ip", HOST, "--local"], {
    stdio: "ignore",
  });
}

async function stopWranglerDev() {
  if (!workerProcess) {
    return;
  }
  workerProcess.kill();
  await new Promise<void>((resolve) => {
    workerProcess?.once("exit", () => resolve());
    setTimeout(resolve, 3_000);
  });
  workerProcess = null;
}

async function waitForImageResponse(url: string, expectedContentType: string, timeoutMs: number) {
  const startedAt = Date.now();
  let lastError: unknown;

  while (Date.now() - startedAt < timeoutMs) {
    try {
      const response = await fetch(url, { signal: AbortSignal.timeout(5_000) });
      if (response.status !== 200) {
        lastError = new Error(`Unexpected status: ${response.status}`);
        await new Promise((resolve) => setTimeout(resolve, 250));
        continue;
      }
      const contentType = response.headers.get("content-type") ?? "";
      if (!contentType.includes(expectedContentType)) {
        lastError = new Error(`Unexpected content-type: ${contentType}`);
        await new Promise((resolve) => setTimeout(resolve, 250));
        continue;
      }
      const bytes = new Uint8Array(await response.arrayBuffer());
      if (bytes.byteLength <= MIN_IMAGE_BYTES) {
        lastError = new Error(`Response too small: ${bytes.byteLength} bytes`);
        await new Promise((resolve) => setTimeout(resolve, 250));
        continue;
      }
      return;
    } catch (error) {
      lastError = error;
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
  }

  throw new Error(`Failed to get valid image response from ${url}: ${String(lastError)}`);
}

describe("cloudflare-workers integration", () => {
  beforeAll(() => {
    runWranglerBuild();
    startWranglerDev();
  }, 60_000);

  afterAll(async () => {
    await stopWranglerDev();
  });

  test("worker serves png image output", async () => {
    await waitForImageResponse(`http://${HOST}:${PORT}/?name=CI`, "image/png", 45_000);
    expect(true).toBe(true);
  }, 60_000);
});
