import { chmod, copyFile } from "node:fs/promises";
import { join } from "node:path";

const target = process.argv[2];

if (!target) {
  throw new Error(
    "Pass the Rust target whose binary this package ships, such as aarch64-apple-darwin.",
  );
}

const executable = target.includes("windows") ? "takumi.exe" : "takumi";

await copyFile(join(import.meta.dir, "..", "artifacts", `cli-${target}`, executable), executable);
await chmod(executable, 0o755);
