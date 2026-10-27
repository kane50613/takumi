import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

const cases = ["control", "ledger"];
const warmups = 2;
const samples = 9;

function fixture(name) {
  if (name === "control") {
    return { type: "text", text: "Hello PDF", style: { fontSize: 16 } };
  }
  return {
    type: "container",
    style: { display: "flex", flexDirection: "column", gap: 4, fontSize: 11 },
    children: [
      { type: "text", text: "Quarter-hour energy ledger", style: { fontSize: 24 } },
      ...Array.from({ length: 96 }, (_, index) => ({
        type: "container",
        style: { display: "flex", flexDirection: "row", gap: 8, padding: 6 },
        children: Array.from({ length: 8 }, (_, column) => ({
          type: "text",
          text: `${index}:${column} ${(index * 17.3 + column * 0.7).toFixed(2)} kWh`,
          style: { width: 80, fontWeight: column === 0 ? 600 : 400 },
        })),
      })),
    ],
  };
}

async function child(wasmPath, name, mode) {
  const phases = {};
  let start = performance.now();
  const { PdfRenderer, initSync } = await import("../../dist/export.mjs");
  phases.importMs = performance.now() - start;
  start = performance.now();
  const bytes = readFileSync(wasmPath);
  phases.readWasmMs = performance.now() - start;
  start = performance.now();
  initSync({ module: bytes });
  phases.initMs = performance.now() - start;
  start = performance.now();
  const renderer = new PdfRenderer();
  phases.constructorMs = performance.now() - start;
  start = performance.now();
  const font = readFileSync(
    new URL("../../../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf", import.meta.url),
  );
  phases.readFontMs = performance.now() - start;
  start = performance.now();
  await renderer.registerFont({ name: "Archivo", data: font });
  phases.registerFontMs = performance.now() - start;
  const node = fixture(name);
  const options = {
    size: "a4",
    landscape: true,
    fontFamilies: ["Archivo"],
    footer: { type: "text", text: "0 / 0", className: "pageNumber" },
  };
  let pdf;
  let hash;
  const times = [];
  try {
    for (let run = mode === "warm" ? -warmups : 0; run < (mode === "warm" ? samples : 1); run++) {
      start = performance.now();
      pdf = await renderer.render(node, options);
      const elapsed = performance.now() - start;
      if (run >= 0) times.push(elapsed);
      const currentHash = createHash("sha256").update(pdf).digest("hex");
      if (hash !== undefined && currentHash !== hash)
        throw new Error(`PDF bytes differ for ${name}/${mode}, render ${run}`);
      hash = currentHash;
    }
  } finally {
    renderer.free();
  }
  if (!pdf) throw new Error("No PDF rendered");
  process.on("exit", () => {
    process.stdout.write(
      `${JSON.stringify({
        ...phases,
        renderMs: times,
        processMs: performance.now(),
        rssKiBAtExit: process.resourceUsage().maxRSS,
        pdfBytes: pdf.length,
        sha256: hash,
      })}\n`,
    );
  });
}

function statistics(values) {
  const sorted = values.toSorted((a, b) => a - b);
  const median = sorted[Math.floor(sorted.length / 2)];
  return { median, min: sorted[0], max: sorted.at(-1) };
}

function compare(paths) {
  for (const path of paths) {
    const wasm = readFileSync(path);
    console.log(
      JSON.stringify({
        path,
        wasmBytes: wasm.length,
        gzipBytes: gzipSync(wasm, { level: 9 }).length,
      }),
    );
  }
  for (const name of cases) {
    let hash;
    for (const mode of ["cold", "warm"]) {
      const results = new Map(paths.map((path) => [path, []]));
      for (let run = -warmups; run < samples; run++) {
        for (const path of run % 2 ? paths.toReversed() : paths) {
          const start = performance.now();
          const result = spawnSync(
            process.execPath,
            [fileURLToPath(import.meta.url), "--child", path, name, mode],
            { encoding: "utf8", timeout: 120_000 },
          );
          const wallMs = performance.now() - start;
          if (result.status !== 0) throw new Error(result.stderr || String(result.error));
          const output = JSON.parse(result.stdout);
          if (hash !== undefined && output.sha256 !== hash)
            throw new Error(`PDF bytes differ for ${name}/${mode}, process ${run}: ${path}`);
          hash = output.sha256;
          if (run >= 0) results.get(path).push({ wallMs, ...output });
        }
      }
      for (const [path, runs] of results) {
        console.log(
          JSON.stringify({
            name,
            mode,
            path,
            wallMs: statistics(runs.map((run) => run.wallMs)),
            renderMs: statistics(runs.flatMap((run) => run.renderMs)),
            perProcessRenderMedianMs: statistics(
              runs.map((run) => statistics(run.renderMs).median),
            ),
            runs,
          }),
        );
      }
    }
  }
}

if (process.argv[2] === "--child") {
  await child(process.argv[3], process.argv[4], process.argv[5]);
} else {
  const paths = process.argv.slice(2);
  if (paths.length !== 2)
    throw new Error("Pass baseline.wasm and candidate.wasm built from the same source");
  compare(paths);
}
