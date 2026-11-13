# Pergame 插件系统设计提案

> 日期:2026-11-07,状态:**已批准**(2026-11-07 用户裁决;tracking issue #8)。§9 开放问题按预填建议定为:①插件读写权限跟随 UI 权限,不提权 ②v1 不做 iframe 隔离(v2 备选)③市场/签名 = v2,v1 管理员手动装 ④`net.fetch` 限方式与大小。
> 输入:`.work/obsidian-compat/RESEARCH.md`(ob 生态盘点教训)+ 本仓架构(Nuxt 3 + CM6 + markdown-it / Rust axum + pg)。

## 0. 设计原则(来自 ob 教训的逆向)

| ob 的痛点 | 我们的原则 |
|---|---|
| API 无版本承诺,宿主升级插件碎 | **`apiVersion` 硬契约**:宿主承诺快照内不破坏,弃用走双版本过渡期 |
| 插件全能,无权限边界 | **能力声明(capabilities)+ 用户同意**:插件声明要什么,用户装的时候批 |
| CJS main.js(Electron 时代产物) | **原生 ESM**,浏览器直载,无 loader 魔法 |
| 生态"官方 API + 事实依赖"两层皮 | **单向狗粮**:官方功能(index/dataview 块)走同一插件 API,API 不诚实立刻暴露 |
| desktop-only 类能力混在主 API | **能力分层 + 运行环境探测**,插件按能力存在性编程,不猜平台 |

## 1. 范围(v1 非目标先行)

**v1 = 前端插件**(浏览器内,与 ob 同威胁模型:插件 = 用户主动安装的可信代码)。

非目标(v1):后端插件(服务端代码执行)、跨实例市场、移动壳适配、插件间通信协议。

理由:后端是 Rust 单二进制 + 数据主权面,不开放任意代码;后端扩展需求将来走声明式配置(webhook/automation),不走代码插件。

## 2. 插件形态

```
my-plugin/
  manifest.json    # 元数据 + 能力声明 + apiVersion
  main.mjs         # ESM 入口(单文件,构建产物)
  styles.css?      # 可选,作用域前缀强制 .plugin-<id>
  README.md?
```

```jsonc
// manifest.json
{
  "id": "word-count",
  "name": "Word Count",
  "version": "0.1.0",
  "apiVersion": "1",                    // 宿主契约版本,主版本不同 = 拒载
  "entry": "main.mjs",
  "capabilities": [                      // 装时用户逐项批准
    "nodes:read",                        //   数据面
    "ui",                                //   注册命令/视图/设置页
    "markdown:render",                   //   md 渲染钩子
    "net:api.example.com"                //   网络出口白名单(逐域)
  ],
  "minHostVersion": "8.15.0"
}
```

分发:实例管理员上传 zip → 存 pg(插件表)+ 产物进 S3 `plugins/`;`/plugins/<id>/main.mjs` 服务;dev 模式 = 本地 origin 白名单 + 动态 import + HMR。市场 = 远期。

## 3. API 面(四层,`import { ... } from "pergame"`)

```ts
// 类型包 @pergame/plugin(独立 npm,含完整 d.ts —— 学 ob 的 d.ts 做法,但天生分层)
export abstract class Plugin {
  static manifest: PluginManifest;
  onload(ctx: PluginContext): Promise<void>;
  onunload?(): Promise<void>;          // 必须可逆:注册即登记,unload 全回收
}

export interface PluginContext {
  api: { version: 1; host: HostInfo };

  // L1 数据 —— 类型化 REST 客户端,身份 = 当前登录用户,权限天然受 actor 约束
  nodes: NodeClient;                    // get/list_children/create/update/append/move/delete
  resources: ResourceClient;            // upload(file, parentId?) -> {url}(multipart,复用现有上传端点)
  search: SearchClient;                 // 全文搜索 + dataview query 端点(#5)
  user: { id; settings: KVStore };      // 插件私有 KV(后端 user_settings JSONB 隔离命名空间)

  // L2 UI —— 声明式注册,Nuxt 组件映射
  ui: {
    command(id: string, title: string, cb: () => void): Disposable;
    ribbon(icon: string, title: string, cb: () => void): Disposable;
    notice(msg: string, opts?): void;
    modal(component: PluginModal): void;
    registerSettingsTab(tab: SettingsTab): Disposable;   // 声明式 schema,宿主渲染
    registerView(type: string, view: PluginView): Disposable;  // v1: 固定槽位(侧栏/tab 页),非自由 dock
  };

  // L3 markdown/编辑器 —— 与现有管线同源
  markdown: {
    registerContainer(name: string, render: ContainerRender): Disposable;  // :::name(::: 与 ``` 双语法)
    registerCodeBlock(lang: string, render: CodeRender): Disposable;
    registerPostProcessor(fn: PostProcessor): Disposable;                  // 渲染后 DOM 钩子(ob 对位物)
  };
  editor: {
    registerCmExtension(ext: CMExtension): Disposable;                     // CM6 同源直通
    registerSuggest(spec: SuggestSpec): Disposable;
  };

  // 能力化网络 —— facade 级强制,非承诺
  net: { fetch(url: URL | string, init?: RequestInit): Promise<Response> }; // 策略见下
}
```

**net 策略(2026-11-07 依 ReadItLater 用例细化)**:web clipper 类插件天然需访问任意域,逐域 manifest 白名单不够用 → 三级:① manifest `net:example.com` 静态白名单;② `net:*` 通配(安装时显著警告);③ **运行时逐域询问**——首遇新域弹确认(可"记住此域",记录进用户批准表)。推荐 ③ 为默认体验,①/② 供插件自选。

**editor 事件钩子(可选增量)**:`editor.onPaste(handler)`——粘贴捕获场景(ReadItLater paste 对话框由命令触发,非必需,但属 L3 自然延伸,PS-C 顺带)。

要点:
- **一切注册返回 `Disposable`** —— unload 语义强制,杜绝 ob 插件卸不干净的普遍病。
- **`pergame` 模块 = 唯一宿主入口**(loader 以此包名注入),插件不直接 import 应用内部模块。
- 视图模型 v1 用**固定槽位**(侧栏面板/tab 内容区),不做 ob 的自由 dock(盘点报告 D 组:最贵最不重要)。

## 4. 加载与生命周期

```
启动 → 读已装插件清单(启用序) → 逐个 dynamic import(main.mjs)
     → 校验 apiVersion + capabilities(安装时已批,载入时复核)
     → new Plugin() → onload(ctx)
     → 任一插件 onload 抛错:记录 + 禁用该插件,不影响其他(隔离原则)
