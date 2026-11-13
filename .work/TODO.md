# TODO

> 2026-11-07 重构:双批次结构。已完成事项入 commit 历史 + AGENTS.md 决策记录,不留在本文件。

## 批次 A — 前端批次(纯前端,独立于重写;**待开工令**)

- [x] **媒体播放器内置化**(video-md → 内置,2026-11-12 完成):issue #6;`html5MediaPlugin` 补 `preload`/`#t=`/`ogv` + PDF 占位符,`ContentCompiled` 后挂载 `PDFViewer`(mermaid 同款);附带修复 Rust `sanitize_html` 对齐 Go bluemonday 策略(audio/video/object/data-*/checkbox/form/SVG/MathML)。
- [x] **Joplin md 特性集对齐** ✅(2026-11-13):issue #7;breaks(默认开)/typographer(默认关)双开关+compile 按设置缓存实例;ins/multimd-table/deflist/abbr 四插件(multimd 垫 `md.utils.assign`);embeds 白名单(YouTube-nocookie/Bilibili/Vimeo)+consent 壳+懒加载;ABC(abcjs 懒加载+abcjsOptions 设置)。浏览器实测 9/9 渲染。Fountain 后置(待需求确认)。
- [x] **M0 ob 语法兼容(缩水版 ✅2026-11-13,用户裁决:冲突/重叠不做,上游优先)**:wikilinks ❌不做(`[[...]]` 与 tooltip 插件冲突 + 上游 `#id` 双链已覆盖)、callout ❌不做(与 `:::` 容器重叠);✅做:frontmatter 解析(YAML 子集 scalar/list)+properties 表渲染+compile 入口 strip、导入改写(相对附件引用→占位→上传后换 API URL,资源先传顺序)。浏览器实测 properties 3 键 + 路径解析 7 例全过。

## 批次 B — 重写批次(Rust + pg + 功能域;**等用户开工令**)

- [x] **Rust 后端重写 + PostgreSQL** ✅(2026-11-08 开工,P1 骨架+Toasty spike PASS → 2026-11-12 已上 QNAP 生产 v0.0.11):issue #1;db 层 = Toasty + sqlx 逃逸层(11 条 `WITH RECURSIVE` 留 sqlx);全新空库;pg GIN + tsvector。
  - [x] 集成测试运行时验证:14 用例全绿(需活后端 + pg + S3,`cargo test --test integration -- --ignored --test-threads=1`),多次活环境跑通;生产 health/SSR/备份全链已核验。
