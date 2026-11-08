# Obsidian 插件生态兼容性盘点 — Collectanea

> 日期:2026-11-07。产出方式:官方 `obsidian.d.ts`(obsidianmd/obsidian-api@master)实测盘点 + 生态面检索。决策门:报告读完裁决走哪条路线(见 §8)。

## 1. 结论摘要

- **完全复用 ob 插件生态 = 重写一个插件宿主**:API 面 11.5k 行 d.ts,浏览器缺 Node/Electron/工作区壳三层地基;单插件"能跑"门槛估 **4-8 周**,生态级兼容 = 无上限追赶。
- **但浏览器安全子集真实存在**:下载榜头部插件(Templater/Dataview/Excalidraw/Tasks/Advanced Tables/Calendar)全部 `isDesktopOnly: false`——纯 JS 插件是生态主体。
- **唯一已对齐的地基:编辑器同为 CodeMirror 6**(Collectanea frontend 已用)——这是插件兼容最大的幸运点。
- **建议**:M0 语法/数据兼容 + top 插件内置化(#5 已走此路);插件加载器列为可选 spike(M1 最小闭环验证),不进 roadmap 承诺。

## 2. 插件技术解剖(兼容到底要仿什么)

ob 插件 = `manifest.json` + `main.js`(CJS)+ `styles.css`,跑在宿主 app 内,依赖五层:

| 层 | 内容 | Collectanea 现状 | 差距定性 |
|---|---|---|---|
| ① `obsidian` API 模块 | 见 §3 实测 | 无 | facade 重写,大 |
| ② 宿主加载器 | Electron `require('obsidian')` 注入 | 浏览器无 CJS require | SystemJS/运行时打包,中-大 |
| ③ Node/Electron | 插件可 `require('fs')` 等 | 浏览器沙箱没有 | **天然不可行子集**(ob 自己用 `isDesktopOnly` 承认) |
| ④ Vault 抽象 | 本地磁盘 TFile + 实时 MetadataCache | REST + 服务端 DB | 可映射,Cache 实时语义要仿 |
| ⑤ 编辑器 | CodeMirror 6 | **同为 CM6** | ✅ 已对齐 |

法律面:clean-room 实现其文档化 API 做互操作 = 可;分发 obsidian runtime = 不可。

## 3. API 面实测(obsidian.d.ts @ master,2026-11)

**总量:11,539 行;`export class` 77、`export interface` 112、`export function` 47。**

按「在 Collectanea 浏览器上下文可实现性」分组(类名实测归堆,量为约数):

| 组 | 内容 | 代表类 | 量级 | 实现难度 |
|---|---|---|---|---|
| A UI 基建 | 设置页/弹窗/通知/菜单/表单组件 | Setting、Modal、Notice、Menu、ButtonComponent、TextComponent、ToggleComponent、SliderComponent、SearchComponent 等 | ~25 类 | 低-中(全部自绘,Nuxt 组件映射) |
| B Vault/元数据 | 文件抽象 + 缓存 | Vault、TFile、TFolder、DataAdapter、MetadataCache、FrontMatterCache、LinkCache、TagCache、HeadingCache、SectionCache、ListItemCache 等 | ~30 类/接口 | 中(Vault→REST;Cache 我们服务端真算,反而更准) |
| C 编辑器/渲染 | CM6 facade + md 后处理 | Editor、MarkdownView、EditorTransaction、EditorSuggest、MarkdownPostProcessor、MarkdownRenderChild、LivePreviewStateType 等 | ~20 类 | 中(CM6 同源,受控暴露;post-processor 接 markdown-it 管线) |
| D 工作区壳 | dock/leaf/split/popout | Workspace、WorkspaceLeaf/Split/Tabs/Root/Ribbon/Sidedock/Window/MobileDrawer、View/ViewState、HistoryHandler | ~15 类 | **大**(Nuxt 无 dock 体系;要么 facade 到 Tab 系统,要么砍) |
| E 平台专属 | Node/Capacitor/Electron/全局键 | FileSystemAdapter(Node fs)、CapacitorAdapter、SecretStorage、Keymap/Scope(浏览器受限)、RequestUrl* 原生语义、popout window | ~10 类 | **放弃**(isDesktopOnly 插件不接) |
| F 官方 Bases API | 数据库视图 | BasesConfigFile/View/Entry/Property/QueryResult/Formula… | 25 类 | 不仿——#5 dataview 已立项自建(定位重叠) |

**浏览器可行面占比(按类数毛估):A+B+C ≈ 75% 可实现,仅 D 壳难、E 层死路。**

## 4. 生态分层(下载榜实证)

[Obsidian Stats](https://www.obsidianstats.com/most-downloaded) 全时下载榜头部:

| 插件 | isDesktopOnly | 纯 JS 可浏览器跑 | 备注 |
|---|---|---|---|
| Excalidraw | ❌否(移动可用) | ✅ | 白板;我们已有 drawio 编辑器(定位重叠) |
| Templater | 否 | ✅ | 模板引擎;内置化候选 |
| **Dataview** | 否 | ✅ | **#5 已立项内置(DQL 子集)**;其继任者 Datacore 观望 |
| Tasks | 否 | ✅ | 任务管理;我们 md 内 checkbox 已有基础 |
| Git | **是** | ❌ Node | 桌面专属;不兼容对象 |
| Advanced Tables / Calendar / QuickAdd / Omnisearch / Style Settings | 否 | ✅ | Style Settings 依赖 CSS 变量约定 |

结论:**纯 JS 插件 = 生态大头且集中在头部**;"无 Node 子集"策略成立。但每插件的隐藏依赖(Editor 假设、metadata 实时性、CSS 变量)要逐个验收,无批次兼容保证。

## 5. 加载机制(浏览器跑 CJS main.js)

- **运行时方案**:SystemJS / `esm.sh` 风格预打包 / Service Worker 拦截 + 打包;CJS→ESM 转换成熟
- **Realm 问题**:插件要摸 CM6 Editor 实例 → 必须与主应用同 realm(不能 iframe 沙箱);同 realm = 插件拥有整页 JS 权限,安全边界 ≈ 无(与 ob 相同,可接受)
- **`require('obsidian')` 注入**:loader 把我们的 facade 模块注册为该包名即可
- 设置页/命令面板 → Nuxt 组件映射

## 6. 分级路线与工作量矩阵

| 里程碑 | 内容 | 估时 | 独立价值 |
|---|---|---|---|
| **M0 语法/数据兼容** | `[[wikilinks]]`、`> [!callout]`、properties/frontmatter、`.md` 导入导出(含附件映射) | 3-5 天 | ob 用户笔记无损迁入;纯内容生态自动复用;**无论后续走不走插件路线都值得做** |
| **M1 加载器 + API 核心** | SystemJS loader + facade(A 组 UI + B 组 Vault/Cache + C 组 post-processor)+ 生命周期 | 3-4 周 | 拿 1-2 个简单插件(Recent Files 级)试跑 |
| **M2 编辑器 facade** | CM6 Editor 受控暴露 + EditorSuggest + transaction | 1-2 周 | Advanced Tables 级插件可跑 |
| **M3 工作区壳** | dock/leaf 模拟或砍掉 | 2-4 周(可砍) | Dataview/Excalidraw 级需要 |
| 逐插件验收 | 每插件适配 + 回归 | 0.5-2 天/个 | 无承诺 |

**决策门**:M1 spike 后再看——若简单插件顺利且 API facade 未失控,可续 M2;否则止步 M0 + 内置化。

## 7. 风险表

| 风险 | 影响 | 缓解 |
|---|---|---|
| ob API 每版漂移 | facade 永久追赶 | 锁定目标 API 版本(manifest minAppVersion);只承诺"某版快照兼容" |
| 隐藏依赖(Editor 假设/实时 Cache 时序) | 单插件验收变长 | 逐插件验收制;维护兼容插件白名单 |
| 同 realm 安全 | 恶意插件整页权限 | 与 ob 同威胁模型;白名单 + 用户自担 |
| D 组壳工程失控 | M3 无底洞 | M3 默认砍,facade 报"不支持" |
| 与自研功能冲突(#5 dataview vs Dataview 插件) | 双引擎打架 | 插件加载器与内置功能互斥开关;文档声明优先内置 |

## 8. 结论与决策门

- **不建议**:以"复用 ob 插件生态"为架构目标(§1 三条理由)。
- **建议**:M0(语法/数据兼容)进重写批次或紧随其后;内置化路线延续(#5 模式);M1 加载器列为**可选 spike**(独立小批次,不承诺)。
- **待用户裁决**:
  - [ ] M0 是否并入 Rust 重写批次(建议:并入 P4/P5,前端语法层改动与重写同期)
  - [ ] M1 spike 是否排期(建议:重写交付后单独小批次)
  - [ ] 本报告结论是否回填公开 issue(建议:等路线裁决后一并)

## 附:参考

- [obsidianmd/obsidian-api(obsidian.d.ts)](https://github.com/obsidianmd/obsidian-api) — 实测对象,master @ 2026-11
- [Obsidian Stats 下载榜](https://www.obsidianstats.com/most-downloaded)
- [r/ObsidianMD 生态讨论](https://www.reddit.com/r/ObsidianMD/)
