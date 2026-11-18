import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import {
  Extractor,
  ExtractorConfig,
  ExtractorLogLevel,
  type IConfigFile,
} from "@microsoft/api-extractor";
import { $, Glob } from "bun";

// Pinned with the wasm and N-API builds in .github/workflows/ci.yml, since rustdoc JSON is unstable.
const NIGHTLY = "nightly-2026-08-05";

const root = resolve(import.meta.dir, "..");
const apiDir = join(root, "api");

type Exports = string | null | Exports[] | { [condition: string]: Exports };

interface PackageJson {
  name: string;
  private?: boolean;
  workspaces?: string[];
  exports?: Exports;
}

interface EntryPoint {
  specifier: string;
  conditions: string[];
  typesPath: string;
}

async function readPackageJson(dir: string): Promise<PackageJson> {
  return JSON.parse(await readFile(join(dir, "package.json"), "utf8"));
}

interface WorkspacePackage {
  dir: string;
  pkg: PackageJson;
}

async function publishedPackages(): Promise<WorkspacePackage[]> {
  const { workspaces = [] } = await readPackageJson(root);
  const packages: WorkspacePackage[] = [];

  for (const pattern of workspaces) {
    for await (const manifest of new Glob(`${pattern}/package.json`).scan(root)) {
      const dir = join(root, dirname(manifest));
      const pkg = await readPackageJson(dir);

      if (!pkg.private) {
        packages.push({ dir, pkg });
      }
    }
  }

  return packages.sort((a, b) => a.pkg.name.localeCompare(b.pkg.name));
}

/** Every ESM `types` target under one `exports` subpath, with the conditions that select it. */
function collectTypes(value: Exports, conditions: string[], found: Map<string, string[]>): void {
  if (value === null || typeof value === "string") {
    return;
  }

  if (Array.isArray(value)) {
    for (const item of value) {
      collectTypes(item, conditions, found);
    }

    return;
  }

  for (const [condition, target] of Object.entries(value)) {
    if (condition === "types" && typeof target === "string") {
      if (!target.endsWith(".d.cts")) {
        found.set(target, [...(found.get(target) ?? []), conditions.join(".") || "default"]);
      }
    } else {
      collectTypes(target, [...conditions, condition], found);
    }
  }
}

function entryPoints(dir: string, pkg: PackageJson): EntryPoint[] {
  if (
    pkg.exports === undefined ||
    pkg.exports === null ||
    typeof pkg.exports === "string" ||
    Array.isArray(pkg.exports)
  ) {
    throw new Error(`${pkg.name} needs an \`exports\` map of subpaths`);
  }

  return Object.entries(pkg.exports).flatMap(([subpath, target]) => {
    const found = new Map<string, string[]>();

    collectTypes(target, [], found);

    return [...found].map(([types, conditions]) => ({
      specifier: pkg.name + subpath.slice(1),
      conditions,
      typesPath: join(dir, types),
    }));
  });
}

