import { describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

// Importing the entry instantiates the module, so a binary the bundler failed to
// carry over surfaces as a load-time ENOENT rather than a later render failure.
const ENTRY = `import { Renderer } from "../../bundlers/bun.mjs";\n\nconsole.log(typeof Renderer);\n`;

describe("bun entry", () => {
  // `bun build` leaves `new URL(specifier, import.meta.url)` untouched, so the entry
  // has to reach the binary through an import the bundler rewrites and emits.
  test("survives bundling with its binary", () => {
    const dir = mkdtempSync(join(import.meta.dir, "..", "node_modules", ".bundler-"));
    const entry = join(dir, "entry.mjs");

    writeFileSync(entry, ENTRY);

    try {
      const built = Bun.spawnSync([
        "bun",
        "build",
        entry,
        "--target=bun",
        `--outdir=${join(dir, "out")}`,
      ]);

      expect(built.exitCode).toBe(0);

      const ran = Bun.spawnSync(["bun", join(dir, "out", "entry.js")]);

      expect(ran.stderr.toString()).toBe("");
      expect(ran.exitCode).toBe(0);
      expect(ran.stdout.toString().trim()).toBe("function");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  }, 60_000);
});
