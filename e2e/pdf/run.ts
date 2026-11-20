import { appendFileSync, cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { freePort, runLogged, serve, tail } from "../harness";

interface Cell {
  name: string;
  /** Directory the commands run in, relative to this folder. */
  cwd: string;
  build?: string[];
  /** Build output, relative to `cwd`. The server runs from a copy of it with no parent node_modules. */
  out?: string;
  /** Starts a server that serves the PDF at `/`. */
  serve: (port: number) => string[];
}

const PDF_MAGIC = "%PDF-";
const root = import.meta.dir;
const logs = join(root, ".logs");
const nitro = "../node_modules/.bin/nitro";
const nuxt = "../node_modules/.bin/nuxt";

const cells: Cell[] = [
  { name: "nitro dev", cwd: "nitro", serve: (port) => [nitro, "dev", "--port", String(port)] },
  {
    name: "nitro build",
    cwd: "nitro",
    build: [nitro, "build"],
    out: ".output",
    serve: () => ["node", "server/index.mjs"],
  },
  {
    name: "nuxt dev",
    cwd: "nuxt",
    serve: (port) => [nuxt, "dev", "--no-fork", "--host", "127.0.0.1", "--port", String(port)],
  },
  {
    name: "nuxt build",
    cwd: "nuxt",
    build: [nuxt, "build"],
    out: ".output",
    serve: () => ["node", "server/index.mjs"],
  },
];

async function check(cell: Cell): Promise<string | undefined> {
  const cwd = join(root, cell.cwd);
  const log = join(logs, cell.name.replaceAll(/[^\w]+/g, "-"));
  let directory = cwd;

  if (cell.build) {
    rmSync(join(cwd, cell.out ?? ""), { recursive: true, force: true });

    const build = await runLogged(cell.build, { cwd, log: `${log}-build.log` });

    if (build.exitCode !== 0) {
      return `build exited ${build.exitCode}: ${tail(build.output())}`;
    }
  }

  if (cell.out) {
    directory = mkdtempSync(join(tmpdir(), "takumi-pdf-e2e-"));
    cpSync(join(cwd, cell.out), directory, { recursive: true });
  }

  try {
    const port = await freePort();
    const response = await serve(cell.serve(port), {
      cwd: directory,
      port,
      path: "/",
      log: `${log}-run.log`,
    });

    if (!response) {
      return `no response: ${tail(readFileSync(`${log}-run.log`, "utf8"))}`;
    }

    const magic = response.body.subarray(0, PDF_MAGIC.length).toString("latin1");

    if (response.status !== 200 || response.type !== "application/pdf" || magic !== PDF_MAGIC) {
      return `status ${response.status} ${response.type}: ${tail(readFileSync(`${log}-run.log`, "utf8"))}`;
    }

    return undefined;
  } finally {
    if (cell.out) {
      rmSync(directory, { recursive: true, force: true });
    }
  }
}

const selected = process.argv.slice(2);
const chosen =
  selected.length > 0
    ? cells.filter((cell) => selected.some((name) => cell.name.includes(name)))
    : cells;
const lines: string[] = [];
let failed = 0;
const started = performance.now();

rmSync(logs, { recursive: true, force: true });
mkdirSync(logs);

for (const cell of chosen) {
  const begin = performance.now();
  const failure = await check(cell);
  const seconds = ((performance.now() - begin) / 1000).toFixed(1);
  const line = `${failure ? "✗" : "✓"} ${cell.name} ${seconds}s`;
  const entry = failure ? `${line}\n    ${failure}` : line;

  if (failure) {
    failed += 1;
  }

  lines.push(entry);
  console.log(entry);
}

const summary = `${chosen.length} cells in ${((performance.now() - started) / 1000).toFixed(0)}s, ${failed} failed`;

console.log(`\n${summary}`);

if (Bun.env.GITHUB_STEP_SUMMARY) {
  appendFileSync(
    Bun.env.GITHUB_STEP_SUMMARY,
    `\`\`\`\n${lines.join("\n")}\n\n${summary}\n\`\`\`\n`,
  );
}

process.exit(failed > 0 ? 1 : 0);
