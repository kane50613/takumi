"use client";

import { lazy, Suspense } from "react";
import { LoadingScreen } from "./loading-screen";

const Playground = lazy(() => import("./playground"));

export function LazyPlayground() {
  return (
    <Suspense
      fallback={
        <div className="relative h-[calc(100dvh-3.5rem)] w-full">
          <LoadingScreen />
        </div>
      }
    >
      <Playground />
    </Suspense>
  );
}