卸载/停用 → onunload → Disposable 全回收 → 移除 styles → 模块引用释放
```

- **沙箱策略**:同 realm(与 ob 相同威胁模型,能力声明是知情同意不是安全边界);未来可选 `sandbox` manifest 字段 → iframe 隔离模式(纯 UI 插件),v2。
- 样式强制前缀 `.plugin-<id>`(构建期 PostCSS 重写),防主题互踩。

## 5. 权限模型(与 ob 的最大差异点)

- 安装时:manifest.capabilities 逐项展示 → 用户勾选批准 → 存 pg(可后改)
- 运行时:facade 强制(nodes 客户端本来就 actor 域内;`net.fetch` 白名单外拒;settings 只见自己命名空间)
- 审计:敏感能力调用记 logs 表(复用现有 log 基建)

## 6. 狗粮计划(证明 API 诚实)

| 功能 | 走插件 API 的部分 |
|---|---|
| #4 目录索引块 | `markdown.registerCodeBlock("index", ...)` |
| #5 dataview 块 | 同上 + `search` 客户端 |
| drawio 编辑器 | `registerCodeBlock("drawio", ...)`(现 builtin,迁移作试点) |
| emoji snippets / callout 容器 | `registerContainer` |

首批官方插件(练手 + 生态种子):word count、recent files、kanban 视图、templater 简版(模板变量替换)。

**生态验证头牌(2026-11-07 定)**:移植用户自研 [SiYuan-ReadItLater](https://github.com/kamasylvia/SiYuan-ReadItLater)(~5.7k 行,15+ 站点 parser,readability+turndown 纯 JS 可全复用;仅 repository 层思源 kernel API → `ctx.nodes/resources` 与 Dialog→modal 需改,≈3-5 天)。它已实测只用 Dialog/Notice/command/设置页(未触碰 dock/protyle)→ 反向印证 §1 砍工作区壳的决策,并驱动了 net 三级策略、`nodes.append`、`resources.upload` 三个 API 增量。

## 7. 开发者体验

- `@pergame/plugin` npm 包:类型 + `create-pergame-plugin` 脚手架 + 本地 dev server(连真实实例,`PERGAME_DEV_PLUGINS=http://localhost:5173` 热载)
- 文档:API reference(d.ts 生成)+ 3 个 from-zero 教程(命令插件 / 视图插件 / md 块插件)
- 测试:`@pergame/plugin-test` 内存宿主 mock(mock ctx,无浏览器)

