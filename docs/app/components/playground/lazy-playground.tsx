"use client";

import { AxeIcon } from "lucide-react";
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
    <div className="playground-loading flex h-[calc(100dvh-3.5rem)] w-screen items-center justify-center gap-2.5 font-mono text-sm text-fd-muted-foreground">
      <AxeIcon className="playground-breathe size-4" />
      <p className="playground-breathe">loading editor + takumi wasm…</p>
    </div>
  );
}
