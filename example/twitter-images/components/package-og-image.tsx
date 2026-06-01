import {
  Box,
  Code,
  File,
  Folder,
  Heart,
  List,
  type LucideIcon,
  Package,
  Puzzle,
  Scale,
  Star,
  Type,
  Variable,
} from "lucide-react";
import { createElement } from "react";
import { SiGithub } from "@icons-pack/react-simple-icons";

type Variant = "download-chart" | "code-tree" | "function-tree";

type TreeRow = {
  name: string;
  depth: number;
  isDir: boolean;
};

type SymbolKind =
  | "function"
  | "class"
  | "interface"
  | "typeAlias"
  | "variable"
  | "enum"
  | "namespace";

type SymbolRow = {
  name: string;
  kind: "section" | "symbol";
  symbolKind: SymbolKind;
};

type JsDelivrFileNode = {
  name: string;
  type: "file" | "directory";
  files?: JsDelivrFileNode[];
};

type PackageInfo = {
  packageName: string;
  version: string | null;
  siteName: string;
  repository: string;
  stars: number;
  likes: number;
  license: string;
  weeklyDownloads: number[];
  treeRows: TreeRow[];
  symbolRows: SymbolRow[];
};

type DailyRawPoint = {
  day: string;
  value: number;
};

type WeeklyDataPoint = {
  value: number;
  weekKey: string;
  weekStart: string;
  weekEnd: string;
  timestampStart: number;
  timestampEnd: number;
};

const compactFormat = new Intl.NumberFormat("en", {
  notation: "compact",
  maximumFractionDigits: 1,
});

const symbolKindIcons: Record<SymbolKind, LucideIcon> = {
  function: Code,
  class: Box,
  interface: Puzzle,
  typeAlias: Type,
  variable: Variable,
  enum: List,
  namespace: Package,
};

const symbolKindTitles: Record<SymbolKind, string> = {
  function: "Functions",
  class: "Classes",
  interface: "Interfaces",
  typeAlias: "Type Aliases",
  variable: "Variables",
  enum: "Enums",
  namespace: "Namespaces",
};

const demoWeeklyDownloads = [
  330_000, 350_000, 360_000, 410_000, 390_000, 420_000, 480_000, 520_000, 530_000, 570_000, 590_000,
  610_000, 640_000, 620_000, 650_000, 670_000, 700_000, 730_000, 760_000, 790_000, 820_000, 860_000,
  900_000, 940_000, 980_000, 1_010_000, 1_040_000, 1_090_000, 1_130_000, 1_170_000, 1_220_000,
  1_260_000, 1_310_000, 1_360_000, 1_410_000, 1_460_000, 1_520_000, 1_580_000, 1_630_000, 1_700_000,
  1_760_000, 1_820_000, 1_900_000, 1_970_000, 2_050_000, 2_130_000, 2_220_000, 2_300_000, 2_400_000,
  2_510_000, 2_620_000, 2_740_000,
];

const demoTreeRows: TreeRow[] = [
  { name: "dist", depth: 0, isDir: true },
  { name: "index.mjs", depth: 1, isDir: false },
  { name: "index.d.ts", depth: 1, isDir: false },
  { name: "core", depth: 0, isDir: true },
  { name: "renderer.ts", depth: 1, isDir: false },
  { name: "styles.ts", depth: 1, isDir: false },
  { name: "utils", depth: 0, isDir: true },
  { name: "color.ts", depth: 1, isDir: false },
  { name: "font.ts", depth: 1, isDir: false },
  { name: "README.md", depth: 0, isDir: false },
  { name: "package.json", depth: 0, isDir: false },
];

