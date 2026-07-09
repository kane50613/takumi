"use client";

import { Loader2 } from "lucide-react";
import { lazy, Suspense } from "react";

const ImageEditor = lazy(() => import("~/components/playground/playground"));

export function LazyPlayground() {
  return (
    <Suspense fallback={<LoadingScreen />}>
      <ImageEditor />
    </Suspense>
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
