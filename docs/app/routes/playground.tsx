"use client";

import { HomeLayout } from "fumadocs-ui/layouts/home";
import { Loader2 } from "lucide-react";
import { lazy, Suspense } from "react";
import { baseOptions } from "~/layout-config";

const ImageEditor = lazy(() => import("~/components/playground/playground"));

const DESCRIPTION =
  "Write JSX, watch Takumi render it to an image in your browser — WASM, no server.";

export default function Playground() {
  return (
    <HomeLayout {...baseOptions}>
      <title>Playground</title>
      <meta name="description" content={DESCRIPTION} />
      <meta name="og:description" content={DESCRIPTION} />
      <Suspense fallback={<LoadingScreen />}>
        <ImageEditor />
      </Suspense>
    </HomeLayout>
  );
}

function LoadingScreen() {
  return (
    <div className="flex h-[calc(100dvh-3.5rem)] w-screen items-center justify-center gap-2.5 font-mono text-sm text-fd-muted-foreground">
      <Loader2 className="w-4 animate-spin" />
      <p>loading editor + takumi wasm…</p>
    </div>
  );
}