## 8. 实施路线(进 Rust 重写批次的排位)

| 阶段 | 内容 | 估时 | 依赖 |
|---|---|---|---|
| PS-A 内核 | loader + 生命周期 + manifest 校验 + L1 数据客户端 | 1 周 | 重写 P1 完成 |
| PS-B UI 层 | L2 声明式注册 + 设置页 + 固定槽位视图(**命令/ribbon ✅2026-11-13**;设置页/视图待做) | 1 周 | PS-A |
| PS-C md/编辑器 | L3 容器/代码块/post-processor/CM6 直通 | 3-5 天 | PS-B;与 #4/#5 合流实现 |
| PS-D 权限与分发 | capabilities 同意 UI + 上传/存储 + 审计 | 3-5 天 | PS-B |
| PS-E 狗粮 | 官方插件 4 个 + drawio 迁移 | 1 周 | PS-C |

合计 ≈ 3.5-4.5 周,**排在重写主体(P1-P5)之后**,#3 MCP / #4 index / #5 dataview 与 PS-C 合流。

## 9. 开放问题(批准时定)

1. 插件是否允许读写"其他用户共享给我的"节点?→ 建议:跟随 UI 权限,无提权
2. v1 是否要"隔离模式"(iframe)?→ 建议:不要,记 v2
3. 插件市场/签名 → v2;v1 管理员手动装
4. `net.fetch` 是否限方式(GET/POST)+ 大小 → 建议限

## 10. 实施记录

### PS-D 分发(2026-11-14 ✅)

- **表**:`0011_plugins.sql` —— `plugins`(实例级安装,slug 主键,artifact_prefix 指向 S3)、`plugin_grants`(每用户能力批准)、`plugin_kv`(每用户每插件键值,比塞 `user_settings` 更好删/更好查)。
- **安装**:`POST /api/admin/plugins`(multipart zip;≤4 MiB、≤64 条目、单文件 ≤2 MiB、路径逐段校验、可选外层目录自动剥)。manifest 由 `plugins_manifest::validate` 服务端校验(与前端同一套规则:slug/semver/主版本/entry/能力白名单/IP 字面量拒绝)。产物写 `plugins/<id>/<version>/`,替换版本时清旧前缀。
- **服务**:`GET /plugins/:id/entry`(text/javascript,同源,`import()` 自带 cookie)与 `GET /plugins/:id/asset/*`。
- **同意**:`GET /api/plugins` 返回已启用插件 + 调用者已批能力;loader 只加载 `granted` 非空的插件,并把 granted 传给 ctx(网络门禁按批准集算)。`PUT /plugins/:id/grants` 过滤掉 manifest 未声明的项。
- **设置**:`/plugins/:id/kv` 读写调用者命名空间(键 ≤128、值 ≤64 KiB JSON);卸载/删号级联。
- **审计**:install/enable/uninstall/grant/proxy 全部写 `logs`。
- **net.proxy**:后端代取(GET/POST、15s、8 MiB、域名按能力白名单),`ensure_public_target` 解析 DNS 后拒绝私网/环回/链路本地/CGNAT/IPv4-mapped;这是剪藏替代 camoufox 的通道(DESIGN §11)。
- **UI**:设置页 `?p=plugins` —— 管理员安装 zip、启停、卸载、勾选能力;普通用户看到已装插件并批准能力。能力变更会卸载并重载该插件。