const demoSymbolRows: SymbolRow[] = [
  { name: "Functions", kind: "section", symbolKind: "function" },
  { name: "render", kind: "symbol", symbolKind: "function" },
  { name: "loadFont", kind: "symbol", symbolKind: "function" },
  { name: "Classes", kind: "section", symbolKind: "class" },
  { name: "Renderer", kind: "symbol", symbolKind: "class" },
  { name: "Interfaces", kind: "section", symbolKind: "interface" },
  { name: "RenderOptions", kind: "symbol", symbolKind: "interface" },
  { name: "Type Aliases", kind: "section", symbolKind: "typeAlias" },
  { name: "CSSProperties", kind: "symbol", symbolKind: "typeAlias" },
  { name: "Variables", kind: "section", symbolKind: "variable" },
  { name: "defaultConfig", kind: "symbol", symbolKind: "variable" },
  { name: "Enums", kind: "section", symbolKind: "enum" },
  { name: "RenderMode", kind: "symbol", symbolKind: "enum" },
  { name: "Namespaces", kind: "section", symbolKind: "namespace" },
  { name: "Takumi", kind: "symbol", symbolKind: "namespace" },
];

const maxRows = 25;

const defaultPackageInfo: PackageInfo = {
  packageName: "takumi-js/helpers",
  version: "0.7.0",
  siteName: "takumi.kane.tw",
  repository: "kane50613/takumi",
  stars: 3020,
  likes: 841,
  license: "MIT",
  weeklyDownloads: demoWeeklyDownloads,
  treeRows: demoTreeRows,
  symbolRows: demoSymbolRows,
};

const requestedPackageName = defaultPackageInfo.packageName;
const requestedVersion = defaultPackageInfo.version;

export const name = "package-og-image";

export const width = 1200;
export const height = 630;

export const fonts = ["geist/Geist[wght].woff2", "geist/GeistMono[wght].woff2"];

// TODO: provide image (logo.svg) via fetchedResources

function smoothPath(points: { x: number; y: number }[]) {
  if (points.length < 2) {
    return "0,0";
  }

  const firstPoint = points[0] as { x: number; y: number };
  const lastIndex = points.length - 1;
  const roundTo2 = (value: number) => Math.round(value * 100) / 100;
  const output = [`${roundTo2(firstPoint.x)},${roundTo2(firstPoint.y)}`];
  const slope: number[] = [];
  const tangent: number[] = [];

  for (let i = 0; i < lastIndex; i += 1) {
    const currentPoint = points[i] as { x: number; y: number };
    const nextPoint = points[i + 1] as { x: number; y: number };
    const deltaX = nextPoint.x - currentPoint.x;
    const deltaY = nextPoint.y - currentPoint.y;
    slope[i] = deltaX === 0 ? 0 : deltaY / deltaX;
  }

  tangent[0] = slope[0] ?? 0;
  tangent[lastIndex] = slope[lastIndex - 1] ?? 0;
  for (let i = 1; i < lastIndex; i += 1) {
    const previousSlope = slope[i - 1] ?? 0;
    const currentSlope = slope[i] ?? 0;
    tangent[i] =
      previousSlope * currentSlope <= 0
        ? 0
        : (2 * previousSlope * currentSlope) / (previousSlope + currentSlope);
  }

  for (let i = 0; i < lastIndex; i += 1) {
    const currentPoint = points[i] as { x: number; y: number };
    const nextPoint = points[i + 1] as { x: number; y: number };
    const x0 = currentPoint.x;
    const y0 = currentPoint.y;
    const x1 = nextPoint.x;
    const y1 = nextPoint.y;
    const segment = x1 - x0;
    const currentTangent = tangent[i] ?? 0;
    const nextTangent = tangent[i + 1] ?? 0;
    output.push(
      `C ${roundTo2(x0 + segment / 3)},${roundTo2(y0 + (currentTangent * segment) / 3)} ${roundTo2(x1 - segment / 3)},${roundTo2(y1 - (nextTangent * segment) / 3)} ${roundTo2(x1)},${roundTo2(y1)}`,
    );
  }

  return output.join(" ");
}

function toIsoDateString(date: Date): string {
  return date.toISOString().slice(0, 10);
}

function addDays(date: Date, days: number): Date {
  const updatedDate = new Date(date);
  updatedDate.setUTCDate(updatedDate.getUTCDate() + days);
  return updatedDate;
}

function parseIsoDateOnly(value: string): Date {
  return new Date(`${value}T00:00:00.000Z`);
}

