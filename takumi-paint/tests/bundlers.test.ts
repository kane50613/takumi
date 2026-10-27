import { expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

// Nitro's `bun` preset sets both `bun` and `unwasm`. Under unwasm a `.wasm` import is a
// `WebAssembly.Module`, which the bun entry would mistake for a file path.
const cases = [
  { name: "bun with unwasm", conditions: ["bun", "unwasm"], loader: "?module", other: "Bun.file" },
  { name: "bun alone", conditions: ["bun"], loader: "Bun.file", other: "?module" },
];

test.each(cases)(
  "$name picks the matching loader",
  ({ conditions, loader, other }) => {
    const dir = mkdtempSync(join(import.meta.dir, "..", "node_modules", ".conditions-"));
    const entry = join(dir, "entry.mjs");

    writeFileSync(entry, `import { default as wasm } from "takumi-paint";\n\nconsole.log(wasm);\n`);

    try {
      const built = Bun.spawnSync([
        "bun",
        "build",
        entry,
        "--target=node",
        "--external=*.wasm",
        "--external=*.wasm?module",
        ...conditions.map((condition) => `--conditions=${condition}`),
      ]);

      expect(built.exitCode).toBe(0);
      expect(built.stdout.toString()).toContain(loader);
      expect(built.stdout.toString()).not.toContain(other);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  },
  60_000,
);
