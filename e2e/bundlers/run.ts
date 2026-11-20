import {
  appendFileSync,
  cpSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { freePort, runLogged, serve, tail } from "../harness";
import { cells, type Cell } from "./matrix";

const PNG_MAGIC = "89504e47";
const INLINED_WASM_BYTES = 3_000_000;
const root = import.meta.dir;
const logs = join(root, ".logs");

interface Outcome {
  failure?: string;
  backend?: string;
}

interface OutputFiles {
  native: number;
  wasm: number;
  inlined: number;
  referencesAddon: boolean;
}

function* walk(directory: string): Generator<string> {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);

    if (entry.isDirectory()) {
      yield* walk(path);
    } else {
      yield path;
    }
  }
}

function inspect(out: string): OutputFiles {
  const files: OutputFiles = { native: 0, wasm: 0, inlined: 0, referencesAddon: false };

  for (const path of walk(out)) {
    if (path.endsWith(".node")) {
      files.native += 1;
    } else if (path.endsWith(".wasm")) {
      files.wasm += 1;
    } else if (/\.[cm]?js$/.test(path)) {
      if (statSync(path).size > INLINED_WASM_BYTES) {
        files.inlined += 1;
      } else if (!files.referencesAddon) {
        files.referencesAddon = readFileSync(path, "utf8").includes("@takumi-rs/core");
      }
    }
  }

  return files;
}

/** A shipped `.node` wins. Outputs that import the addon from node_modules ship none, so a reference to it counts as native. */
function detectBackend(files: OutputFiles): "native" | "wasm" | "none" {
  if (files.native > 0) {
    return "native";
  }

  if (files.wasm > 0 || files.inlined > 0) {
    return "wasm";
  }

  return files.referencesAddon ? "native" : "none";
}

function magicOf(output: string): string | undefined {
  return output.match(/^magic ([0-9a-f]+)$/m)?.[1];
}

async function serveCell(cell: Cell, directory: string, log: string): Promise<string | undefined> {
  const port = await freePort();
  const response = await serve([cell.runtime, cell.entry ?? ""], {
    cwd: directory,
    port,
    path: cell.path ?? "/",
    log,
  });

  if (!response) {
    return undefined;
  }

  return response.status === 200
    ? `magic ${response.body.subarray(0, 4).toString("hex")}`
    : `status ${response.status}: ${tail(readFileSync(log, "utf8"))}`;
}

async function render(cell: Cell, out: string, log: string): Promise<string | undefined> {
  const entry = cell.entry ?? "";
  let directory = out;

  if (cell.isolated) {
    directory = mkdtempSync(join(tmpdir(), "takumi-e2e-"));
    cpSync(out, directory, { recursive: true });
  }

  try {
    if (cell.runtime === "workerd") {
      const run = await runLogged(["bun", join(root, "workerd.ts"), directory, entry], {
        cwd: root,
        log,
      });

      return run.exitCode === 0
        ? magicOf(run.output())
        : `workerd exited ${run.exitCode}: ${tail(run.output())}`;
    }

    if (cell.mode === "server") {
      const result = await serveCell(cell, directory, log);

      return result?.startsWith("magic")
        ? magicOf(result)
        : (result ?? `no response: ${tail(readFileSync(log, "utf8"))}`);
    }

    const run = await runLogged([cell.runtime, entry], { cwd: directory, log });

    return run.exitCode === 0
      ? magicOf(run.output())
      : `exit ${run.exitCode}: ${tail(run.output())}`;
  } finally {
    if (cell.isolated) {
      rmSync(directory, { recursive: true, force: true });
    }
  }
}

async function check(cell: Cell): Promise<Outcome> {
  const cwd = join(root, cell.cwd ?? ".");
  const out = join(cwd, cell.out);
  const log = join(logs, cell.name.replaceAll(/[^\w]+/g, "-"));

  // A stale Wrangler redirect to a deleted output makes the next build fail.
  rmSync(join(cwd, ".wrangler"), { recursive: true, force: true });
  rmSync(out, { recursive: true, force: true });

  const build = await runLogged(cell.build, { cwd, env: cell.env, log: `${log}-build.log` });

  if (build.exitCode !== 0) {
    return { failure: `build exited ${build.exitCode}: ${tail(build.output())}` };
  }

  const files = inspect(out);
  const backend = detectBackend(files);
  const problems: string[] = [];

  if (backend !== cell.backend) {
    problems.push(`expected ${cell.backend} backend, found ${JSON.stringify(files)}`);
  }

  if (files.inlined > 0) {
    problems.push(
      `${files.inlined} JS file(s) over ${INLINED_WASM_BYTES} bytes, likely inlined wasm`,
    );
  }

  if (cell.runtime !== "none") {
    const magic = await render(cell, out, `${log}-run.log`);

    if (magic !== PNG_MAGIC) {
      problems.push(`render returned ${magic ?? "nothing"}`);
    }
  }

  return { backend, failure: problems.join("; ") || undefined };
}

const selected = process.argv.slice(2);
const chosen =
  selected.length > 0
    ? cells.filter((cell) => selected.some((name) => cell.name.includes(name)))
    : cells;
const lines: string[] = [];
let unexpected = 0;
const started = performance.now();

rmSync(logs, { recursive: true, force: true });
mkdirSync(logs);

for (const cell of chosen) {
  const begin = performance.now();
  const outcome = await check(cell);
  const seconds = ((performance.now() - begin) / 1000).toFixed(1);
  const backend =
    outcome.backend && outcome.backend !== cell.backend
      ? `${cell.backend}, found ${outcome.backend}`
      : cell.backend;
  const [mark, note] = cell.expectedFailure
    ? outcome.failure
      ? ["○", `fails as expected: ${cell.expectedFailure}\n    found: ${outcome.failure}`]
      : ["✗", `passes, but expected to fail: ${cell.expectedFailure}`]
    : outcome.failure
      ? ["✗", outcome.failure]
      : ["✓", undefined];

  if (mark === "✗") {
    unexpected += 1;
  }

  const line = `${mark} ${cell.name} (${cell.runtime}, ${backend}) ${seconds}s`;
  const entry = note ? `${line}\n    ${note}` : line;

  lines.push(entry);
  console.log(entry);
}

const summary = `${chosen.length} cells in ${((performance.now() - started) / 1000).toFixed(0)}s, ${unexpected} unexpected`;

console.log(`\n${summary}`);

if (Bun.env.GITHUB_STEP_SUMMARY) {
  appendFileSync(
    Bun.env.GITHUB_STEP_SUMMARY,
    `\`\`\`\n${lines.join("\n")}\n\n${summary}\n\`\`\`\n`,
  );
}

process.exit(unexpected > 0 ? 1 : 0);