- [x] **Rust 代码质量改进项** ✅(2026-11-13,审计 6 项全落地 + 集成测试修复 + 3 真 bug):①`#![deny(unsafe_code)]`(forbid 与 test scoped allow 互斥,deny 等效;生产 unsafe 0,8 处全在 `#[cfg(test)]`);②删 `croner`/`aws-config`;③拆分 6 文件全 <600(restore→apply/plan/portable/members、backup→mod/store/run/schedule、resources→upload/download/orphans、nodes→mod/trash、repo→mod/tests、routes/backup→mod/tests);④rustdoc 全覆盖 + 6 doc-tests(抽 `src/lib.rs` 纯 binary 才跑得起);⑤llvm-cov 行 13.8%→24.3%(纯逻辑多 85%+,余量需活 pg/S3)+ proptest 3 组(cron/sanitize/cookie);⑥PGO/SoA/SIMD 判 N/A。**真 bug**:环检查 `(Uuid,)`↔bigint 错配恒 500(UUIDv7 批量改残留)、还原子先父后撞外键(改两轮插入)、2FA `qr_code_url` 返 Display 摘要非 otpauth URL(改 `to_url()`)。**集成**:14/14 全绿(修 3 过时测试:target 走 probe-draft token、备份前建 target、还原传 force、sync 改快照语义)。单测 44→117。
- [x] **Per-user 配额 + 内容寻址去重** ✅(2026-11-13):①users.max_file/total_size(NULL=回落 config;config 0=无限制;admin 豁免;PATCH 双 Option 清 null;`GET /users/@me/quota`;CDN 条读服务端+无限制态;用户管理页配额输入);②空/缺省=无限制口径(TRASH_RETENTION unset/空=不清理,0=全清;SNAPSHOT_INTERVAL unset/空=关,0=非法 warn 关;全局审计余量无限制语义);③去重(`object_blobs`+`blobs/{sha256}`+refcount;upload/update/restore acquire;purge/删账户递减;末引用删对象;备份读 blob 优先;orphan 扫失引 blob)。**生产迁移实测**:30 对象全迁 `bytes_saved=0`(无重复)。单测 117→119,集成 14→16,覆盖率行 22.9%。
- [x] **MCP server** ✅(2026-11-12,issue #3 保持开启待用户确认):`rmcp 3.9` + `/api/mcp` streamable HTTP + bearer token middleware + 7 工具 + `api_tokens` 表(migration 0007)+ 设置页 `?p=apiTokens` + docs `use/mcp`。**已实测**:隔离库全链(token 生命周期/7 工具/越权 -32003/环 -32602/回收站/吊销 401)+ 只读窗口(四写 -32023、三读放行、REST 同窗 423)+ 生产库真数据(list/read/search/create/update/delete 全通)。**未做**:速率限制(P1 裁决不做);docker 镜像未含(下次 release 才上 QNAP)。
- [x] **目录索引文件** ✅(2026-11-13):issue #4;`metadata.index=true` 身份标记(服务端签发,客户端不可伪造)+ 建夹自动创建(响应 `{node,index}`)+ GET 懒重建 + 唯一索引 `0010` + 锁定(改名/移动/删除/pin 全拒,内容可编辑;MCP 三写工具同锁)+ 设置批量改名(`indexFileName`,`index_renamed` 计数,含回收站行)+ `displayIndexFiles` 可见性开关 + index 块子树渲染(过滤 index/目录优先/图标)+ 目录行跳转懒建 + 全站排序 index 置顶。单测 128 + 集成新用例 `directory_index_lifecycle`。spec = `features/directory-index.md`。
- [x] **Dataview 查询引擎** ✅(2026-11-13):issue #5;DQL 子集(LIST/TABLE/FROM folder+tag/WHERE 比较+contains/startswith+AND/OR/NOT/SORT/LIMIT 硬顶100/相对时间-7d)+手写递归下降+参数化 planner(字段白名单/深度32)+`POST /api/nodes/query`(仅本人节点/5s 超时)+前端 ` ```dataview `/` ```index ` 块(LIST/TABLE/index 预设 `LIST FROM 当前目录`)+可点跳转。**实测**:单测 6/6+集成 17/17+浏览器 3 块渲染+跳转。spec = `features/dataview.md`。
- [ ] **自建插件系统**:issue #8(已批准);PS-A..E ≈ 3.5-4.5 周;spec = `plugin-system/DESIGN.md`(§9 已定案)。
  - [x] **PS-A 内核 ✅(2026-11-13)**:`app/types/plugin.ts` 契约 + `services/plugins/{manifest,registry,context,loader}.ts`(manifest 全校验:slug/semver/apiVersion 主版本/entry 路径穿越/capabilities 白名单;DisposableBag LIFO;host 载入-隔离-卸载;ctx L1 = nodes/resources/search/settings(KV)/ui.notice/net 门禁)+ `app/plugins/plugin-host.client.ts` 由 `NUXT_PUBLIC_DEV_PLUGINS` 驱动 + `public/plugins/demo` 参考插件 + docs `5.administer/3.plugins.md`。验证:25 项内核断言全过(含故意坏插件隔离、重复 id、错误主版本、`..` 穿越)+ 真实 HTTP 分发 + ESM 求值 e2e(demo 插件经 dev server 载入/unload)。
  - [x] **PS-B 第一片 ✅(2026-11-13,真浏览器验证)**:`ctx.ui.command`/`ctx.ui.ribbon` + `services/plugins/ui.ts`(反应式注册表、`<pluginId>:` 命名空间、重复 id 抛错、unload 自动回收)+ 命令面板 `Plugins` 分组(Enter/点击执行)+ Dock 渲染插件按钮;demo 插件示范两者。断言 12 项全过;真实浏览器确认:命令面板出现 `Plugins` 分组并执行(notice 回显)、Dock 出现插件按钮。
  - [x] **PS-D 分发 ✅(2026-11-14)**:`migration 0011`(plugins/plugin_grants/plugin_kv)+ 管理员 zip 安装(`POST /api/admin/plugins`,服务端 manifest 全校验、路径穿越/大小/条目上限)+ 产物存 S3 `plugins/<id>/<version>/` 并同源服务 entry/asset + 每用户能力同意(`PUT /plugins/:id/grants`,只接受 manifest 声明的)+ 每插件 KV(`/plugins/:id/kv`,持久化到 pg)+ 审计入 `logs` + `GET /api/plugins` 供 loader + 卸载删产物/授权/KV。**另加 `net.proxy`**(后端代取破 CORS,域名白名单 + 仅 GET/POST + 8 MiB + SSRF 守卫拒绝私网/IP 字面量),替代剪藏对 camoufox 的依赖(DESIGN §11)。验证:集成用例 `plugin_registry_lifecycle` 活环境通过 + API 全链(安装/列表/entry/grant/kv/proxy/SSRF 拒绝/卸载)+ 真浏览器(loader 从注册表加载已装插件、命令面板执行、设置页列出能力)。
  - [x] **PS-B 收尾 ✅(2026-11-14)**:设置页 Plugins(管理员安装/启停/卸载、能力勾选、非管理员只看可加载项)。
  - [x] **PS-C md/编辑器 ✅(2026-11-14)**:`ctx.markdown.registerCodeBlock/registerContainer/registerPostProcessor` + `ctx.editor.registerCmExtension`;注册表 globalThis 单例(避免 bundler 双实例),编译器按 hook 版本重建缓存;容器规则 raw-capture,未认领的 `:::name` 仍归内建容器;fence 回落到宿主渲染器(高亮/复制按钮不受影响)。验证:15 项 bun 断言 + 真浏览器(figure/aside/post-processor 三件套在真实文档渲染)。
  - [x] **PS-E 首批官方插件 ✅(2026-11-14)**:`plugins/official/{recent-files,daily-note,web-clipper}` + `scripts/build-plugin-packs.sh` 打包到 `frontend/public/plugin-packs/*.zip`(设置页可直接安装)。web-clipper 走 `net.proxy` 抓正文,不依赖浏览器自动化;站点专用 parser 未移植(README 已注明)。
  - [ ] 插件系统剩余:固定槽位视图、`ui.modal`、插件设置页声明式 schema、drawio 迁移、ReadItLater 站点 parser 移植。
  - **PS-E 剪藏不走 camoufox MCP**(2026-11-14 用户裁决,详见 `plugin-system/DESIGN.md` §11):思源 ReadItLater 依赖 camoufox 是插件沙箱限制;Pergame 用 `net.fetch`(页面直取)+ 新增 `net.proxy`(后端代取,破 CORS)+ 用户提供 cookie/粘贴兜底。不读其它标签页 DOM(Web 无此权限)。

- [x] **笔记软连接/克隆**(Trilium 对位)✅(2026-11-09,NL-A/B/C 三阶段):克隆表 node_links + 环校验 + 端点 + 树 CTE 扩展 + 前端;规格 = `features/note-links.md`。


## 回收站(用户裁决 2026-11-09:本地误删恢复走回收站,不做 git 级版本)
- [x] **回收站机制** ✅(2026-11-09):软删 + 保留期 + 恢复/彻底删/清空 + 树查询过滤 + 前端视图。spec = `features/trash.md`。
  - 上游 Alexandrie 无回收站(物理 `DELETE FROM nodes` + 级联 + 删 S3 对象),本机实测确认。

## 待人工验证

> **2026-11-10 成熟度评估**:可开始人工测试,定位为「开发验证测试」而非验收测试。(2026-11-13 校数:128 单测 + doc-tests + 18 集成全绿,~21k 行 Rust / 80 文件,10 migration)
> 后端 80 路由 / 218 Vue;
> 认证/2FA/OIDC/节点/软链接/回收站/快照/还原/fan-out/只读锁均已对真实 pg + 对象存储活测。
> 部署步骤 = 思源 `Pergame 外部基础设施部署指南`(id 20261111085400-74zeey6);旧版指南已删。

**拦验收不拦测试的缺口:**

1. **自包含版 compose 仍未跑过**——2026-11-12:external 版随 v0.0.4→v0.0.11 在 QNAP Portainer stack 42 常驻(backend/frontend 双容器,镜像钉 tag),health/静态/SSR/备份全链/WebDAV/CDN 鉴权读均已活测。**仍未知**:自包含版(`docker-compose.yml`,pg + RustFS 一起起)的卷挂载与 healthcheck 依赖链。
2. ~~**dr-backup 真实备份从未跑成**~~ —— **已失效(2026-11-12 快照=全备份,边车删除 `06a30906`)**:实例 DR 改由用户级快照直写 targets 覆盖,本条作废。
3. **前端交互走查 ✅ 2026-11-13(Playwright 真浏览器,本机最新代码 + NAS 基础设施)**——建文档/保存/树内可见、CDN 页上传附件 + 鉴权读 200(匿名 401)、回收站列表 + Restore、零 target 快照一次性 token 链接 + zip 结构(`meta.json`/`manifest.json`/`blobs/`)+ token 单次消费、**快照目标页全链**(保存前 Test 门禁 → Test connection → Save target → 行内 Test → Edit/Cancel → Sync now(`copied 6 skipped 1 pruned 0`)→ 有对象时 Delete 被拒 → **Empty 按钮** → Delete 成功)、**快照列表 + 二次确认还原**(首击 arm 出 plan 统计 `Incoming 2 · Current 2 · Replaced 2 · Added 0 · Removed 0` + 覆盖文件提示 + Profile 提示;勾 force 后二击执行,服务端自动生成 Pre-restore safety snapshot)。**同轮修**:目标行补「Empty」按钮(此前 empty 只有 API,UI 无法自解 delete-check)。测试账号/目录/target/对象已全清。
4. **frontend flags 的 `.env`→compose 断链**(2026-11-11 发现):`CONFIG_HIDE_NATIVE_LOGIN`/`CONFIG_OIDC_PROVIDER_AUTO_REDIRECT` 在 `.env.example` 与 docs 有名,但自包含 compose 未映射成 `NUXT_PUBLIC_CONFIG_HIDE_NATIVE_LOGIN`/`NUXT_PUBLIC_CONFIG_OIDC_PROVIDER_AUTO_REDIRECT` → 设了不生效;OIDC-only 部署会踩。修法 = compose frontend 块补两行映射。
5. ~~**目录索引的点击链**~~ —— **已验(2026-11-13,Playwright + Chromium 真浏览器)**:侧边栏 index 紧跟目录、index 块过滤容器与 index 自身且目录前置、点目录行跳该目录 index、目录页「Open index file」跳转、`displayIndexFiles` 关闭后侧边栏无 index(目录仍在)。另:同轮修掉该批次遗留的 SFC 模板缺闭合标签(设置页整个不可用),并新增 `bun run lint:sfc` 闸门。

6. **线上部署落后 main 30 个提交(2026-11-13 实测)**:QNAP stack 42 仍是 `v0.0.11` 镜像(11-12 16:12 打的 tag),而 `281bf11e`(附件改走后端鉴权读,11-12 17:08)以及其后的配额/去重、Rust 质量、MCP、#4 index、#5 dataview、PS-A、PS-B 全在 tag 之外(`git tag --contains 281bf11e` 为空)。实测证据:同一测试附件在本机最新后端 `GET /api/resources/:id/file` → **200 / 39 B**,线上 `:8200` 同路径 → **404(空 body = 路由不存在)**。即线上附件/CDN 读是坏的,修法 = 用当前 main 打 **v0.0.12** 并 `PUT /api/stacks/42` 更新(NAS 恢复在线后执行)。

**明确未实现(按批次,非 bug)**:批次 A ✅全清(媒体播放器 ✅2026-11-12、Joplin md ✅2026-11-13、M0 缩水 ✅2026-11-13)、Dataview ✅2026-11-13、目录索引 ✅2026-11-13;剩余:插件系统(#8)。(Pergamena 改名 2026-11-14 用户裁决撤销:当前名称已合适。)

### 建议测试顺序

- [x] **1. compose 起全栈**:external 版(2 服务)✅ 2026-11-12 于 QNAP Portainer stack 42(镜像钉 tag,随 v0.0.4→v0.0.11 逐版跟进),`/api/health` → `{"database":true,"status":"ok"}`。自包含版(4 服务,pg+RustFS+backend+frontend)仍待测。
- [x] **2. 核心 CRUD** ✅ 2026-11-13(Playwright,本机最新代码 + NAS 基础设施):注册 201 → 登录进 `/dashboard/home` → 新建文档(标题 + CodeMirror 正文)保存跳 `/dashboard/docs/edit/<id>` → 树内可见 → 正文渲染 → CDN 页上传附件(`AppDrop` file input + Upload)→ `GET /api/resources/:id/file` 带 cookie **200 / 39 B**,匿名 401。
- [x] **3. 回收站** ✅ 2026-11-13(Playwright):`DELETE /api/nodes/:id` → `/dashboard/trash` 列出(名称/大小/删除时间)→ 点 Restore → 节点恢复可读、trash 列表 0 行。
- [x] **4. 快照全链** ✅ 2026-11-13:零 target → 一次性 token 链接 + zip(`meta.json`/`manifest.json`/`blobs/<sha256>`,token 二次访问 404);有 target → 快照进列表(`format v2`、size、memo)→ 首击 arm 显示 plan 统计 + 覆盖文件/Profile 提示 → 勾 force 二击执行 → 服务端自动建 Pre-restore safety snapshot。**未验**:只读提示条(`MaintenanceBanner`,还原太快抓不到窗口)、上传 zip 回灌。
- [x] **5. 异地目标** ✅ 2026-11-13(Playwright):draft probe `Reachable · 0 snapshot object(s) · <fingerprint>` → Save(保存前 Test 门禁生效)→ 行内 Test → Sync now(`copied 6 skipped 1 pruned 0`)→ 有对象时 Delete 被拒(delete-check 409)→ **Empty**(`deleted_objects:10`)→ Delete 成功。
- [x] **6. ~~实例 DR(dr-backup)~~ 已随边车删除作废** → 改为:零 target 快照(S3 inline zip + 有效期 + token 链接下载)✅ 2026-11-13 验证通过。
- [ ] **7. 进阶**:2FA(enroll/verify/recovery)、OIDC(Keycloak 环境已有)、SMTP 邮件。

### 旧条目

- [ ] 自托管 smoke(本机无 docker):`docker compose up --build` 后端正常启动(`S3_*` 链路)、注册/登录可用。改名后默认值已切 `pergame`(DB/桶/CDN 路径),首次起库即新名。
- [ ] GitHub repo secrets:`MINIO_ACCESSKEY`/`MINIO_SECRETKEY` 手动改名 `S3_ACCESSKEY`/`S3_SECRETKEY`(deploy_to_prod.yml 引用已改)。
- [x] ~~`scripts/dr-backup.sh` 实机验证~~ —— **作废**(2026-11-12 边车删除,脚本/镜像/文档同批移除;实例 DR 由快照直写替代)。
- [ ] push 51+ commits 到 origin(kamasylvia/Pergame)——待拍板(公开仓 push 前可按全局规则压缩重组)。(2026-11-09:首 15 个已压缩为 5 主题 commits 并推上 origin/main `6e0abd8a`;此后增量照常。)

## 运维约束(2026-11-12)

- **migration `0007`–`0010` 已在生产库 `pergame` 落库 → 文件冻结**:改动会让下次部署 checksum 校验失败。要调整表结构,只能追加 `0011_*`。
- 生产库当前 4 用户**均非 admin**,`/api/admin/backup/*` 对所有人 403;恢复需 `ADMIN_ACCOUNTS=<uuid>` 或改库。
- 生产回收站存有更早的测试遗留(用户在 2026-11-12 明确:**不用清,以后统一清库**)。

## 维护说明

- 项目约定与决策记录 → 根 `AGENTS.md`。
- 批次/续接点 → `pergame-infra/GOAL-STATE.md`;教训 → `LESSONS.md`;spec → `features/`、`plugin-system/`、`rust-migration/`、`obsidian-compat/`。
- issue 索引:#1 pg · #2 S3 改名(已实施) · #3 MCP · #4 索引 · #5 dataview · #6/#7 批次 A · #8 插件系统。

## 备份子系统(2026-11-10 三批全落地)

- [ ] **实机验证**:有 docker 的机器上跑 `docker compose up` → Instance DR 配 conf → 立即备份 / `-l` / `-R`;前端异地方向(settings → Offsite snapshot targets)建 target 跑一次真实异地复制。

## 换机续接(2026-11-09)

- 续接点唯一正本 = `.work/pergame-infra/GOAL-STATE.md` §3(当前精确位置 / 下一步动作 / 环境指引 / 验证门 / 最近批次)。
- 环境重建三件套:`rustup override set 1.95.0`(backend-rs)、`GOPROXY=https://goproxy.cn,direct`、`bun install --registry https://registry.npmmirror.com`。
