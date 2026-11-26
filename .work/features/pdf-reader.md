# PDF 阅读器（pdf.js 整页 ＋ 思源式批注）

> 2026-11-26。用户原话：基础功能全的 PDF 阅读器，两种预览方式可切换，批注只要思源式双链（不要文件内嵌）；下载/导出时可选是否带批注。

## 结论先行（与思源/Joplin 的关系）

- 思源 = pdf.js（vendored）＋自写样式 ＋ **原生批注层（块双链，不写文件）**；pdf.js 自带编辑功能被藏起。
- Joplin = `@joplin/pdf-viewer`（pdf.js viewer fork），iframe 纯阅读，无批注。
- pergame：引擎同源（pdf.js 整页 viewer），批注学思源（库行＋双链），下载烧录用 pdf-lib。

## 阅读器本体

- `frontend/public/pdfjs/`：mozilla/pdf.js **v6.5.438** tag，`npx gulp generic` 产物，布局照搬 `build/generic/{web,build}`。
- 裁剪：测试 PDF / debugger / sourcemap / locale（只留 en-US、zh-CN、zh-TW，`locale.json` 同步裁），约 8MB。升级：重跑构建＋重裁，来源记 `VERSION.txt`。
- 挂载：`/pdfjs/web/viewer.html?file=<同源资源URL>[#page=N]`，cookie 鉴权（同源 fetch 默认带 cookie）。
- viewer 自带 CSP（`style-src 'self'`）：**内联 `<style>` 被杀**，静态规则走 `web/pergame-annots.css` 经 `<link>` 注入；`el.style.*` 的 CSSOM 写不受限（实测）。
- 自带标注编辑器必须死：工具栏按钮 CSS 藏（`#editorModeButtons`），`annotationEditorMode` 设 DISABLE 会抛（setter 只认进入模式），改事件卫栏——`annotationeditormodechanged` 非 NONE 即打回（NONE=0）。

## 批注模型（只做思源式）

- 存储：`pdf_annotations`（migration 0016）：resource_id、page（1-based）、quads（PDF 默认用户空间 points，旋转无关，1..64 个）、quote（2000）、comment（5000）、color（0..7，`PDF_ANNOT_COLORS`）。
- 文件恒干净：批注永不写回存储字节；`annotation_refs` 由文档内 `#annotation:<uuid>` reconcile（ride `reconcile_refs`＋restore backfill，与 `node_refs` 同构）。
- 双链：文档→PDF（`#annotation:` 链接开面板定位＋闪烁）；PDF→文档（悬浮 title 看 comment，点开编辑器看 `refs` 反链列表＋跳文档）。
- 创建只做**划选**（单页，多页拒绝），颜色 8 选。墨迹/图形/表单以后再说。
- API：`GET/POST /resources/:id/annotations`（Read/Update 门，create 限 PDF）、`GET/PATCH/DELETE /annotations/:id`（读=文件 Read，写=创建者或文件 Update）、`GET /annotations/:id/refs`（来源可见性过滤）。

## 两种预览方式

- **右侧面板（默认）**：文档页右侧 560px sticky 面板，TOC 让位；Esc 关，跳文档关。普通 PDF 链接点击开面板；面板内可划选创建批注。
- **内嵌**：普通 PDF 链接就地变内嵌整页 viewer（620px 高）；`![pdf]` 无论何模式都是内嵌整页 viewer。
- 内嵌 viewer 的覆盖层只读（悬浮看 comment，点击进右侧面板编辑）。
- 模式：全局默认 `pdfPreviewDefault`（设置/其它）；单篇覆盖 `node.metadata.pdfPreview`；页眉一键 side⇄inline（显式覆盖，Metadata 弹窗可回跟随默认）。
- 轻量 `PDFViewer` 组件保留（CDN 预览/孤儿预览仍用），文档内嵌已切整页 viewer；`ResourcePreviewModal` 已删（被面板取代）。

## 下载/导出

- 面板内下载键：拉原字节＋批注行，pdf-lib 烧成标准 `/Highlight`（QuadPoints＋Contents＋颜色＋Print 标记）副本下载（`*-annotated.pdf`），原文件不动。
- viewer 自带下载/打印走原文件（= 不带批注），正好是“不带批注”选项。
- 快照备份含库=批注已结构化备份，不烧录。

## 覆盖层实现要点（`helpers/pdf-annots.ts`）

- 等 viewer 就绪（poll `PDFViewerApplication.pdfViewer.pagesCount`），同源直调 API。
- 绘制：每页 `.page` 内插 `.pg-ann-overlay` 层，quads 经 `convertToViewportPoint` 转 CSS px；zoom/rotate/render 事件重绘。
- 划选：textLayer range rects 转 `convertToPdfPoint`，超 64/跨页/空拒绝。
- 跳转：`scrollPageIntoView`＋CSS 闪烁。
- v6 无 `convertToViewportRectangle`（只有 Point 版），别踩。
