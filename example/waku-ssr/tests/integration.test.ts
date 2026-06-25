import { describe, expect, test } from "vitest";
import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const DIST_SERVER_DIR = join(process.cwd(), "dist", "server");

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

describe("waku-ssr integration", () => {
  test("build resolves the napi backend for the node target", () => {
    runBunBuild();

    expect(existsSync(DIST_SERVER_DIR)).toBe(true);

    const files = listFiles(DIST_SERVER_DIR).filter(
      (file) => file.endsWith(".js") || file.endsWith(".mjs"),
    );
    const allContent = files.map((file) => readFileSync(file, "utf8")).join("\n");

    // `#backend` resolves the node target to napi, dynamically imported so the
    // native addon stays external and its `.node` binary loads from node_modules.
    expect(allContent).toContain("@takumi-rs/core");
  }, 60_000);
});
