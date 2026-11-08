# TODO

> 2026-11-07 重构:双批次结构。已完成事项入 commit 历史 + AGENTS.md 决策记录,不留在本文件。

## 批次 A — 前端批次(纯前端,独立于重写;**待开工令**)

- [ ] **媒体播放器内置化**(video-md → 内置):issue #6;`![alt](url.mp4...)` → video/audio renderer override + PDF 内嵌;0.5-1 天;spec = `features/markdown-parity.md` §2。
- [ ] **Joplin md 特性集对齐**:issue #7;breaks/typographer(带设置开关)/ins/multimd-table/deflist/abbr/embeds/ABC;约 5-6 天;spec 同上 §3。
- [ ] **M0 ob 语法兼容(全量四件,2026-11-07 采纳)**:wikilinks 解析(名字解析走搜索 API)、callout 渲染(复用现有容器 CSS)、properties/frontmatter 渲染(schema 与 #5 dataview `metadata.*` 同批设计)、导入改写;`![[embed]]`/transclusion 后置;3-5 天。

## 批次 B — 重写批次(Rust + pg + 功能域;**等用户开工令**)

- [ ] **Rust 后端重写 + PostgreSQL**:issue #1;db 层 = Toasty + sqlx 逃逸层(P1 spike 验证不过则回退 sqlx 为主);全新空库;pg GIN + tsvector。**P0 前置已完成(2026-11-08,见 `rust-migration/RESEARCH.md` §8)**;下一步 = P1(骨架 + Toasty spike),待开工令。
  - [ ] P0 集成测试运行时验证(待人工:需 mysql + minio + 活后端;`go test ./tests/...`)
- [ ] **MCP server**:issue #3;读写全量、streamable HTTP `/api/mcp`、rmcp、每用户 API token;spec = `features/mcp-server.md`。
- [ ] **目录索引文件**:issue #4;自动建 index、设置管名(批量改名)/可见性、永远置顶(order=-1 双端强制);index 块 = #5 预设;spec = `features/directory-index.md`。
- [ ] **Dataview 查询引擎**:issue #5;DQL 子集、内置字段+metadata JSONB、`POST /api/nodes/query`、前端容器块;spec = `features/dataview.md`。
- [ ] **自建插件系统**:issue #8(已批准);PS-A..E ≈ 3.5-4.5 周,排重写主体后;ReadItLater 移植 = 生态验证头牌;spec = `plugin-system/DESIGN.md`(§9 已定案)。

- [ ] **笔记软连接/克隆**(Trilium 对位):同笔记多目录出现;克隆表 node_links + 环校验 + 三端点 + 树 CTE 扩展 + 前端;规格 = `features/note-links.md`(已含模型决策:克隆表方案,否决 content 分离重构)。

## 待人工验证

- [ ] 自托管 smoke(本机无 docker):`docker compose up --build` 后端正常启动(`S3_*` 链路)、注册/登录可用。
- [ ] GitHub repo secrets:`MINIO_ACCESSKEY`/`MINIO_SECRETKEY` 手动改名 `S3_ACCESSKEY`/`S3_SECRETKEY`(deploy_to_prod.yml 引用已改)。
- [ ] `scripts/dr-backup.sh` 实机验证(需 pg_dump + rclone + 真实目的地):dry-run 全链路 + 一次真实 dump/copy/prune;conf 复制自 `dr-backup.conf.example`(chmod 600)。

- [ ] **笔记软连接/克隆**(Trilium 对位):同笔记多目录出现;克隆表 node_links + 环校验 + 三端点 + 树 CTE 扩展 + 前端;规格 = `features/note-links.md`(已含模型决策:克隆表方案,否决 content 分离重构)。

## 待人工验证

- [ ] push 51+ commits 到 origin(kamasylvia/Collectanea)——遵令未 push,待拍板(公开仓 push 前可按全局规则压缩重组)。

## 维护说明

- 项目约定与决策记录 → 根 `AGENTS.md`。
- 批次/续接点 → `collectanea-infra/GOAL-STATE.md`;教训 → `LESSONS.md`;spec → `features/`、`plugin-system/`、`rust-migration/`、`obsidian-compat/`。
- issue 索引:#1 pg · #2 S3 改名(已实施) · #3 MCP · #4 索引 · #5 dataview · #6/#7 批次 A · #8 插件系统。