### PS-C markdown/编辑器钩子(2026-11-14 ✅)

- `ctx.markdown.{registerCodeBlock,registerContainer,registerPostProcessor}`、`ctx.editor.registerCmExtension`;全部返回 `Disposable`,随插件 bag 回收。
- 注册表放 `globalThis`(bundler 可能给出两个模块实例,双注册表 = 插件注册进没人读的表;真浏览器验证时踩到,见 LESSONS)。
- 编译器缓存键含 hook 版本,注册后下一次 compile 自动重建;fence 回落到宿主规则(高亮/复制按钮保留),未认领的 `:::name` 仍归内建容器。
- 首版不迁移内建块(index/dataview/mermaid),避免动已验证渲染路径。

### PS-E 官方插件(2026-11-14 ✅ 首批)

- `plugins/official/`:recent-files(命令+ribbon)、daily-note(按天建/开 + 设置存父目录)、web-clipper(`net.proxy` → 正文 → 建笔记)。
- `scripts/build-plugin-packs.sh` 产出 `frontend/public/plugin-packs/*.zip`,设置页可直接安装(无需构建插件仓库)。
- ReadItLater 站点专用 parser 未移植:先用通用正文抽取,后续按需求补站点规则。

### PS-B 第一片(2026-11-13 ✅,命令与 ribbon)

- **注册表**:`services/plugins/ui.ts` —— Vue `shallowReactive` 表(`commands`/`ribbons`),注册返回 `Disposable`,id 以 `<pluginId>:` 命名空间化,重复 id 抛错(连带使该插件加载失败并回滚)。`clearPluginUi(pluginId)` 兜底清理。
- **上下文**:`ctx.ui.command(id,title,run)` / `ctx.ui.ribbon(icon,title,run)`;注册经 host 的 per-plugin bag 追踪,`unload` 无需插件配合即可回收。
- **命令面板**:`CommandCenter` 行类型拆成「导航(`path`)或执行(`run`)」,QuickSearchTab 新增 `Plugins` 分组(🧩 + `Plugin · <id>`),Enter/点击执行;`AdvancedSearchTab` 行为不变。
- **Dock**:`Sidebar/Dock.vue` 渲染插件 ribbon 按钮(置底组之上),点击执行回调。
- **参考插件**:demo 增 `ctx.ui.command` + `ctx.ui.ribbon`。
- **验证**:12 项断言(命名空间、重复拒绝、执行、坏插件回滚、unload 清空)+ eslint/stylelint/typecheck 全绿。
- **未做(留 PS-B 第二片)**:`registerSettingsTab`(需 PS-D 的持久化 KV 才有意义)、固定槽位视图、`ui.modal`。

### PS-A 内核(2026-11-13 ✅)