function formatIsoDateOnly(date: Date): string {
  return date.toISOString().slice(0, 10);
}

function differenceInUtcDaysInclusive(startIso: string, endIso: string): number {
  const start = parseIsoDateOnly(startIso);
  const end = parseIsoDateOnly(endIso);
  return Math.floor((end.getTime() - start.getTime()) / 86400000) + 1;
}

function splitIsoRangeIntoChunksInclusive(
  startIso: string,
  endIso: string,
  maximumDaysPerRequest: number,
): Array<{ startIso: string; endIso: string }> {
  const totalDays = differenceInUtcDaysInclusive(startIso, endIso);
  if (totalDays <= maximumDaysPerRequest) {
    return [{ startIso, endIso }];
  }

  const chunks: Array<{ startIso: string; endIso: string }> = [];
  let cursorStart = parseIsoDateOnly(startIso);
  const finalEnd = parseIsoDateOnly(endIso);

  while (cursorStart.getTime() <= finalEnd.getTime()) {
    const cursorEnd = addDays(cursorStart, maximumDaysPerRequest - 1);
    const actualEnd = cursorEnd.getTime() < finalEnd.getTime() ? cursorEnd : finalEnd;
    chunks.push({
      startIso: formatIsoDateOnly(cursorStart),
      endIso: formatIsoDateOnly(actualEnd),
    });
    cursorStart = addDays(actualEnd, 1);
  }

  return chunks;
}

function mergeDailyPoints(points: DailyRawPoint[]): DailyRawPoint[] {
  const valuesByDay = new Map<string, number>();
  for (const point of points) {
    valuesByDay.set(point.day, (valuesByDay.get(point.day) ?? 0) + point.value);
  }

  return Array.from(valuesByDay.entries())
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([day, value]) => ({ day, value }));
}

function buildRollingWeeklyEvolutionFromDaily(
  daily: DailyRawPoint[],
  rangeStartIso: string,
  rangeEndIso: string,
): WeeklyDataPoint[] {
  const sorted = daily.slice().sort((a, b) => a.day.localeCompare(b.day));
  const rangeStartDate = parseIsoDateOnly(rangeStartIso);
  const rangeEndDate = parseIsoDateOnly(rangeEndIso);
  const groupedByIndex = new Map<number, number>();

  for (const item of sorted) {
    const itemDate = parseIsoDateOnly(item.day);
    const dayOffset = Math.floor((itemDate.getTime() - rangeStartDate.getTime()) / 86400000);
    if (dayOffset < 0) {
      continue;
    }

    const weekIndex = Math.floor(dayOffset / 7);
    groupedByIndex.set(weekIndex, (groupedByIndex.get(weekIndex) ?? 0) + item.value);
  }

  return Array.from(groupedByIndex.entries())
    .sort(([a], [b]) => a - b)
    .map(([weekIndex, value]) => {
      const weekStartDate = addDays(rangeStartDate, weekIndex * 7);
      const weekEndDate = addDays(weekStartDate, 6);
      const clampedWeekEndDate =
        weekEndDate.getTime() > rangeEndDate.getTime() ? rangeEndDate : weekEndDate;
      const weekStartIso = toIsoDateString(weekStartDate);
      const weekEndIso = toIsoDateString(clampedWeekEndDate);

      return {
        value,
        weekKey: `${weekStartIso}_${weekEndIso}`,
        weekStart: weekStartIso,
        weekEnd: weekEndIso,
        timestampStart: weekStartDate.getTime(),
        timestampEnd: clampedWeekEndDate.getTime(),
      };
    });
}

function toDateOnly(value?: string): string | null {
  if (!value) {
    return null;
  }

  const dateOnly = value.slice(0, 10);
  return /^\d{4}-\d{2}-\d{2}$/.test(dateOnly) ? dateOnly : null;
}

function fetchNpmDownloadsRange(packageName: string, startIso: string, endIso: string) {
  const encodedName = encodeURIComponent(packageName);
  return fetchJson<{ downloads: Array<{ day: string; downloads: number }> }>(
    `https://api.npmjs.org/downloads/range/${startIso}:${endIso}/${encodedName}`,
  );
}

