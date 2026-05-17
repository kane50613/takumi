"use client";

import { HomeLayout } from "fumadocs-ui/layouts/home";
import { Loader2 } from "lucide-react";
import { lazy, Suspense } from "react";
import { baseOptions } from "~/layout-config";

const ImageEditor = lazy(() => import("~/components/playground/playground"));

export default function Playground() {
  return (
    <HomeLayout {...baseOptions}>
      <title>Playground</title>
      <meta name="description" content="You can try out Takumi and make quick prototypes here." />
      <meta
        name="og:description"
        content="You can try out Takumi and make quick prototypes here."
      />
      <Suspense fallback={<LoadingScreen />}>
        <ImageEditor />
      </Suspense>
    </HomeLayout>
  );
}

function LoadingScreen() {
  return (
    <div className="flex text-fd-muted-foreground justify-center items-center w-screen h-[calc(100dvh-3.5rem)] gap-2.5">
      <Loader2 className="animate-spin w-4" />
      <p>Loading the editor and Takumi wasm binary...</p>
    </div>
  );
}
