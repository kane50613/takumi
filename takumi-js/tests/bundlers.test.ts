import { describe, expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const NODE_CORE_FAILURE_MESSAGE =
  "Failed to load @takumi-rs/core in Node.js runtime. Takumi requires the native napi-rs module in Node environments.";
const STATIC_WASM_IMPORT_PATTERNS = ['from "@takumi-rs/wasm"', 'require("@takumi-rs/wasm")'];
const TAKUMI_JS_ROOT = join(import.meta.dir, "..");

function runBunBuild(cwd: string) {
  const result = Bun.spawnSync(["bun", "run", "build"], {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });

  if (result.exitCode !== 0) {
    const stderr = new TextDecoder().decode(result.stderr);
    const stdout = new TextDecoder().decode(result.stdout);
    throw new Error(`Build failed in ${cwd}\nstdout:\n${stdout}\nstderr:\n${stderr}`);
  }
}

function listFiles(dir: string): string[] {
  const entries = readdirSync(dir, { withFileTypes: true });

  return entries.flatMap((entry) => {
    const entryPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      return listFiles(entryPath);
    }
    return [entryPath];
  });
}

function readJsLikeFiles(dir: string) {
  return listFiles(dir)
    .filter((file) => file.endsWith(".js") || file.endsWith(".mjs") || file.endsWith(".cjs"))
    .map((file) => ({
      file,
      content: readFileSync(file, "utf8"),
    }));
}

function assertNoStaticWasmImports(files: { file: string; content: string }[]) {
  for (const { file, content } of files) {
    for (const pattern of STATIC_WASM_IMPORT_PATTERNS) {
      expect(content.includes(pattern), `unexpected static wasm import in ${file}`).toBeFalse();
    }
  }
}

describe("bundle regression", () => {
  test("render chunk keeps node guard and no static wasm import", () => {
    runBunBuild(TAKUMI_JS_ROOT);

    const distDir = join(TAKUMI_JS_ROOT, "dist");
    const files = readJsLikeFiles(distDir).filter((file) => file.file.includes("/render-"));
    const allContent = files.map((file) => file.content).join("\n");

    expect(files.length).toBeGreaterThan(0);
    expect(allContent).toContain(NODE_CORE_FAILURE_MESSAGE);
    expect(allContent).toContain("@takumi-rs/wasm/next");
    assertNoStaticWasmImports(files);
  });
});