async function fetchDailyRangeCached(packageName: string, startIso: string, endIso: string) {
  const response = await fetchNpmDownloadsRange(packageName, startIso, endIso);
  return [...response.downloads]
    .sort((a, b) => a.day.localeCompare(b.day))
    .map((d) => ({ day: d.day, value: d.downloads }));
}

async function fetchDailyRangeChunked(packageName: string, startIso: string, endIso: string) {
  const maximumDaysPerRequest = 540;
  const ranges = splitIsoRangeIntoChunksInclusive(startIso, endIso, maximumDaysPerRequest);
  if (ranges.length === 1) {
    return fetchDailyRangeCached(packageName, startIso, endIso);
  }

  const all: DailyRawPoint[] = [];
  for (const range of ranges) {
    const part = await fetchDailyRangeCached(packageName, range.startIso, range.endIso);
    all.push(...part);
  }

  return mergeDailyPoints(all);
}

function resolveWeekRange(weeks: number): { startIso: string; endIso: string } {
  const today = new Date();
  const yesterday = new Date(
    Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), today.getUTCDate() - 1),
  );
  const endDateOnly = toDateOnly(undefined);
  const end = endDateOnly ? parseIsoDateOnly(endDateOnly) : yesterday;
  const start = addDays(end, -(weeks * 7) + 1);

  return {
    startIso: toIsoDateString(start),
    endIso: toIsoDateString(end),
  };
}

async function fetchPackageDownloadEvolution(packageName: string, weeks = 52): Promise<number[]> {
  const { startIso, endIso } = resolveWeekRange(weeks);
  const sortedDaily = await fetchDailyRangeChunked(packageName, startIso, endIso);
  const weekly = buildRollingWeeklyEvolutionFromDaily(sortedDaily, startIso, endIso);
  return weekly.map((item) => item.value);
}

function buildSparklineSrc(values: number[]) {
  if (values.length < 2) {
    return "";
  }

  const chartWidth = 500;
  const chartHeight = 200;
  const padY = 8;
  const maxValue = Math.max(...values);
  const minValue = Math.min(...values);
  const valueRange = maxValue - minValue || 1;

  const points = values.map((value, index) => ({
    x: (index / (values.length - 1)) * chartWidth,
    y: padY + (1 - (value - minValue) / valueRange) * (chartHeight - padY * 2),
  }));
  const firstPoint = points[0];
  const lastPoint = points.at(-1);
  if (!firstPoint || !lastPoint) {
    return "";
  }

  const pathData = smoothPath(points);
  const firstX = firstPoint.x;
  const lastX = lastPoint.x;

  const svg = [
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${chartWidth} ${chartHeight}" fill="none" preserveAspectRatio="none">`,
    `<defs><linearGradient id="af" x1="0" y1="0" x2="0" y2="1">`,
    `<stop offset="0%" stop-color="white" stop-opacity="0.07"/>`,
    `<stop offset="100%" stop-color="white" stop-opacity="0.005"/>`,
    "</linearGradient></defs>",
    `<path d="M ${firstX},${chartHeight} L ${pathData} L ${lastX},${chartHeight} Z" fill="url(#af)"/>`,
    `<path d="M ${pathData}" stroke="rgba(255,255,255,0.18)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/>`,
    "</svg>",
  ].join("");

  return `data:image/svg+xml;base64,${btoa(svg)}`;
}

