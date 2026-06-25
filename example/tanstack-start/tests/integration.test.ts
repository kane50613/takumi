import { describe, expect, test } from "vitest";
import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const DIST_SERVER_DIR = join(process.cwd(), "dist", "server");
const STATIC_WASM_IMPORT_PATTERNS = ['from "@takumi-rs/wasm"', 'require("@takumi-rs/wasm")'];

function runBunBuild() {
  execFileSync("bun", ["run", "build"], {
    stdio: "pipe",
    encoding: "utf8",
  });
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

describe("tanstack-start integration", () => {
  test("build emits workerd+wasm artifacts and keeps napi + static wasm imports out", () => {
    runBunBuild();

    expect(existsSync(DIST_SERVER_DIR)).toBe(true);

    const files = listFiles(DIST_SERVER_DIR).filter(
      (file) => file.endsWith(".js") || file.endsWith(".mjs"),
    );
    const allContent = files.map((file) => readFileSync(file, "utf8")).join("\n");

    expect(allContent).toContain("workerd");
    expect(allContent).toContain(".wasm");
    // The `#backend` conditions resolve the worker build to WASM, so the native
    // napi addon must never reach this bundle.
    expect(allContent).not.toContain("@takumi-rs/core");

    for (const content of files.map((file) => readFileSync(file, "utf8"))) {
      for (const pattern of STATIC_WASM_IMPORT_PATTERNS) {
        expect(content.includes(pattern)).toBe(false);
      }
    }
  }, 60_000);
});
