# 圖表庫的 SVG 輸出與 takumi 整合

研究日期:2026-08-30
方法:npm 發布版 `@takumi-rs/core` 2.13.2 與 `takumi-pdf` 0.12.2 實測,ECharts 6 SSR 輸出。

## 結論

不做內建 chart API。能吐 SVG 字串的庫直接可用,raster 與 PDF 都實測通過。交付一頁整合文件(`docs/content/docs/charts.mdx`)。附帶發現一個 subset 掃描缺口,建議加一個小 API。

## 生態分層(能否零 DOM 吐 SVG)

| 庫                                    | 零 DOM SVG | 方式                                                                       | 備註                       |
| ------------------------------------- | ---------- | -------------------------------------------------------------------------- | -------------------------- |
| Apache ECharts                        | 是         | `init(null, null, { renderer: "svg", ssr: true })` + `renderToSVGString()` | 5.3+ 官方零依賴 SSR,首選   |
| Vega / Vega-Lite                      | 是         | `new View(parse(spec), { renderer: "none" })` + `view.toSVG()`             | bundle 肥                  |
| d3-shape / d3-scale                   | 是         | 純數學,產 `path d` 字串,自己包 JSX `<svg>`                                 | 最貼 takumi JSX,無成品樣式 |
| visx                                  | 是         | React SVG primitives + `renderToStaticMarkup`                              | 低階元件純 SVG             |
| nivo                                  | 部分       | 官方宣稱 SSR 支援                                                          | 未實測                     |
| Observable Plot                       | 否         | 需要 `document`(jsdom/linkedom)                                            | Workers 不友善             |
| Recharts                              | 否         | 3.x SSR 壞著(recharts#5997)                                                | 避開                       |
| Chart.js / uPlot / lightweight-charts | 否         | canvas-only                                                                | 出局                       |

## 實測結果(全部通過)

- **Raster(OG image)**:ECharts smooth line + 半透明 `areaStyle`、dasharray 虛線、環圈圖 label line、`LinearGradient` + `borderRadius` 柱狀圖,全部正確。
- **PDF 是真向量**:兩張 560×360 圖表的 A4 = 21KB。content stream 實測:Image XObject **0 個**、貝茲曲線 op 1,104、`/ShadingType` 2 個(gradient 走 PDF shading)、dasharray 保留。
- **三種嵌入方式都通**:inline `<svg>` 直嵌 HTML(最乾淨)、`images: { key: svgBytes }` + `<img src="key">`、base64 data URI。

## 發現的缺口

### SVG 內文字對 subset 掃描不可見

`collectCodepoints` 只走 node tree 的 text node,看不到 SVG 字串裡的 `<text>`。`googleFonts("Noto Sans TC")` 的 105 個 slice 被 `subsetFonts` 按樹內碼點過濾後,CJK 軸標籤需要的 slice 全被丟掉,label 出豆腐。「一月」偶爾存活只是恰好搭上未被濾的 slice。

**Workaround(已實測,105 → 3 slices,全部 label 正確)**:

```ts
const all = await googleFonts(["Noto Sans TC"]);
const fonts = subsetFonts({ fonts: all, source: labels.join("") }).map((f) => ({
  ...f,
  ranges: [],
}));
```

先按 label 字串預濾,再清掉 `ranges` 讓 render 內部的第二次過濾放行。

**建議修法**:render options 加 `subsetText?: string`,串進 `prepareRenderInput` 的 codepoint source(與 `LIST_MARKER_CHARACTERS`、counter 字元同一個洞的同一種補法)。一行 API,文件 workaround 即可退役。

## 來源

- [ECharts SSR handbook](https://apache.github.io/echarts-handbook/en/how-to/cross-platform/server/)(`ssr: true` + `renderToSVGString`,5.3+ 零依賴)
- [Vega View API](https://vega.github.io/vega/docs/api/view/)(`renderer: 'none'` + `toSVG()`)
- [Observable Plot #1550](https://github.com/observablehq/plot/issues/1550)(SSR 要 DOM)
- [recharts #5997](https://github.com/recharts/recharts/issues/5997)(3.x SSR 壞)
- 實測 script:session scratchpad `chartlab/`(render.ts / grad.ts / pdf.ts / inline.ts / cjk2.ts)