function normalizeGitUrl(rawUrl: string) {
  return rawUrl
    .replace(/^git\+/, "")
    .replace(/^git:\/\//, "https://")
    .replace(/^git@github.com:/, "https://github.com/")
    .replace(/\.git$/, "");
}

function extractRepositoryPath(repository: unknown) {
  if (!repository) {
    return null;
  }

  if (typeof repository === "string") {
    return normalizeGitUrl(repository);
  }

  if (typeof repository === "object" && repository && "url" in repository) {
    const repo = repository as { url?: string; directory?: string };
    if (!repo.url) {
      return null;
    }

    const base = normalizeGitUrl(repo.url);
    if (!repo.directory) {
      return base;
    }

    return `${base}/tree/HEAD/${repo.directory.replace(/^\/+/, "")}`;
  }

  return null;
}

function parseGitHubRepository(repositoryUrl: string | null) {
  if (!repositoryUrl) {
    return null;
  }

  const match = repositoryUrl.match(/github\.com\/(?<owner>[^/]+)\/(?<repo>[^/#]+)/i);
  if (!match?.groups?.owner || !match.groups.repo) {
    return null;
  }

  return {
    owner: match.groups.owner,
    repo: match.groups.repo,
  };
}

function extractLicense(license: unknown) {
  if (typeof license === "string") {
    return license;
  }

  if (typeof license === "object" && license && "type" in license) {
    const value = (license as { type?: string }).type;
    if (value && !value.includes(" ")) {
      return value;
    }
  }

  return "";
}

async function fetchJson<T>(url: string) {
  const response = await fetch(url, {
    headers: {
      "User-Agent": "takumi-og-image-example",
      Accept: "application/json",
    },
  });

  if (!response.ok) {
    throw new Error(`Request failed (${response.status}): ${url}`);
  }

  return (await response.json()) as T;
}

function sortedNodes(nodes: JsDelivrFileNode[]) {
  return [...nodes].sort((a, b) => {
    if (a.type !== b.type) {
      return a.type === "directory" ? -1 : 1;
    }

    return a.name.localeCompare(b.name);
  });
}

function flattenTree(nodes: JsDelivrFileNode[]) {
  const rows: TreeRow[] = [];

  function walk(currentNodes: JsDelivrFileNode[], depth: number) {
    for (const node of sortedNodes(currentNodes)) {
      if (rows.length >= maxRows) {
        return;
      }

      rows.push({ name: node.name, depth, isDir: node.type === "directory" });
      if (node.files) {
        walk(node.files, depth + 1);
      }
    }
  }

  walk(nodes, 0);
  return rows;
}

function findDeclarationFiles(nodes: JsDelivrFileNode[], prefix = "") {
  const declarationPaths: string[] = [];

  for (const node of sortedNodes(nodes)) {
    const currentPath = prefix ? `${prefix}/${node.name}` : node.name;

    if (node.type === "file" && currentPath.endsWith(".d.ts")) {
      declarationPaths.push(currentPath);
      continue;
    }

    if (node.files) {
      declarationPaths.push(...findDeclarationFiles(node.files, currentPath));
    }
  }

  return declarationPaths;
}

function buildSymbolRowsFromDeclarationSource(source: string) {
  const symbolMap: Record<SymbolKind, string[]> = {
    function: [],
    class: [],
    interface: [],
    typeAlias: [],
    variable: [],
    enum: [],
    namespace: [],
  };

  const patterns: Record<SymbolKind, RegExp> = {
    function: /\bexport\s+(?:declare\s+)?function\s+([A-Za-z_$][\w$]*)/g,
    class: /\bexport\s+(?:declare\s+)?class\s+([A-Za-z_$][\w$]*)/g,
    interface: /\bexport\s+interface\s+([A-Za-z_$][\w$]*)/g,
    typeAlias: /\bexport\s+type\s+([A-Za-z_$][\w$]*)\s*=/g,
    variable: /\bexport\s+(?:declare\s+)?(?:const|let|var)\s+([A-Za-z_$][\w$]*)/g,
    enum: /\bexport\s+(?:declare\s+)?enum\s+([A-Za-z_$][\w$]*)/g,
    namespace: /\bexport\s+(?:declare\s+)?namespace\s+([A-Za-z_$][\w$]*)/g,
  };

  (Object.keys(patterns) as SymbolKind[]).forEach((kind) => {
    const seen = new Set<string>();
    const matches = source.matchAll(patterns[kind]);

    for (const match of matches) {
      const symbolName = match[1];
      if (!symbolName || seen.has(symbolName)) {
        continue;
      }

      seen.add(symbolName);
      symbolMap[kind].push(symbolName);
      if (symbolMap[kind].length >= 8) {
        break;
      }
    }
  });

  const rows: SymbolRow[] = [];

  (Object.keys(symbolMap) as SymbolKind[]).forEach((kind) => {
    const symbols = symbolMap[kind];
    if (symbols.length === 0 || rows.length >= maxRows) {
      return;
    }

    rows.push({
      name: symbolKindTitles[kind],
      kind: "section",
      symbolKind: kind,
    });
    for (const symbolName of symbols) {
      if (rows.length >= maxRows) {
        break;
      }

      rows.push({ name: symbolName, kind: "symbol", symbolKind: kind });
    }
  });

  return rows;
}

async function fetchPackageVersionMeta(
  encodedName: string,
  packageName: string,
  version: string | null,
) {
  const registryMeta = await fetchJson<{
    "dist-tags"?: { latest?: string };
  }>(`https://registry.npmjs.org/${encodedName}`);
  const resolvedVersion = version ?? registryMeta["dist-tags"]?.latest ?? null;
  if (!resolvedVersion) {
    return null;
  }

  const packageVersionMeta = await fetchJson<{
    name?: string;
    repository?: unknown;
    license?: unknown;
  }>(`https://registry.npmjs.org/${encodedName}/${resolvedVersion}`);

  const packageDisplayName = packageVersionMeta.name ?? packageName;

  return {
    resolvedVersion,
    packageDisplayName,
    packageVersionMeta,
  };
}

async function fetchGitHubStars(repositoryUrl: string | null) {
  const repoRef = parseGitHubRepository(repositoryUrl);
  if (!repoRef) {
    return { repoRef: null, stars: 0 };
  }

  const repoMeta = await fetchJson<{ stargazers_count?: number }>(
    `https://api.github.com/repos/${repoRef.owner}/${repoRef.repo}`,
  ).catch(() => null);

  return {
    repoRef,
    stars: repoMeta?.stargazers_count ?? 0,
  };
}

function fetchJsDelivrFiles(encodedName: string, version: string) {
  return fetchJson<{ files?: JsDelivrFileNode[] }>(
    `https://data.jsdelivr.com/v1/packages/npm/${encodedName}@${version}`,
  )
    .then((result) => result.files ?? [])
    .catch(() => []);
}

async function resolveSymbolRows(files: JsDelivrFileNode[], encodedName: string, version: string) {
  const declarationCandidates = findDeclarationFiles(files).slice(0, 4);

  for (const declarationPath of declarationCandidates) {
    const declarationUrl = `https://cdn.jsdelivr.net/npm/${encodedName}@${version}/${declarationPath}`;
    const declarationSource = await fetch(declarationUrl)
      .then((response) => (response.ok ? response.text() : ""))
      .catch(() => "");

    if (!declarationSource) {
      continue;
    }

    const parsedRows = buildSymbolRowsFromDeclarationSource(declarationSource);
    if (parsedRows.length > 0) {
      return parsedRows;
    }
  }

  return demoSymbolRows;
}

async function loadPackageInfo(packageName: string, version: string | null): Promise<PackageInfo> {
  try {
    const encodedName = encodeURIComponent(packageName);
    const versionMeta = await fetchPackageVersionMeta(encodedName, packageName, version);
    if (!versionMeta) {
      return defaultPackageInfo;
    }

    const { packageDisplayName, packageVersionMeta, resolvedVersion } = versionMeta;
    const repositoryUrl = extractRepositoryPath(packageVersionMeta.repository);
    const { repoRef, stars } = await fetchGitHubStars(repositoryUrl);

    const weeklyDownloads = await fetchPackageDownloadEvolution(packageName, 52).catch(
      () => demoWeeklyDownloads,
    );

    const files = await fetchJsDelivrFiles(encodedName, resolvedVersion);
    const treeRows = files.length > 0 ? flattenTree(files) : demoTreeRows;
    const symbolRows =
      files.length > 0
        ? await resolveSymbolRows(files, encodedName, resolvedVersion)
        : demoSymbolRows;

    const repository = repoRef ? `${repoRef.owner}/${repoRef.repo}` : defaultPackageInfo.repository;

    return {
      packageName: packageDisplayName,
      version: resolvedVersion,
      siteName: defaultPackageInfo.siteName,
      repository,
      stars,
      likes: 0,
      license: extractLicense(packageVersionMeta.license),
      weeklyDownloads,
      treeRows,
      symbolRows,
    };
  } catch {
    return defaultPackageInfo;
  }
}

const packageInfo = await loadPackageInfo(requestedPackageName, requestedVersion);

function PackageTitleBlock({
  org,
  shortName,
  version,
}: {
  org: string | null;
  shortName: string;
  version: string | null;
}) {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: "0.75rem",
        maxWidth: "100%",
      }}
    >
      {org ? (
        <div
          style={{
            fontSize: "2.5rem",
            opacity: 0.5,
            lineClamp: 1,
            textOverflow: "ellipsis",
            fontFamily: "Geist Mono",
            lineHeight: 1,
            letterSpacing: "-0.03em",
          }}
        >
          {org}
        </div>
      ) : null}
      <div
        style={{
          fontSize: shortName.length > 20 ? "4rem" : "4.75rem",
          letterSpacing: "-0.05em",
          lineHeight: 1,
          lineClamp: 1,
          wordBreak: "break-all",
          textOverflow: "ellipsis",
          fontFamily: "Geist Mono",
        }}
      >
        {shortName}
      </div>
      {version ? (
        <div
          style={{
            fontSize: "2.25rem",
            opacity: 0.7,
            paddingTop: "0.75rem",
            lineHeight: 1,
            fontFamily: "Geist Mono",
          }}
        >
          v{version}
        </div>
      ) : null}
    </div>
  );
}

function PackageMetaRow({
  owner,
  repo,
  formattedStars,
  formattedLikes,
  license,
}: {
  owner: string;
  repo: string;
  formattedStars: string;
  formattedLikes: string;
  license: string;
}) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: "1.25rem",
        fontSize: "2rem",
        color: "rgba(226, 232, 240, 0.7)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
        <SiGithub width={28} height={28} fill="white" />
        <span>
          {owner}
          <span style={{ opacity: 0.5 }}>/</span>
          {repo}
        </span>
      </div>
      {formattedStars ? (
        <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
          <Star width={28} height={28} fill="white" />
          <span>{formattedStars}</span>
        </div>
      ) : null}
      {formattedLikes ? (
        <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
          <Heart width={28} height={28} fill="white" />
          <span>{formattedLikes}</span>
        </div>
      ) : null}
      {license && !license.includes(" ") ? (
        <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
          <Scale width={28} height={28} />
          <span>{license}</span>
        </div>
      ) : null}
    </div>
  );
}

