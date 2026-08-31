# takumi-js 與 takumi-pdf 下一步研究

研究日期：2026-08-30

## 結論

兩個套件不該採用同一份 roadmap。

- `takumi-js` 已有明確使用量。下一階段應優先守住相容性、bundle 體積與 v3 升級路徑。
- `takumi-pdf` 還在補齊文件產品的基本能力。下一個主要功能應是有語意的表格，其次才是更多 PDF 選項。
- 兩者都應先修正文件。現有文件描述落後於已發布功能，甚至互相矛盾。

## 現況

| 指標                            |       takumi-js |      takumi-pdf |
| ------------------------------- | --------------: | --------------: |
| 最新 npm 版本                   |          2.13.2 |          0.12.2 |
| 2026-07-30 至 2026-08-28 下載量 |         871,161 |          44,504 |
| 發布版 wasm 原始大小            | 3,791,164 bytes | 4,153,528 bytes |
| 發布版 wasm gzip -9             | 1,594,929 bytes | 1,723,362 bytes |
| 發布版 wasm Brotli q11          | 1,228,034 bytes | 1,321,477 bytes |

下載量來自 npm 的 [`takumi-js`](https://api.npmjs.org/downloads/point/2026-07-30:2026-08-28/takumi-js) 與 [`takumi-pdf`](https://api.npmjs.org/downloads/point/2026-07-30:2026-08-28/takumi-pdf) API。wasm 大小以 npm 發布包 `@takumi-rs/wasm@2.13.2` 與 `takumi-pdf@0.12.2` 重壓測得。

這個差距會影響排序。`takumi-js` 的變更會碰到較多既有使用者。`takumi-pdf` 則適合繼續補足遷移阻力最大的能力。

## 先做：校正文件與 issue

目前至少有四個可直接驗證的落差：

- [Reports 頁面](../../docs/content/docs/pdf/reports.mdx)仍寫著 `<table>` 不受支援，也說表頭不會跨頁重複。
- [Tables 頁面](../../docs/content/docs/tables.mdx)仍寫著 `border-collapse` 尚未實作。它已在 [PR #1374](https://github.com/kane50613/takumi/pull/1374) 合併。
- [PDF/A 頁面](../../docs/content/docs/pdf/pdf-a.mdx)把表格「無版面支援」與「無結構標籤」混在一起。實際缺口是後者。
- [PDF 比較頁](../../docs/content/docs/pdf/comparison.mdx)仍以 `takumi-pdf 0.4` 為基準。現在發布版是 0.12.2。

GitHub 的 [`<table>` support` #1212](https://github.com/kane50613/takumi/issues/1212) 也仍開著。問題中要求的表格版面、邊框合併與重複表頭已經發布。

### 建議交付

- 更新上述頁面。
- 在 #1212 回覆目前支援範圍後關閉。
- 以 0.12.2 重跑 PDF benchmark。
- 將 benchmark 版本與 wasm 大小由腳本產生，避免手動數字再次過期。

完成標準：文件不再把已發布能力列為 unsupported。比較頁的版本與 npm latest 相同。

## takumi-js

### P0：定義 v3，而不是繼續累積相容層

`stylesheets` 已被 `css` 取代。`keyframes` 也預告在 v3 移除。這些舊入口仍存在於 facade、N-API 與 wasm binding：

- [`takumi-js/src/render.ts`](../../takumi-js/src/render.ts)
- [`takumi-napi/src/renderer.rs`](../../takumi-napi/src/renderer.rs)
- [`takumi-wasm/src/model.rs`](../../takumi-wasm/src/model.rs)
- [`takumi-pdf-js/src/export.ts`](../../takumi-pdf-js/src/export.ts)

不要只在 `takumi-js` 刪除欄位。底層 bindings 若保留舊型別，公開 API 仍會分裂。

### 建議範圍

- 寫一頁 v3 migration guide。
- 在所有 JS 入口移除 `stylesheets`。
- 移除獨立 `keyframes` 選項。動畫只透過 `css` 的 `{ keyframes, steps }`。
- 保留 v2 的最後一個 minor 版本作為完整警告期。
- 為 v2 與 v3 各做一個編譯 fixture。v2 範例應收到警告。v3 範例應在 typecheck 時拒絕舊欄位。

完成標準：facade、N-API、wasm 與 PDF 的公開型別只剩一種 CSS 輸入模型。

### P0：把 runtime 相容性當成產品契約

`takumi-js` 會依 import conditions 在 native 與 wasm 間切換。[package exports](../../takumi-js/package.json)同時處理 Node、Bun、Workers、瀏覽器與 `unwasm`。近期 changelog 也多次修正 Turbopack、Vite、webpack、Nitro 與 WebContainer 的解析問題。

現有 [`bundlers.test.ts`](../../takumi-js/tests/bundlers.test.ts) 主要以 Bun bundler 模擬 condition。它能抓到 export-map 錯誤，卻不是實際框架建置。

### 建議範圍

建立小型、可發布的 smoke matrix：

| 環境                 | 最低驗證                    |
| -------------------- | --------------------------- |
| Node ESM / CJS       | native backend 可載入並渲染 |
| Bun                  | native backend 可載入並渲染 |
| Vite browser         | wasm 成為可部署 asset       |
| webpack Node         | `.node` 不進入 bundle       |
| Next.js Node / Edge  | 各自選到 native / wasm      |
| Nitro / WebContainer | `unwasm` 路徑可渲染         |
| Cloudflare Workers   | bundle 可部署並完成一次渲染 |

完成標準：每個宣稱支援的 runtime 都有實際 build 加 render，不只檢查輸出的 import 字串。

### P1：守住 wasm 體積

[官方比較頁](../../docs/content/docs/comparison-to-satori.mdx)已把 wasm 大小列為弱點。發布版目前是 1.59 MB gzip。Cloudflare Workers 免費方案的 [Worker size 上限是 3 MB](https://developers.cloudflare.com/workers/platform/limits/)。引擎已吃掉超過一半額度。

下一步不一定是拆 wasm。先讓體積變化可見：

- 對 `takumi-js` 加上與 PDF 相同的 raw、gzip、Brotli 報表。
- 設定 step-change ceiling，不以每次微幅成長阻擋開發。
- 用 `twiggy` 或 `wasm-tools` 保存前 20 大符號與 crate 貢獻。
- 每季做一次可移除功能實驗。先看 image decoder、動畫 encoder 與 SVG 路徑，不先猜哪一塊最大。

完成標準：每個 PR 都看得到 wasm 變化。一次新增功能不能在沒有說明下增加超過 5%。

### P1：把相容性測試移到輸出層

`takumi-js` 的定位包含 Satori 與 `next/og` 遷移。[比較頁](../../docs/content/docs/comparison-to-satori.mdx)宣稱大部分情況只要交換 import。這項承諾需要真實模板保護。

建立一組 migration corpus：

- 從官方 Satori examples 與本 repo migration guide 取模板。
- 同時用舊 renderer 與 Takumi 產生輸出。
- 針對版面幾何、換行、font fallback 與 emoji 建 golden。
- 允許已記錄的差異。新差異必須有 changeset。

這比再增加一批單一 CSS property 測試更接近使用者看到的破壞。

## takumi-pdf

### P0：加入表格結構標籤

表格版面已經能處理 `<table>`、跨欄、邊框合併與重複 `<thead>`。但 [`tags.rs`](../../takumi-pdf/src/tags.rs)沒有把 `table`、`tr`、`th` 或 `td` 對應為 PDF structure elements。[PDF/A 文件](../../docs/content/docs/pdf/pdf-a.mdx)也明確記錄這個可及性缺口。

這是最合理的下一個 PDF 功能。報表與發票都常用表格。現在的 PDF 可以看，卻不能讓 screen reader 依列與欄導覽。

### 建議切分

- `table` → `Table`
- `thead`、`tbody`、`tfoot` → `THead`、`TBody`、`TFoot`
- `tr` → `TR`
- `th` → `TH`，帶 `Scope` 或 header association
- `td` → `TD`
- `caption` → `Caption`
- `rowspan`、`colspan` 寫入 table attributes
- 重複表頭的繪圖內容仍只對應一份邏輯結構

驗證不能只靠「veraPDF 通過」。加入結構樹 golden，並用至少一個 screen reader 或 PAC 檢查實際導覽順序。

完成標準：一份跨頁表格可依列、欄與表頭關係導覽。`tagged: "ua1"` 與 `"ua2"` 都通過。

### P1：量測並處理 Node event-loop 阻塞

`PdfRenderer.render()` 是 async API，但資源準備完成後會直接呼叫同步 wasm binding。來源可在 [`takumi-pdf-js/src/export.ts`](../../takumi-pdf-js/src/export.ts)與 [`takumi-pdf-wasm/src/lib.rs`](../../takumi-pdf-wasm/src/lib.rs)看到。大文件會在同一條 JS thread 完成 layout、subsetting 與 serialization。

這不代表現在一定要做 native binding。先量測：

- 1、10、100 頁文件的 event-loop delay。
- 單一 renderer 的循序 throughput。
- 多個 worker thread 的 throughput 與記憶體。
- native Rust backend 對 wasm 的差距。

若 100 頁文件造成超過 50 ms 的連續阻塞，提供 Node worker entry。只有在 worker 的複製與啟動成本仍太高時，再做 PDF N-API backend。

完成標準：文件說明併發模型。Node 使用者有一條不阻塞 request thread 的正式路徑。

### P1：加入 page ranges

[Puppeteer migration map](../../docs/content/docs/pdf/from-puppeteer.mdx)列出的缺口中，`pageRanges` 最符合 PDF 工作流。使用者可先渲染整份文件，再輸出指定頁面，適合預覽、重印與分段下載。

建議 API 使用結構化資料，不直接複製 Puppeteer 的字串 grammar：

```ts
render(document, {
  pages: [1, { from: 4, to: 8 }, 12],
});
```

分頁仍須先跑完整份文件，因為 `totalPages` 與 target counters 依賴完整頁數。只在 composition 與 serialization 階段略過未選頁面。

完成標準：頁碼、目錄 target 與 outline 在抽取後仍指向正確頁面。無效範圍回傳具體錯誤。

### P2：補齊長文排版控制

[`from-react-pdf` migration guide](../../docs/content/docs/pdf/from-react-pdf.mdx)仍有兩個長文缺口：hyphenation callback 與 `minPresenceAhead`。

排序建議：

- 先做 heading keep-with-next。它直接改善報告中頁尾孤立標題。
- 再評估 hyphenation dictionary 或 callback。多語系斷詞的 API 與可重現性成本較高。

不要先做 `<PDFViewer>`。瀏覽器已能用 Blob URL 或 iframe 顯示 `Uint8Array`。它不是 renderer 核心能力。

### P2：等待原生 table layout，不再擴大平行實作

目前表格會先轉成 grid。[Tables 文件](../../docs/content/docs/tables.mdx)已明說這是近似實作。Taffy 的原生 CSS table layout 仍在 [DioxusLabs/taffy#1094](https://github.com/DioxusLabs/taffy/pull/1094) 進行中。

在 upstream 合併前，只修 correctness bug 與有使用者案例的差異。不要再自行重寫完整 column distribution。保留 differential fixtures，等 upstream 可用後比較 golden，再一次切換。

## 共用工程順序

以下順序減少重工：

| 階段 | 交付                                          |
| ---- | --------------------------------------------- |
| 1    | 修文件、關閉已完成 issue、重跑 benchmark      |
| 2    | 鎖定 v3 API 與 runtime smoke matrix           |
| 3    | PDF table semantics                           |
| 4    | wasm 體積報表與 PDF event-loop benchmark      |
| 5    | 根據量測選 worker entry 或 native PDF binding |
| 6    | page ranges 與長文排版控制                    |

CSS、layout、文字 shaping 與 scene painting 仍應先落在共用 core。PDF 專屬工作只處理 pagination、structure、interactivity 與 serialization。`takumi-pdf` 本身也說明它會走與 SVG backend 相同的 scene，見 [`takumi-pdf/src/lib.rs`](../../takumi-pdf/src/lib.rs)。

## 暫時不做

- 不為了 roadmap 數量追逐更多 CSS property。先由 migration corpus 找到實際差異。
- 不重寫完整 table algorithm。等待 Taffy upstream。
- 不先做 PDF streaming。頁數、頁尾計數與 target counters 需要全文件資訊，現有架構無法真正邊分頁邊完成輸出。
- 不把 PDF 模糊濾鏡列為近期工作。PDF 沒有直接等價 primitive，目前清楚拒絕比靜默 rasterize 更可靠。

## 來源

- [takumi-js package](../../takumi-js/package.json)
- [takumi-pdf package](../../takumi-pdf-js/package.json)
- [takumi-js changelog](../../takumi-js/CHANGELOG.md)
- [takumi-pdf changelog](../../takumi-pdf/CHANGELOG.md)
- [Takumi 與 Satori 比較](../../docs/content/docs/comparison-to-satori.mdx)
- [PDF renderer 比較](../../docs/content/docs/pdf/comparison.mdx)
- [PDF pagination](../../docs/content/docs/pdf/pagination.mdx)
- [PDF/A 與 PDF/UA](../../docs/content/docs/pdf/pdf-a.mdx)
- [Cloudflare Workers limits](https://developers.cloudflare.com/workers/platform/limits/)
- [Taffy CSS table layout PR](https://github.com/DioxusLabs/taffy/pull/1094)