- **契约**:`frontend/app/types/plugin.ts`(`HOST_API_VERSION = 1`、`PluginManifest`、`PluginContext`、`Disposable`、L1 客户端接口)。该文件不依赖 Nuxt/Vue,内核可在 app 外加载与测试。
- **manifest 校验**(`services/plugins/manifest.ts`):id slug、semver、`apiVersion` 主版本相等、`capabilities` 白名单(`nodes:read|nodes:write|ui|markdown:render|settings` + `net:<domain>`/`net:*`)、`entry` 必须是**插件根内**的相对 `.mjs`(拒绝 `..`/绝对路径/反斜杠),一次报全部错误。
- **注册回收**(`registry.ts`):`DisposableBag` LIFO 释放,卸载后再注册立即释放(避免迟到注册泄漏)。
- **加载器**(`loader.ts`):`load(manifest, entryUrl)` / `loadFromUrl(base)`;重复 id、错误主版本、缺 default export、onload 抛错 → 记 failed 并回滚已注册资源,不影响其他插件;`unload` 先 `onunload` 再清 bag。
- **上下文**(`context.ts`):`api.version/host`、`manifest`、`nodes`(get/listChildren/create/update/remove,走同一 REST 与权限)、`resources.upload`、`search.fulltext/query`、`settings`(PS-A 内存 KV,PS-D 落 `user_settings` 命名空间)、`ui.notice`、`net.fetch`(capability 域名白名单 + 仅 GET/POST + 8 MiB 上限,响应按字节截断校验)。
- **启动**:`app/plugins/plugin-host.client.ts` 读 `NUXT_PUBLIC_DEV_PLUGINS`(逗号分隔 base URL),`useNuxtApp().$pluginHost` 暴露给 PS-B。
- **参考插件**:`frontend/public/plugins/demo/`(manifest + main.mjs,数一遍文档并 notice)。
- **验证**:25 项内核断言(manifest 拒绝矩阵/网络门禁/生命周期/隔离/KV)+ 真实 HTTP 取 manifest 与入口、真实 ESM 求值 e2e(载入 → notice → unload)。本机无浏览器(见 TODO 缺口),浏览器内交互留待有环境时补验。

## 11. 设计补充:剪藏不需要浏览器自动化(2026-11-14 用户裁决)

**背景**:ReadItLater(SiYuan 插件)剪藏知乎时依赖 camoufox MCP,这是**思源插件沙箱的限制**——插件没有任意网络出口,也没有后端代理,只能外挂一个浏览器自动化服务代抓。Pergame 的宿主形态不同,这条依赖不必继承。

**结论**:PS-E 的 ReadItLater 移植**不引入 camoufox MCP**,改为宿主提供的能力:

1. **`net.fetch`(已有,PS-A)**:manifest 声明 `net:<domain>`,插件在页面里直接 fetch。适合有 CORS 的站点(YouTube/Bilibili/Vimeo 等公开 API)。
2. **`net.proxy`(PS-D/PS-E 新增,后端代取)**:`POST /api/plugins/{id}/proxy` 由 Rust 后端代取目标 URL,突破浏览器 CORS 限制,返回正文(限大小/限域/限方法,审计入 `logs`)。这是替代 camoufox 的主力路径——绝大多数文章页是服务端可直取的 HTML。
3. **需要登录态的页面**:后端代理**不带**用户浏览器 cookie;由用户在插件设置里粘贴该站 cookie(插件 KV 存,权限 `net:<domain>` + 显式同意),或退回“复制页面 HTML/正文粘贴到剪藏框”。**不做**读取用户浏览器其它标签页的 DOM——Web 应用没有这个权限,那是扩展才有的能力。
4. **交互式挑战**(知乎的部分反爬)仍可能失败:失败即报错并提示粘贴,不静默降级,也不引入第二个自动化服务。

**记要**:凡是“思源插件必须外挂 MCP/浏览器才能做的事”,先问 Pergame 宿主是否有更直接的通道(页面 fetch / 后端代理 / 用户提供凭据);只有需要**真实浏览器交互**且**不可由用户手动完成**的场景,才考虑外部自动化,并且必须是可选依赖而非默认路径。