function VariantPanel({
  variant,
  sparklineSrc,
  treeRows,
  symbolRows,
}: {
  variant: Variant;
  sparklineSrc: string;
  treeRows: TreeRow[];
  symbolRows: SymbolRow[];
}) {
  if (variant === "download-chart" && sparklineSrc) {
    return (
      <img
        src={sparklineSrc}
        alt="Download chart"
        style={{
          position: "absolute",
          left: 0,
          bottom: 0,
          width: "100%",
          height: "65%",
          opacity: 0.3,
        }}
      />
    );
  }

  if (variant === "code-tree" && treeRows.length > 0) {
    return (
      <div
        style={{
          position: "absolute",
          right: "2rem",
          top: "2rem",
          bottom: "2rem",
          width: 340,
          display: "flex",
          flexDirection: "column",
          gap: 2,
          opacity: 0.3,
          overflow: "hidden",
          fontSize: "1.25rem",
          lineHeight: 1.45,
          color: "rgba(226, 232, 240, 0.95)",
          fontFamily: "Geist Mono",
        }}
      >
        {treeRows.map((row, index) => (
          <div
            key={`${row.name}-${index}`}
            style={{
              display: "flex",
              alignItems: "center",
              paddingLeft: `${row.depth * 20}px`,
            }}
          >
            {row.isDir ? <Folder width={18} height={18} /> : <File width={18} height={18} />}
            <span style={{ marginLeft: "0.5rem" }}>{row.name}</span>
          </div>
        ))}
      </div>
    );
  }

  if (variant === "function-tree" && symbolRows.length > 0) {
    return (
      <div
        style={{
          position: "absolute",
          right: "2rem",
          top: "2rem",
          bottom: "2rem",
          width: 340,
          display: "flex",
          flexDirection: "column",
          gap: 2,
          opacity: 0.3,
          overflow: "hidden",
          fontSize: "1.25rem",
          lineHeight: 1.45,
          color: "rgba(226, 232, 240, 0.95)",
          fontFamily: "Geist Mono",
        }}
      >
        {symbolRows.map((row, index) => {
          const icon = symbolKindIcons[row.symbolKind];

          return (
            <div
              key={`${row.name}-${index}`}
              style={{
                display: "flex",
                alignItems: "center",
                paddingLeft: row.kind === "symbol" ? "20px" : 0,
              }}
            >
              {createElement(icon, { width: 18, height: 18 })}
              <span
                style={{
                  marginLeft: "0.5rem",
                  fontSize: row.kind === "section" ? "1.05rem" : "1.2rem",
                }}
              >
                {row.name}
              </span>
            </div>
          );
        })}
      </div>
    );
  }

  return null;
}

