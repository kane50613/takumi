import { Glob } from "bun";
import { cpSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = import.meta.dir;
const repo = join(root, "..", "..");
const packs = join(root, ".packs");
const packages = {
  "takumi-js": "takumi-js",
  core: "takumi-napi",
  wasm: "takumi-wasm",
  helpers: "takumi-helpers",
};

function run(command: string[], cwd: string): void {
  const { exitCode } = Bun.spawnSync(command, { cwd, stdout: "inherit", stderr: "inherit" });

  if (exitCode !== 0) {
    throw new Error(`${command.join(" ")} exited with ${exitCode}`);
  }
}

// The published core package loads its binary from a per-platform package, so build one from the local `.node`.
function packPlatformPackage(): string {
  const [binary] = [...new Glob("core.*.node").scanSync(join(repo, "takumi-napi"))];

  if (!binary) {
    throw new Error("takumi-napi has no built core.*.node binary");
  }

  const name = `@takumi-rs/core-${binary.slice("core.".length, -".node".length)}`;
  const directory = join(packs, "platform");

  mkdirSync(directory, { recursive: true });
  cpSync(join(repo, "takumi-napi", binary), join(directory, binary));
  writeFileSync(
    join(directory, "package.json"),
    JSON.stringify({ name, version: "0.0.0", main: binary }),
  );
  run(["bun", "pm", "pack", "--quiet", "--filename", join(packs, "platform.tgz")], directory);

  return join(packs, "platform.tgz");
}

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

const platformPackage = packPlatformPackage();

run(["bun", "install"], root);
run(["bun", "add", "--no-save", platformPackage], root);
