# Feature Spec — Markdown 功能对齐(媒体播放器内置化 + Joplin 特性集)

> 日期:2026-11-07。跟踪:issue #6(媒体播放器)/ issue #7(Joplin md 特性集)。需求源:用户自研 joplin-plugin-video-md 内置化 + Joplin Markdown 设置页截图全清单。
> 现状盘点基准:`frontend/app/helpers/markdown/`(markdown-it 管线,17 个插件模块)。

## 一、现状(盘点实证,勿重复建设)

已支持:数学(katex)、Mermaid、`==mark==`、脚注、`~sub~`/`^sup^`、emoji、anchor + TOC 组件、checkbox(交互式)、containers/cards/colors、counters(图/表自动编号)、内部链接插件、tooltip、代码复制、linkify、raw html、highlight.js。

## 二、媒体播放器内置化(issue #6,源自 video-md)

用户自研 [joplin-plugin-video-md](https://github.com/kamasylvia/joplin-plugin-video-md):markdown-it `image` renderer override——`![alt](url.mp4|webm|ogv|mov)` → `<video controls preload="metadata" style="max-width:100%">`,保留 query string 与 `#t=start,end`,其余回落原 image 规则。

内置化方案(同一 renderer override 思路,新建 `app/helpers/markdown/media.ts`):

| 语法 | 渲染 |
|---|---|
| `![alt](url.mp4/webm/ogv/mov)`(大小写不敏感) | `<video controls preload="metadata">`,alt 为回落文本;`#t=` 保留;**同时支持本仓资源节点 CDN URL** |
| `![alt](url.mp3/wav/ogg/flac/aac)` | `<audio controls preload="metadata">` |
| `![alt](<资源节点 PDF>.pdf)` | 复用现有 `PDFViewer.vue` 内嵌渲染 |

估时:0.5-1 天。CSS `max-width:100%` + 不自动播放,与 video-md 行为一致。

## 三、Joplin 特性集差距(issue #7)

| Joplin 特性 | 状态 | 行动 | 估时 |
|---|---|---|---|
| 软换行(breaks) | ❌ 未开 | markdown-it `breaks: true`,**做成用户设置开关**(默认开,对齐 Joplin;注意与硬换行语义冲突提示) | 0.5 天 |
| Typographer(智能引号/破折号) | ❌ 未开 | `typographer: true` + 设置开关 | 0.5 天 |
| Linkify | ✅ | — | — |
| ++insert++(`<ins>`) | ❌(现有 `__x__`→`<u>` 是另一语法) | `markdown-it-ins`,与 underline 插件并存 | 0.5 天 |
| MultiMarkdown 表格(跨行/跨列/段内表格) | ❌(仅标准表格) | `markdown-it-multimd-table` | 0.5 天 |
| deflist 定义列表 | ❌ | `markdown-it-deflist` | 0.5 天 |
| 缩写 abbr | ❌ | `markdown-it-abbr` | 0.5 天 |
| External embeds(YouTube 等) | ❌ | URL → 沙箱 iframe(YouTube/Bilibili/Vimeo 白名单模板,懒加载 consent-to-play 壳,隐私友好) | 1 天 |
| ABC 记谱法 | ❌ | abcjs 集成(` ```abc ` 代码块渲染)+ 设置项(对齐 Joplin 的 abcjs options JSON5) | 1 天 |
| Fountain 剧本语法 | ❌ | markdown-it-fountain 生态质量待验;**后置**,用户需求确认后再做 | 1-2 天(待验) |
| PDF 查看器 | ✅ 资源级 | md 内嵌并入 #6 | — |
| file:// URL | n/a | web 应用:CDN 直链天然支持,不做 file:// | — |

小计:#6 ≈ 0.5-1 天;#7 ≈ 5-6 天(不含 Fountain 待验项)。

## 四、实施时机

全部为**纯前端 markdown-it 层改动,不依赖 Rust 重写**(重写不动 frontend)——现在做不返工。时机待用户裁决:立即做 vs 排进批次。

## 五、开放问题

1. breaks/typographer 默认值:对齐 Joplin 全开,还是保守默认关?(建议:breaks 开——自托管用户多为 md 新手;typographer 关——可能破坏代码/URL 内引号)
2. External embeds 白名单是否可配置(实例管理员级)?
3. Fountain 是否真需要(用户场景里出现频率存疑)