/** The body of API Extractor's report for one declaration file, without its header or release tags. */
async function extractApi(dir: string, typesPath: string, tempDir: string): Promise<string> {
  const configObject: IConfigFile = {
    projectFolder: dir,
    mainEntryPointFilePath: typesPath,
    compiler: {
      overrideTsconfig: {
        compilerOptions: {
          target: "ESNext",
          module: "ESNext",
          moduleResolution: "bundler",
          lib: ["ESNext", "DOM", "DOM.Iterable"],
          typeRoots: [join(root, "node_modules/@types")],
          types: ["node"],
          strict: true,
          skipLibCheck: true,
        },
        files: [typesPath],
      },
    },
    apiReport: {
      enabled: true,
      reportFileName: "report",
      reportFolder: tempDir,
      reportTempFolder: tempDir,
      includeForgottenExports: true,
    },
    docModel: { enabled: false },
    dtsRollup: { enabled: false },
    tsdocMetadata: { enabled: false },
    newlineKind: "lf",
    messages: {
      compilerMessageReporting: { default: { logLevel: ExtractorLogLevel.Error } },
      extractorMessageReporting: {
        default: { logLevel: ExtractorLogLevel.None },
        "ae-forgotten-export": { logLevel: ExtractorLogLevel.None, addToApiReportFile: true },
      },
      tsdocMessageReporting: { default: { logLevel: ExtractorLogLevel.None } },
    },
  };

  const config = ExtractorConfig.prepare({
    configObject,
    configObjectFullPath: join(dir, "api-extractor.json"),
    packageJsonFullPath: join(dir, "package.json"),
  });

  const errors: string[] = [];
  const result = Extractor.invoke(config, {
    localBuild: true,
    messageCallback: (message) => {
      if (message.logLevel === ExtractorLogLevel.Error) {
        errors.push(message.formatMessageWithLocation(dir));
      }

      message.handled = true;
    },
  });

  if (!result.succeeded) {
    throw new Error(`API Extractor failed on ${typesPath}:\n${errors.join("\n")}`);
  }

  const report = await readFile(join(tempDir, "report.api.md"), "utf8");
  const body = /```ts\n([\s\S]*)```/.exec(report)?.[1] ?? "";

  return body
    .replace(/^\/\/ \(No @packageDocumentation comment for this package\)\n/m, "")
    .split("\n")
    .map((line) =>
      line
        .replace("// @public", "//")
        .replace(/ \(undocumented\)$/, "")
        .replace(/ by the entry point \S+$/, "")
        // A warning's declaration location moves with unrelated edits and bundle chunk names.
        .replace(/^\/\/ \S+\.d\.[cm]?ts:\d+:\d+ - /, "// "),
    )
    .filter((line) => !/^\s*\/\/$/.test(line))
    .join("\n")
    .trim();
}

/** One package's report: every entry point, with entry points that share a surface listed together. */
async function packageReport({ dir, pkg }: WorkspacePackage, tempDir: string): Promise<string> {
  const entries = entryPoints(dir, pkg);
  const surfaces = new Map<string, string>();
  const sections = new Map<string, string[]>();

  for (const entry of entries) {
    let surface = surfaces.get(entry.typesPath);

    if (surface === undefined) {
      surface = await extractApi(dir, entry.typesPath, tempDir);
      surfaces.set(entry.typesPath, surface);
    }

    const sameSpecifier = entries.filter(({ specifier }) => specifier === entry.specifier);
    const label =
      sameSpecifier.length > 1
        ? `\`${entry.specifier}\` (${entry.conditions.join(", ")})`
        : `\`${entry.specifier}\``;

    sections.set(surface, [...(sections.get(surface) ?? []), label]);
  }

  const body = [...sections]
    .map(([surface, labels]) => `## ${labels.join(", ")}\n\n\`\`\`ts\n${surface}\n\`\`\``)
    .join("\n\n");

  return `# ${pkg.name}\n\n${body}\n`;
}

async function writeJsReports(): Promise<void> {
  const jsDir = join(apiDir, "js");
  const tempDir = await mkdtemp(join(tmpdir(), "takumi-api-"));

  await rm(jsDir, { recursive: true, force: true });

  try {
    for (const workspacePackage of await publishedPackages()) {
      const file = join(jsDir, `${workspacePackage.pkg.name}.api.md`);

      await mkdir(dirname(file), { recursive: true });
      await writeFile(file, await packageReport(workspacePackage, tempDir));
    }
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
}

interface CargoMetadata {
  packages: { name: string; publish: string[] | null; targets: { kind: string[] }[] }[];
}

async function writeRustReports(): Promise<void> {
  const metadata: CargoMetadata = await $`cargo metadata --format-version 1 --no-deps`
    .cwd(root)
    .quiet()
    .json();
  const crates = metadata.packages
    .filter(
      ({ publish, targets }) =>
        publish?.length !== 0 && targets.some(({ kind }) => kind.includes("lib")),
    )
    .map(({ name }) => name)
    .sort();

  const rustDir = join(apiDir, "rust");

  await rm(rustDir, { recursive: true, force: true });
  await mkdir(rustDir, { recursive: true });

  for (const crate of crates) {
    const api =
      await $`cargo +${NIGHTLY} public-api --package ${crate} --all-features --simplified --color never`
        .cwd(root)
        .quiet()
        .text();

    await writeFile(join(rustDir, `${crate}.txt`), api);
  }
}

const target = process.argv[2] ?? "all";

if (target !== "all" && target !== "js" && target !== "rust") {
  throw new Error(`usage: bun run api [all|js|rust], got ${target}`);
}

if (target !== "rust") {
  await writeJsReports();
}

if (target !== "js") {
  await writeRustReports();
}
