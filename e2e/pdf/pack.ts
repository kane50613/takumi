import { mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { run } from "../harness";

const root = import.meta.dir;
const repo = join(root, "..", "..");
const packs = join(root, ".packs");
const packages = {
  "takumi-pdf": "takumi-pdf-js",
  helpers: "takumi-helpers",
};

rmSync(packs, { recursive: true, force: true });
rmSync(join(root, "node_modules"), { recursive: true, force: true });
rmSync(join(root, "bun.lock"), { force: true });
mkdirSync(packs);

for (const [name, directory] of Object.entries(packages)) {
  run(
    ["bun", "pm", "pack", "--quiet", "--filename", join(packs, `${name}.tgz`)],
    join(repo, directory),
  );
}

run(["bun", "install"], root);
