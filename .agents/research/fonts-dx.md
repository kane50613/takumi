# 私有字型在 Workers 上的載入 DX

研究日期:2026-08-30
範圍:Cloudflare Workers + takumi-js + 私有 TTF(以 10–30MB 的 CJK 字型為最壞情況)

## 結論

引擎與 JS 管線已經具備「只載入用到的 subset」的全部機制。缺的是把私有 TTF 切成 subset 的那一步:Google Fonts 有 css2 幫使用者切,私有字型沒有任何工具。最有價值的交付是一個 build-time 切片工具加一頁文件,不是引擎改動。

## 現有機制(已驗證,不用重做)

| 層          | 機制                                                                                                        | 位置                                |
| ----------- | ----------------------------------------------------------------------------------------------------------- | ----------------------------------- |
| Rust 註冊   | `FontResource::subset_of` + `subset_rank`:同邏輯 family 下多個 subset,shaper 按 rank 走 cmap 撿第一個涵蓋的 | `takumi-core/src/resources/font.rs` |
| Rust 記憶體 | `FontSource::from_shared` / `from_static` 零拷貝。blob id 用 content hash,glyph cache 跨 renderer 共享      | 同上                                |
| JS 過濾     | `subsetFonts`:收集內容樹碼點,丟掉 `ranges` 不相交的 subset;無 range 的字型永遠保留                          | `takumi-helpers/src/fonts.ts`       |
| JS 惰性載入 | `FontSubset.data()` 是 thunk,被 `subsetFonts` 濾掉的 subset 根本不會 fetch                                  | 同上                                |
| JS 去重     | `FontRegistry` 以 `key` 去重,同 process 只註冊一次                                                          | `takumi-helpers/src/renderer.ts`    |
| 管線接線    | `prepareRenderInput` 對每次 render 自動跑 subsetFonts(含 list marker 與 counter 字元)                       | 同上                                |

`googleFonts()` 把這條線走完了:css2 回應的 `unicode-range` 變成 `FontSubset[]`,CJK 字型約 120 個 slice,一份文件實際只抓兩三個。

## 缺口:私有 TTF 走不進這條線

私有 20MB TTF 現在只有一條路:整份 fetch → 整份進 wasm linear memory。

- Workers isolate 上限 128MB。wasm linear memory 只長不縮,20MB 高水位常駐,多字型疊加後 evict 機率上升。
- 每次 cold start 都要從 R2 拉整份 20MB。
- binding 拷貝已是下限(Uint8Array → linear memory 一次,`takumi-wasm/src/renderer.rs:47`),沒有引擎側可省的刀。
- Worker bundle 上限(free 3MB gz)使 bundle 內嵌字型不可行,`from_static` 在這條路線上用不到。

競品在這件事上同樣空白:satori 的官方建議是 Google Fonts css2 `text=` 技巧,私有字型一律「自己先跑 pyftsubset」。沒有人把這步做成產品。

## 建議

### 1. Build-time 切片工具(主要交付)

`@takumi-rs/helpers` 出一個 Node 端 build script(或獨立 CLI):

```
takumi-font-slice NotoSansTC-Regular.ttf --out ./public/fonts
# 產出 slices/*.woff2 + manifest.json
```

- manifest 就是現有 `FontSubset[]` 形狀(`name`/`subsetOf`/`subsetRank`/`ranges`/`key`,`data` 換成 `url`)。
- runtime 一行接上:`fonts: await fontsFromManifest("https://cdn/fonts/manifest.json")`。之後 `subsetFonts` 與惰性 `data()` 全自動,Workers 內每次 render 只抓用到的 slice。
- 切片實作候選:`harfbuzzjs`(hb-subset 的 wasm 發行版,npm 現成,Node build 環境跑,不進 Worker bundle)。API 細節未實測,落地前先 spike。
- CJK 切法直接抄 Google:對照字型的 script,用 css2 對 Noto 同 script 字型回的 `unicode-range` 表當預設切片(vendored 成 JSON),拉丁字型按 latin/latin-ext 兩刀。
- 不要用 takumi-pdf vendored 的 subsetter:它吃 `GlyphRemapper`(gid 層,PDF embed 用),做碼點層 subset 還缺 cmap closure 與 GSUB closure,等於重寫 hb-subset。

### 2. 文件配方(立刻可做,零 code)

在 docs 加一頁「Private fonts on Workers」:

- 教 `hb-subset`/`pyftsubset` 切片 + 手寫 manifest 的最小範例。
- 教 R2 / Static Assets 放置與 Cache API。
- 明確警告:整份 20MB TTF 直接塞 `fonts` 的記憶體後果。

工具落地前這頁就能止血。工具落地後改成一行命令。

### 3. 不做

- **Runtime 動態 subset(Worker 內現切)**:切之前還是得把整份字型拉進記憶體,高水位問題原封不動,只省後續 render。複雜度換不到主要痛點。
- **JS 端收緊 subset 過濾(看 family)**:memory 已有判決 — JS 看不到 cascade,猜錯 = `MissingGlyphs` render 失敗(#1186)。多抓只是頻寬,少抓是壞掉。
- **HTTP Range 惰性讀表**:sfnt 的 cmap/glyf offset 結構不配合,工程量科幻。
- **引擎側「字型選擇搬進 Rust」**:維持既有判決,重評條件不變(多 family、每份文件只用一兩個的 CMS 場景)。

## 來源

- `takumi-core/src/resources/font.rs`(subset group、`from_shared`、content-hash blob id)
- `takumi-helpers/src/fonts.ts`(`googleFonts`、`subsetFonts`、`FontSubset`)
- `takumi-helpers/src/renderer.ts`(`FontRegistry`、`prepareRenderInput`)
- `takumi-wasm/src/renderer.rs`(binding 拷貝路徑)
- [Workers limits](https://developers.cloudflare.com/workers/platform/limits/)(128MB isolate、bundle 上限)
- [harfbuzzjs](https://github.com/harfbuzz/harfbuzzjs)(hb-subset wasm,未實測)