function PackageOgImage({
  packageName,
  version,
  siteName,
  repository,
  stars,
  likes,
  license,
  variant,
  treeRows,
  symbolRows,
  weeklyDownloads,
}: {
  packageName: string;
  version: string | null;
  siteName: string;
  repository: string;
  stars: number;
  likes: number;
  license: string;
  variant: Variant;
  treeRows: TreeRow[];
  symbolRows: SymbolRow[];
  weeklyDownloads: number[];
}) {
  const slashIndex = packageName.indexOf("/");
  const org =
    packageName.startsWith("@") && slashIndex > -1 ? packageName.slice(0, slashIndex) : null;
  const shortName =
    packageName.startsWith("@") && slashIndex > -1
      ? packageName.slice(slashIndex + 1)
      : packageName;
  const [owner, repo] = repository.split("/");
  const sparklineSrc = buildSparklineSrc(weeklyDownloads);
  const formattedStars = stars > 0 ? compactFormat.format(stars) : "";
  const formattedLikes = likes > 0 ? compactFormat.format(likes) : "";

  if (!owner || !repo) {
    throw new Error("Invalid repository");
  }

  return (
    <div
      style={{
        position: "relative",
        overflow: "hidden",
        width: "100%",
        height: "100%",
        backgroundColor: "#020617",
        color: "#e2e8f0",
        fontFamily: "Geist",
        display: "flex",
        flexDirection: "column",
        justifyContent: "center",
      }}
    >
      <div
        style={{
          position: "absolute",
          top: -40,
          left: 48,
          width: 700,
          height: 700,
          borderRadius: "50%",
          filter: "blur(64px)",
          backgroundColor: "rgba(226, 232, 240, 0.03)",
        }}
      />

      <div
        style={{
          padding: "3.75rem",
          display: "flex",
          flexDirection: "column",
          gap: "3rem",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
          <img src="logo.svg" width={60} height={60} alt="logo" />
          <h1
            style={{
              fontSize: "2.75rem",
              margin: 0,
              letterSpacing: "-0.04em",
              fontFamily: "Geist Mono",
            }}
          >
            {siteName}
          </h1>
        </div>

        <PackageTitleBlock org={org} shortName={shortName} version={version} />
        <PackageMetaRow
          owner={owner}
          repo={repo}
          formattedStars={formattedStars}
          formattedLikes={formattedLikes}
          license={license}
        />
      </div>
      <VariantPanel
        variant={variant}
        sparklineSrc={sparklineSrc}
        treeRows={treeRows}
        symbolRows={symbolRows}
      />
    </div>
  );
}

export default function PackageOgImageExample() {
  return <PackageOgImage {...packageInfo} variant="function-tree" />;
}
