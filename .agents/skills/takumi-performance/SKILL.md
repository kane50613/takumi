---
name: takumi-performance
description: Use when Takumi renders are slow, either the first render or repeated ones, or when a render server's memory grows with traffic or restarts out of memory. This includes sizing the renderer's `cache` pool, reading `cacheStats()`, and profiling a render route.
---

# Takumi performance

Measure before you change a budget. A cache helps only when later renders reuse what it keeps, and [performance.md](https://takumi.kane.tw/docs/performance.md) says what each cache holds and how long.

## Steps

1. **Measure cold and warm renders.** Time the first render in a fresh process, then repeated renders in the same process with representative text, images, formats, and concurrency. For memory, record the process RSS across a few hundred renders. Done when you have the cold time, the warm time, and an RSS series for the slow template.

2. **Move setup out of the request.** Done when each of these holds:
   - Any `Renderer` is created at module level, as step 6 of `takumi-og-image` describes.
   - Every font comes from a module-level `fonts` list.
   - No code sets `images.fetchCache`, since renderers keep downloads in their own bounded store.

3. **Read what the cache keeps.** Images, glyphs, fonts, and stylesheets share one pool of `cache.maxBytes`, 128 MiB by default. `cacheStats()` from `takumi-js` reports what the pool holds and the hits, misses, and evictions of `images`, `glyphs`, and `fonts`. Raise `maxBytes` on a `Renderer` from `takumi-js/node` when a kind evicts under steady traffic, or set a kind to `"none"` when nothing reuses it. Done when each change has a before and after `cacheStats()` and timing showing it helps.

4. **Keep RSS flat on Linux.** When RSS keeps growing while the caches are bounded, try the allocator settings in the "Keep memory flat on Linux servers" section and record the RSS series again. Done when RSS levels off across the run, or you have reported the series to the user with the settings tried.
