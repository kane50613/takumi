import { readFileSync } from "node:fs";
import { createServer } from "node:net";

const SERVER_TIMEOUT_MS = 60_000;

export interface Served {
  status: number;
  type: string | null;
  body: Buffer;
}

export function run(command: string[], cwd: string): void {
  const { exitCode } = Bun.spawnSync(command, { cwd, stdout: "inherit", stderr: "inherit" });

  if (exitCode !== 0) {
    throw new Error(`${command.join(" ")} exited with ${exitCode}`);
  }
}

export async function runLogged(
  command: string[],
  options: { cwd: string; env?: Record<string, string>; log: string },
) {
  const stream = Bun.file(options.log);
  const process = Bun.spawn(command, {
    cwd: options.cwd,
    env: { ...Bun.env, ...options.env },
    stdout: stream,
    stderr: stream,
  });

  return { exitCode: await process.exited, output: () => readFileSync(options.log, "utf8") };
}

export function tail(text: string): string {
  const lines = text.trim().split("\n");

  return (lines.find((line) => /\w+Error: /.test(line)) ?? lines.slice(-6).join(" | ")).slice(
    0,
    300,
  );
}

export async function freePort(): Promise<number> {
  const server = createServer();

  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));

  const address = server.address();

  await new Promise((resolve) => server.close(resolve));

  if (typeof address !== "object" || !address) {
    throw new Error("could not reserve a port");
  }

  return address.port;
}

/** Starts a server on `port` and fetches `path` once it answers. A dev server answers 503 while it builds, so that counts as not ready. */
export async function serve(
  command: string[],
  options: { cwd: string; port: number; path: string; log: string; env?: Record<string, string> },
): Promise<Served | undefined> {
  const stream = Bun.file(options.log);
  const server = Bun.spawn(command, {
    cwd: options.cwd,
    env: { ...Bun.env, PORT: String(options.port), HOSTNAME: "127.0.0.1", ...options.env },
    stdout: stream,
    stderr: stream,
  });
  const deadline = Date.now() + SERVER_TIMEOUT_MS;

  try {
    while (Date.now() < deadline && server.exitCode === null) {
      try {
        const response = await fetch(`http://127.0.0.1:${options.port}${options.path}`, {
          signal: AbortSignal.timeout(Math.max(deadline - Date.now(), 1)),
        });
        const body = Buffer.from(await response.arrayBuffer());

        if (response.status !== 503) {
          return { status: response.status, type: response.headers.get("content-type"), body };
        }
      } catch {
        // Not listening yet.
      }

      await Bun.sleep(500);
    }

    return undefined;
  } finally {
    server.kill();
    await server.exited;
  }
}
